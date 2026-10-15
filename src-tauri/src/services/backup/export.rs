//! Backup export: plaintext + encrypted files, and the automatic snapshot.

use super::*;
use crate::services::crypto;
use std::path::Path;

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
    crypto::validate_passphrase(passphrase)?;
    let payload = build_payload(conn)?;
    let json = serialize_backup(&payload, chrono::Utc::now().timestamp())?;
    let encrypted = crypto::encrypt(json.as_bytes(), passphrase)?;
    std::fs::write(path, encrypted)?;
    Ok(())
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

