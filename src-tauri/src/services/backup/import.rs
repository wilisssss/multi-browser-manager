//! Backup import: reads a plaintext or encrypted file, validates it, then
//! applies it in a single transaction.

use super::*;
use crate::browser::launcher;
use crate::error::AppError;
use crate::commands::profile::insert_profile;
use crate::commands::proxy::insert_proxy_with_password;
use crate::services::crypto;
use rusqlite::params;
use std::path::Path;

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
    let content = if crypto::is_encrypted(&bytes) {
        let pass = passphrase.ok_or_else(|| {
            AppError::validation(
                "This backup is encrypted — provide the passphrase to import it",
            )
        })?;
        String::from_utf8(crypto::decrypt(&bytes, pass)?)
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
    }

    apply_import(conn, profiles_root, &backup)
}

/// Applies a validated backup inside one transaction: a failure rolls back
/// everything instead of leaving a partial import behind.
pub(super) fn apply_import(
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
            restart_on_crash: profile.restart_on_crash,
            folder_id: None, // folders are local-only; imports land unfiled
            fingerprint: profile.fingerprint.clone(),
            startup_url: profile.startup_url.clone(),
        };
        insert_profile(&tx, &imported)?;

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

