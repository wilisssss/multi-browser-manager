//! Backup import/export engine (moved from `commands/backup.rs` — A3).
//!
//! Pure functions over `&Connection` + paths: no Tauri state, so the import
//! conflict rules are unit-testable against an in-memory database.

use crate::browser::launcher;
use crate::commands::profile::{insert_profile, list_profiles};
use crate::commands::proxy::insert_proxy_with_password;
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use crate::models::proxy::Proxy;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Portable backup file format. Proxy passwords are excluded (they live in
/// the OS keychain and cannot be exported); credential secrets ARE included
/// by design so one file migrates a whole farming setup between devices —
/// see the README warning about where backups are stored.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupFile {
    pub format_version: u32,
    pub exported_at: i64,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub proxies: Vec<Proxy>,
    #[serde(default)]
    pub credentials: Vec<CredentialBackup>,
}

/// Portable credential record. Includes secrets by design (local-only usage):
/// exports are meant to migrate everything to a new device in one file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CredentialBackup {
    pub profile_id: String,
    pub platform: String,
    pub label: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub seed_phrase: Option<String>,
    pub evm_address: Option<String>,
    pub notes: Option<String>,
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

impl ImportResult {
    fn empty() -> Self {
        ImportResult {
            imported_profiles: 0,
            imported_proxies: 0,
            imported_credentials: 0,
            skipped_profiles: 0,
            skipped_proxies: 0,
            skipped_credentials: 0,
            conflicts: Vec::new(),
        }
    }
}

const FORMAT_VERSION: u32 = 1;

/// Raw backup content, without the per-run `exported_at` stamp — hashing the
/// serialized form of this is how "only if changed" actually detects change
/// (the full JSON always differs because of the timestamp).
#[derive(Serialize)]
struct BackupPayload {
    format_version: u32,
    profiles: Vec<Profile>,
    proxies: Vec<Proxy>,
    credentials: Vec<CredentialBackup>,
}

/// Reads profiles + proxies + credentials from the DB (the query part of a
/// backup, shared by export, snapshots and the change detector).
fn build_payload(conn: &Connection) -> AppResult<BackupPayload> {
    let profiles = list_profiles(conn)?;
    let proxies = crate::commands::proxy::list_proxies(conn)?;
    let credentials: Vec<CredentialBackup> =
        crate::commands::credentials::list_all_credentials(conn)?
            .iter()
            .map(CredentialBackup::from)
            .collect();
    Ok(BackupPayload {
        format_version: FORMAT_VERSION,
        profiles,
        proxies,
        credentials,
    })
}

fn serialize_backup(payload: &BackupPayload, exported_at: i64) -> AppResult<String> {
    let backup = BackupFile {
        format_version: payload.format_version,
        exported_at,
        profiles: payload.profiles.clone(),
        proxies: payload.proxies.clone(),
        credentials: payload.credentials.clone(),
    };
    Ok(serde_json::to_string_pretty(&backup)?)
}

