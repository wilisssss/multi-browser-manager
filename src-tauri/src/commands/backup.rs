use crate::browser::launcher;
use crate::commands::profile::db_lock;
use crate::commands::proxy::insert_proxy_with_password;
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use crate::models::proxy::Proxy;
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Portable backup file format. Passwords are intentionally excluded —
/// see plan section 7.5.
#[derive(Debug, Serialize, Deserialize)]
struct BackupFile {
    format_version: u32,
    exported_at: i64,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(default)]
    proxies: Vec<Proxy>,
    #[serde(default)]
    credentials: Vec<CredentialBackup>,
}

/// Portable credential record. Includes secrets by design (local-only usage):
/// exports are meant to migrate everything to a new device in one file.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CredentialBackup {
    profile_id: String,
    platform: String,
    label: String,
    username: Option<String>,
    password: Option<String>,
    seed_phrase: Option<String>,
    evm_address: Option<String>,
    notes: Option<String>,
}

impl From<&crate::commands::credentials::Credential> for CredentialBackup {
    fn from(c: &crate::commands::credentials::Credential) -> Self {
        Self {
            profile_id: c.profile_id.clone(),
            platform: c.platform.clone(),
            label: c.label.clone(),
            username: c.username.clone(),
            password: c.password.clone(),
            seed_phrase: c.seed_phrase.clone(),
            evm_address: c.evm_address.clone(),
            notes: c.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported_profiles: usize,
    pub imported_proxies: usize,
    pub imported_credentials: usize,
    pub skipped_profiles: usize,
    pub skipped_proxies: usize,
    pub skipped_credentials: usize,
    pub conflicts: Vec<String>,
}

const FORMAT_VERSION: u32 = 1;

#[tauri::command]
pub fn export_profiles(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let conn = db_lock(&state)?;
    write_backup_file(&conn, std::path::Path::new(&path))
}

/// Writes a portable backup (profiles + proxies, no passwords) to `path`.
/// Shared by manual export and the automatic snapshot task.
pub fn write_backup_file(conn: &rusqlite::Connection, path: &std::path::Path) -> AppResult<()> {
    let profiles: Vec<Profile> = crate::commands::profile::list_profiles(conn)?;

    let proxies: Vec<Proxy> = crate::commands::proxy::list_proxies(conn)?;

    let credentials: Vec<CredentialBackup> =
        crate::commands::credentials::list_all_credentials(conn)?
            .iter()
            .map(CredentialBackup::from)
            .collect();

    let backup = BackupFile {
        format_version: FORMAT_VERSION,
        exported_at: chrono::Utc::now().timestamp(),
        profiles,
        proxies,
        credentials,
    };

    std::fs::write(path, serde_json::to_string_pretty(&backup)?)?;
    Ok(())
}

/// Automatic snapshot: writes a timestamped backup into `<data>/backups/` if
/// the newest snapshot is older than 24 h, then prunes to the newest
/// `auto_backup_keep` files. Returns the written path on success.
pub fn run_snapshot_if_due(state: &AppState) -> Option<std::path::PathBuf> {
    let (settings, conn) = {
        let conn = db_lock(state).ok()?;
        (crate::commands::settings::load_settings(&conn), conn)
    };
    if !settings.auto_backup {
        return None;
    }

    let dir = state.profiles_root.parent()?.join("backups");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("mbm: failed to create backups dir: {e}");
        return None;
    }

    // Collect existing snapshots, newest modification time first.
    let mut snapshots: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("mbm-auto-") && name.ends_with(".json") {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((mtime, e.path()))
            } else {
                None
            }
        })
        .collect();
    snapshots.sort_by(|a, b| b.0.cmp(&a.0));

    // Due when there is no snapshot yet or the newest is older than 24 h.
    let due = match snapshots.first() {
        None => true,
        Some((mtime, _)) => mtime.elapsed().map(|d| d.as_secs() > 24 * 3600).unwrap_or(true),
    };
    if !due {
        return None;
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("mbm-auto-{stamp}.json"));
    if let Err(e) = write_backup_file(&conn, &path) {
        eprintln!("mbm: auto-backup failed: {e}");
        return None;
    }

    // Prune older snapshots beyond the configured keep count.
    let keep = settings.auto_backup_keep.max(1) as usize;
    for (_, old) in snapshots.iter().skip(keep.saturating_sub(1)) {
        if let Err(e) = std::fs::remove_file(old) {
            eprintln!("mbm: failed to prune old snapshot {}: {e}", old.display());
        }
    }
    eprintln!("mbm: auto-backup written to {}", path.display());
    Some(path)
}