/// Writes a plaintext portable backup (profiles + proxies + credentials) to
/// `path`. Shared by manual export and the automatic snapshot task.
pub fn write_backup_file(conn: &Connection, path: &Path) -> AppResult<()> {
    let payload = build_payload(conn)?;
    let json = serialize_backup(&payload, chrono::Utc::now().timestamp())?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Writes a passphrase-encrypted backup (feature: encrypted export). The
/// plaintext payload is identical to the plaintext export; only the file on
/// disk differs.
pub fn write_encrypted_backup_file(
    conn: &Connection,
    path: &Path,
    passphrase: &str,
) -> AppResult<()> {
    super::crypto::validate_passphrase(passphrase)?;
    let payload = build_payload(conn)?;
    let json = serialize_backup(&payload, chrono::Utc::now().timestamp())?;
    let encrypted = super::crypto::encrypt(json.as_bytes(), passphrase)?;
    std::fs::write(path, encrypted)?;
    Ok(())
}

/// Reads + validates + imports a backup file (plaintext or encrypted).
/// `passphrase` is required only for encrypted files; a `None` on an
/// encrypted file returns a clear validation error so the UI can prompt.
pub fn import_backup_file(
    conn: &mut Connection,
    profiles_root: &Path,
    path: &Path,
    passphrase: Option<&str>,
) -> AppResult<ImportResult> {
    let bytes = std::fs::read(path)?;
    let content = if super::crypto::is_encrypted(&bytes) {
        let pass = passphrase.ok_or_else(|| {
            AppError::validation(
                "This backup is encrypted — provide the passphrase to import it",
            )
        })?;
        String::from_utf8(super::crypto::decrypt(&bytes, pass)?)
            .map_err(|_| AppError::validation("Decrypted backup is not valid UTF-8"))?
    } else {
        String::from_utf8(bytes)
            .map_err(|_| AppError::validation("Backup file is not valid UTF-8"))?
    };

    let backup: BackupFile = serde_json::from_str(&content)
        .map_err(|e| AppError::validation(format!("Invalid backup file: {e}")))?;

    if backup.format_version > FORMAT_VERSION {
        return Err(AppError::validation(format!(
            "Backup file version {} is newer than supported version {FORMAT_VERSION}",
            backup.format_version
        )));
    }

    // Validate every entry BEFORE touching the DB so a malformed file fails
    // cleanly instead of half-importing.
    for proxy in &backup.proxies {
        if proxy.label.trim().is_empty() {
            return Err(AppError::validation(
                "Backup contains a proxy with an empty label",
            ));
        }
        if let Err(e) = crate::proxy_manager::ensure_valid_protocol(&proxy.protocol) {
            return Err(AppError::validation(format!(
                "Backup proxy '{}' has an invalid protocol: {e}",
                proxy.label
            )));
        }
        if proxy.host.trim().is_empty() || !(1..=65535).contains(&proxy.port) {
            return Err(AppError::validation(format!(
                "Backup proxy '{}' has an invalid host/port",
                proxy.label
            )));
        }
    }
    for profile in &backup.profiles {
        if profile.name.trim().is_empty() {
            return Err(AppError::validation(
                "Backup contains a profile with an empty name",
            ));
        }
        if let Err(e) = launcher::validate_extra_args(profile.extra_args.as_deref().unwrap_or(""))
        {
            return Err(AppError::validation(format!(
                "Backup profile '{}' has invalid extra args: {e}",
                profile.name
            )));
        }
    }

    apply_import(conn, profiles_root, &backup)
}

/// Applies a validated backup inside one transaction: a failure rolls back
/// everything instead of leaving a partial import behind.
fn apply_import(
    conn: &mut Connection,
    profiles_root: &Path,
    backup: &BackupFile,
) -> AppResult<ImportResult> {
    let mut result = ImportResult::empty();
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
        // Conflict = same name OR same data dir. The id lookup below uses the
        // exact same condition so a dir-conflict under a different name also
        // maps credentials onto the existing profile (was name-only: import
        // silently dropped the credentials in that case).
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
                "SELECT id FROM profiles WHERE name = ?1 COLLATE NOCASE OR user_data_dir = ?2 LIMIT 1",
                params![profile.name, profile.user_data_dir],
                |r| r.get::<_, String>(0),
            ) {
                profile_id_map.insert(profile.id.clone(), local_id);
            }
            continue;
        }

        // Create a fresh data dir for the imported profile on this machine.
        let new_id = uuid::Uuid::new_v4().to_string();
        let user_data_dir = profiles_root
            .join(&new_id)
            .to_string_lossy()
            .to_string();
        launcher::ensure_user_data_dir(Path::new(&user_data_dir))?;

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
        let imported = Profile {
            id: new_id.clone(),
            name: profile.name.clone(),
            browser_type: profile.browser_type.clone(),
            user_data_dir,
            proxy_id,
            notes: profile.notes.clone(),
            status: "stopped".to_string(),
            created_at: now,
            updated_at: now,
            last_used_at: None,
            pinned: profile.pinned,
            extra_args: profile.extra_args.clone(),
            restart_on_crash: profile.restart_on_crash,
            stop_timeout_secs: profile.stop_timeout_secs,
            groups: Vec::new(),
        };
        insert_profile(&tx, &imported)?;

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