/// Exposes the auto-backup directory so the UI can display it.
#[tauri::command]
pub fn get_backup_dir(state: State<'_, AppState>) -> AppResult<String> {
    let dir = state
        .profiles_root
        .parent()
        .map(|p| p.join("backups"))
        .ok_or_else(|| AppError::Internal("No data root configured".into()))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn import_profiles(state: State<'_, AppState>, path: String) -> AppResult<ImportResult> {
    let content = std::fs::read_to_string(&path)?;
    let backup: BackupFile = serde_json::from_str(&content).map_err(|e| {
        AppError::Validation(format!("Invalid backup file: {e}"))
    })?;

    if backup.format_version > FORMAT_VERSION {
        return Err(AppError::Validation(format!(
            "Backup file version {} is newer than supported version {FORMAT_VERSION}",
            backup.format_version
        )));
    }

    // Validate every entry BEFORE touching the DB so a malformed file fails
    // cleanly instead of half-importing.
    for proxy in &backup.proxies {
        if proxy.label.trim().is_empty() {
            return Err(AppError::Validation(
                "Backup contains a proxy with an empty label".into(),
            ));
        }
        if let Err(e) = crate::proxy_manager::ensure_valid_protocol(&proxy.protocol) {
            return Err(AppError::Validation(format!(
                "Backup proxy '{}' has an invalid protocol: {e}",
                proxy.label
            )));
        }
        if proxy.host.trim().is_empty() || !(1..=65535).contains(&proxy.port) {
            return Err(AppError::Validation(format!(
                "Backup proxy '{}' has an invalid host/port",
                proxy.label
            )));
        }
    }
    for profile in &backup.profiles {
        if profile.name.trim().is_empty() {
            return Err(AppError::Validation(
                "Backup contains a profile with an empty name".into(),
            ));
        }
    }

    let mut conn = db_lock(&state)?;
    let mut result = ImportResult {
        imported_profiles: 0,
        imported_proxies: 0,
        imported_credentials: 0,
        skipped_profiles: 0,
        skipped_proxies: 0,
        skipped_credentials: 0,
        conflicts: Vec::new(),
    };

    // One transaction for the whole import: a failure rolls back everything
    // instead of leaving a partial import behind.
    let tx = conn.transaction()?;

    // Backup credentials reference the profile ids of the exporting machine;
    // map them to the ids actually created (or already present) here.
    let mut profile_id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Import proxies first so profile FK references stay valid.
    for proxy in &backup.proxies {
        let conflict: Option<String> = tx
            .query_row(
                "SELECT label FROM proxies WHERE id = ?1 OR label = ?2 LIMIT 1",
                params![proxy.id, proxy.label],
                |r| r.get::<_, String>(0),
            )
            .ok();

        if conflict.is_some() {
            result.skipped_proxies += 1;
            result
                .conflicts
                .push(format!("Proxy '{}' already exists — skipped", proxy.label));
            continue;
        }

        insert_proxy_with_password(&tx, proxy, None)?;
        result.imported_proxies += 1;
    }

    for profile in &backup.profiles {
        let conflict: Option<String> = tx
            .query_row(
                "SELECT name FROM profiles WHERE name = ?1 COLLATE NOCASE OR user_data_dir = ?2 LIMIT 1",
                params![profile.name, profile.user_data_dir],
                |r| r.get::<_, String>(0),
            )
            .ok();

        if conflict.is_some() {
            result.skipped_profiles += 1;
            result
                .conflicts
                .push(format!("Profile '{}' already exists — skipped", profile.name));
            // Credentials of a skipped profile attach to the existing profile.
            if let Ok(local_id) = tx.query_row(
                "SELECT id FROM profiles WHERE name = ?1 COLLATE NOCASE LIMIT 1",
                params![profile.name],
                |r| r.get::<_, String>(0),
            ) {
                profile_id_map.insert(profile.id.clone(), local_id);
            }
            continue;
        }

        // Create a fresh data dir for the imported profile on this machine.
        let new_id = uuid::Uuid::new_v4().to_string();
        let user_data_dir = state
            .profiles_root
            .join(&new_id)
            .to_string_lossy()
            .to_string();
        launcher::ensure_user_data_dir(std::path::Path::new(&user_data_dir))?;

        // If the referenced proxy didn't make it in (skipped on conflict or
        // missing from the backup), drop the reference instead of failing the
        // whole import with a foreign-key violation.
        let proxy_id = match &profile.proxy_id {
            Some(pid) => {
                let exists: bool = tx.query_row(
                    "SELECT COUNT(*) > 0 FROM proxies WHERE id = ?1",
                    params![pid],
                    |r| r.get(0),
                )?;
                if exists { Some(pid.clone()) } else { None }
            }
            None => None,
        };

        let now = chrono::Utc::now().timestamp();
        tx.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, proxy_id, notes, status, created_at, updated_at, last_used_at, pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'stopped', ?7, ?8, NULL, ?9)",
            params![
                new_id,
                profile.name,
                profile.browser_type,
                user_data_dir,
                proxy_id,
                profile.notes,
                now,
                now,
                profile.pinned as i64,
            ],
        )?;

        // Restore tag assignments; tags missing from this DB are skipped.
        for group in &profile.groups {
            let exists: bool = tx.query_row(
                "SELECT COUNT(*) > 0 FROM groups WHERE name = ?1",
                params![group.name],
                |r| r.get(0),
            )?;
            if exists {
                tx.execute(
                    "INSERT OR IGNORE INTO profile_groups (profile_id, group_id)
                     SELECT ?1, id FROM groups WHERE name = ?2",
                    params![new_id, group.name],
                )?;
            }
        }

        profile_id_map.insert(profile.id.clone(), new_id);

        result.imported_profiles += 1;
    }

    // Import credentials last, remapped to the local profile ids.
    for cred in &backup.credentials {
        let Some(target_profile) = profile_id_map.get(&cred.profile_id) else {
            result.skipped_credentials += 1;
            result
                .conflicts
                .push(format!("Credential '{}' — profile not found, skipped", cred.label));
            continue;
        };

        // Dedupe: same profile + platform + username already present.
        let dup: bool = tx.query_row(
            "SELECT COUNT(*) > 0 FROM credentials WHERE profile_id = ?1 AND platform = ?2
             AND COALESCE(username, '') = COALESCE(?3, '')",
            params![target_profile, cred.platform, cred.username],
            |r| r.get(0),
        )?;
        if dup {
            result.skipped_credentials += 1;
            continue;
        }

        tx.execute(
            "INSERT INTO credentials (id, profile_id, platform, label, username, password, seed_phrase, evm_address, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                uuid::Uuid::new_v4().to_string(),
                target_profile,
                cred.platform,
                cred.label,
                cred.username,
                cred.password,
                cred.seed_phrase,
                cred.evm_address,
                cred.notes,
                chrono::Utc::now().timestamp()
            ],
        )?;
        result.imported_credentials += 1;
    }

    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_file_round_trips() {
        let backup = BackupFile {
            format_version: FORMAT_VERSION,
            exported_at: 1234567890,
            profiles: vec![Profile {
                id: "id-1".into(),
                name: "Work".into(),
                browser_type: "chromium".into(),
                user_data_dir: "/tmp/mbm/id-1".into(),
                proxy_id: None,
                notes: Some("hello".into()),
                status: "stopped".into(),
                created_at: 1,
                updated_at: 1,
                last_used_at: None,
                pinned: true,
                groups: vec![],
            }],
            proxies: vec![Proxy {
                id: "px-1".into(),
                label: "lab".into(),
                protocol: "http".into(),
                host: "127.0.0.1".into(),
                port: 8080,
                username: None,
                created_at: 1,
            }],
            credentials: vec![CredentialBackup {
                profile_id: "id-1".into(),
                platform: "x".into(),
                label: "X".into(),
                username: Some("@handle".into()),
                password: Some("pw".into()),
                seed_phrase: Some("word1 word2".into()),
                evm_address: Some("0xabc".into()),
                notes: None,
            }],
        };

        let json = serde_json::to_string(&backup).unwrap();
        let parsed: BackupFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.profiles.len(), 1);
        assert!(parsed.profiles[0].pinned);
        assert_eq!(parsed.proxies.len(), 1);
        assert_eq!(parsed.proxies[0].label, "lab");
        // Secrets survive the round trip — that is the point of the export.
        assert_eq!(parsed.credentials.len(), 1);
        assert_eq!(parsed.credentials[0].password.as_deref(), Some("pw"));
        assert_eq!(parsed.credentials[0].seed_phrase.as_deref(), Some("word1 word2"));
        assert_eq!(parsed.credentials[0].profile_id, "id-1");
    }

    #[test]
    fn deserializes_minimal_backup_file() {
        // Older or hand-edited files: profiles/proxies may be absent entirely.
        let json = r#"{"format_version":1,"exported_at":42}"#;
        let parsed: BackupFile = serde_json::from_str(json).expect("defaults should fill the rest");
        assert!(parsed.profiles.is_empty());
        assert!(parsed.proxies.is_empty());
    }

    #[test]
    fn profile_deserializes_without_pinned_field() {
        // Pre-pinned-era profile JSON must still parse (serde default false).
        let json = r#"{
            "id": "x", "name": "Old", "browserType": "chromium",
            "userDataDir": "/tmp/mbm/x", "status": "stopped",
            "createdAt": 1, "updatedAt": 1
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert!(!p.pinned);
        assert!(p.groups.is_empty());
    }
}