/// Deterministic content fingerprint used for "did the config change"
/// equality — never a security property. Deliberately NOT DefaultHasher:
/// its seed is randomized per process, so a hash persisted across restarts
/// would never match. FNV-1a is stable across runs.
fn content_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

fn payload_hash(payload: &BackupPayload) -> AppResult<String> {
    Ok(content_hash(serde_json::to_vec(payload)?.as_slice()))
}

/// Automatic snapshot: writes a timestamped backup into `<data>/backups/` at
/// most once a day AND only when the backup payload actually changed (content
/// hash compared against the newest snapshot's hash, persisted in a sidecar
/// file so it survives MBM restarts), then prunes to the newest
/// `auto_backup_keep` files. Returns the written path on success.
pub fn run_snapshot_if_due(state: &crate::AppState) -> Option<std::path::PathBuf> {
    let (settings, conn) = {
        let conn = crate::commands::profile::db_lock(state).ok()?;
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

    // Build the payload in memory first; only touch disk when its content
    // differs from the last accepted snapshot.
    let payload = match build_payload(&conn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("mbm: auto-backup failed: {e}");
            return None;
        }
    };
    let hash = match payload_hash(&payload) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("mbm: auto-backup hashing failed: {e}");
            return None;
        }
    };
    let hash_file = dir.join(".last-hash");
    if std::fs::read_to_string(&hash_file).ok().as_deref() == Some(hash.as_str()) {
        return None; // nothing changed since the last snapshot
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
    let json = match serialize_backup(&payload, chrono::Utc::now().timestamp()) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("mbm: auto-backup serialization failed: {e}");
            return None;
        }
    };
    if let Err(e) = std::fs::write(&path, json) {
        eprintln!("mbm: auto-backup failed: {e}");
        return None;
    }
    // Remember the accepted content hash (best-effort: a failure here only
    // means the next check writes a duplicate snapshot, not data loss).
    let _ = std::fs::write(&hash_file, &hash);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn mem_conn() -> Connection {
        db::init(Path::new(":memory:")).expect("in-memory db")
    }

    fn tmp_profiles_root(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mbm-import-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Regression test for the audit finding: a backup profile that conflicts
    /// only via `user_data_dir` (different name) was skipped AND its
    /// credentials silently dropped, because the id lookup matched on name
    /// only. Credentials must attach to the existing profile instead.
    #[test]
    fn dir_conflict_maps_credentials_to_existing_profile() {
        let mut conn = mem_conn();
        let root = tmp_profiles_root("dirconflict");
        let existing_dir = root.join("existing").to_string_lossy().to_string();

        // Existing local profile "A" using the data dir that the backup's
        // "B" references.
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES ('local-a', 'A', 'chromium', ?1, 'stopped', 1, 1)",
            params![existing_dir],
        )
        .unwrap();

        let backup = BackupFile {
            format_version: FORMAT_VERSION,
            exported_at: 1,
            profiles: vec![Profile {
                id: "remote-b".into(),
                name: "B".into(),
                browser_type: "chromium".into(),
                user_data_dir: existing_dir.clone(),
                ..Profile::new("remote-b", "B", "chromium", &existing_dir)
            }],
            proxies: vec![],
            credentials: vec![CredentialBackup {
                profile_id: "remote-b".into(),
                platform: "site".into(),
                label: "acct".into(),
                username: Some("@u".into()),
                password: Some("pw".into()),
                ..Default::default()
            }],
        };

        let result = apply_import(&mut conn, &root, &backup).unwrap();
        assert_eq!(result.skipped_profiles, 1);
        assert_eq!(result.imported_profiles, 0);

        let creds: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM credentials WHERE profile_id = 'local-a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(creds, 1, "credential must attach to the existing profile");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fresh_profiles_import_with_new_ids_and_credentials() {
        let mut conn = mem_conn();
        let root = tmp_profiles_root("fresh");

        let backup = BackupFile {
            format_version: FORMAT_VERSION,
            exported_at: 1,
            profiles: vec![Profile::new("remote-1", "Fresh", "chromium", "/old/dir")],
            proxies: vec![],
            credentials: vec![CredentialBackup {
                profile_id: "remote-1".into(),
                platform: "site".into(),
                label: "acct".into(),
                password: Some("pw".into()),
                ..Default::default()
            }],
        };

        let result = apply_import(&mut conn, &root, &backup).unwrap();
        assert_eq!(result.imported_profiles, 1);
        assert_eq!(result.imported_credentials, 1);

        let (local_id, dir): (String, String) = conn
            .query_row(
                "SELECT id, user_data_dir FROM profiles WHERE name = 'Fresh'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_ne!(local_id, "remote-1", "imported profiles get fresh local ids");
        assert!(dir.starts_with(&root.to_string_lossy().to_string()));
        std::path::Path::new(&dir).exists().then_some(()).expect("data dir created");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn encrypted_export_round_trips_through_import_validation() {
        let root = tmp_profiles_root("enc");
        let payload = BackupPayload {
            format_version: FORMAT_VERSION,
            profiles: vec![],
            proxies: vec![],
            credentials: vec![],
        };
        let json = serialize_backup(&payload, 42).unwrap();
        let enc = super::super::crypto::encrypt(json.as_bytes(), "backup pass").unwrap();
        assert!(super::super::crypto::is_encrypted(&enc));
        let dec = super::super::crypto::decrypt(&enc, "backup pass").unwrap();
        let parsed: BackupFile = serde_json::from_slice(&dec).unwrap();
        assert_eq!(parsed.exported_at, 42);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn content_hash_is_stable_and_change_sensitive() {
        let a = content_hash(b"profiles+proxies");
        assert_eq!(a, content_hash(b"profiles+proxies"));
        assert_ne!(a, content_hash(b"profiles+proxies!"));
    }

    #[test]
    fn payload_hash_survives_timestamp_only_changes() {
        fn payload(n_profiles: u32) -> BackupPayload {
            let profiles = (0..n_profiles)
                .map(|i| Profile::new(format!("id-{i}"), format!("P{i}"), "chromium", format!("/tmp/mbm/{i}")))
                .collect();
            BackupPayload {
                format_version: FORMAT_VERSION,
                profiles,
                proxies: vec![],
                credentials: vec![],
            }
        }
        assert_eq!(
            payload_hash(&payload(2)).unwrap(),
            payload_hash(&payload(2)).unwrap()
        );
        assert_ne!(
            payload_hash(&payload(1)).unwrap(),
            payload_hash(&payload(2)).unwrap()
        );
    }

    #[test]
    fn backup_file_round_trips() {
        let mut p = Profile::new("id-1", "Work", "chromium", "/tmp/mbm/id-1");
        p.notes = Some("hello".into());
        p.pinned = true;
        let backup = BackupFile {
            format_version: FORMAT_VERSION,
            exported_at: 1234567890,
            profiles: vec![p],
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
        assert_eq!(parsed.proxies[0].label, "lab");
        assert_eq!(parsed.credentials[0].password.as_deref(), Some("pw"));
        assert_eq!(parsed.credentials[0].seed_phrase.as_deref(), Some("word1 word2"));
    }

    #[test]
    fn deserializes_minimal_backup_file() {
        let json = r#"{"format_version":1,"exported_at":42}"#;
        let parsed: BackupFile = serde_json::from_str(json).expect("defaults should fill the rest");
        assert!(parsed.profiles.is_empty());
        assert!(parsed.proxies.is_empty());
    }

    #[test]
    fn profile_deserializes_without_optional_fields() {
        // Pre-extra-args / pre-pinned era profile JSON must still parse.
        let json = r#"{
            "id": "x", "name": "Old", "browserType": "chromium",
            "userDataDir": "/tmp/mbm/x", "status": "stopped",
            "createdAt": 1, "updatedAt": 1
        }"#;
        let p: Profile = serde_json::from_str(json).unwrap();
        assert!(!p.pinned);
        assert!(!p.restart_on_crash);
        assert!(p.extra_args.is_none());
        assert!(p.stop_timeout_secs.is_none());
        assert!(p.groups.is_empty());
    }
}
