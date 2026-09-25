//! Trash / undo-delete engine (feature recommendation 6).
//!
//! Deleting a profile used to be an immediate, unrecoverable wipe of the
//! user-data-dir. Now the data directory is *moved* (same filesystem → cheap
//! rename) to `<data>/trash/<profile-id>` and a full snapshot of the profile
//! row + its credentials is parked in the `deleted_profiles` table, so an
//! accidental delete can be undone. Snapshots older than `TRASH_RETENTION_
//! DAYS` are purged at startup.

use crate::browser::launcher;
use crate::commands::credentials::Credential;
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How long a trashed profile stays restorable.
pub const TRASH_RETENTION_DAYS: i64 = 30;

/// What the UI needs to offer "Undo" after a delete.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashInfo {
    pub id: String,
    pub name: String,
}

/// Full snapshot parked in `deleted_profiles.payload`.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashedProfile {
    pub profile: Profile,
    pub credentials: Vec<Credential>,
}

pub fn trash_root(profiles_root: &Path) -> PathBuf {
    profiles_root
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .join("trash")
}

/// Moves the profile's data dir to the trash and parks a restorable snapshot
/// in `deleted_profiles`. The live `profiles` row is deleted here — callers
/// must still purge keychain secrets (best-effort, legacy entries only).
pub fn move_profile_to_trash(
    conn: &Connection,
    profiles_root: &Path,
    profile: &Profile,
    credentials: Vec<Credential>,
) -> AppResult<TrashInfo> {
    let trash_dir = trash_root(profiles_root).join(&profile.id);
    let dir = Path::new(&profile.user_data_dir);
    // Only MBM-managed dirs are moved; foreign paths (or missing dirs) are
    // simply not archived — the snapshot row still allows restoring metadata.
    if dir.starts_with(profiles_root) && dir.exists() {
        std::fs::create_dir_all(trash_dir.parent().expect("trash root has a parent"))?;
        if trash_dir.exists() {
            // A previous delete of the same id left junk — replace it.
            std::fs::remove_dir_all(&trash_dir)?;
        }
        std::fs::rename(dir, &trash_dir)?;
    }

    let payload = serde_json::to_string(&TrashedProfile {
        profile: profile.clone(),
        credentials,
    })?;
    conn.execute(
        "INSERT OR REPLACE INTO deleted_profiles (id, profile_name, payload, trash_dir, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            profile.id,
            profile.name,
            payload,
            trash_dir.to_string_lossy().to_string(),
            chrono::Utc::now().timestamp()
        ],
    )?;

    Ok(TrashInfo {
        id: profile.id.clone(),
        name: profile.name.clone(),
    })
}

/// Restores a trashed profile: row + credentials + data dir. Fails cleanly
/// when the name is taken again or the payload no longer parses.
pub fn restore_profile_from_trash(
    conn: &Connection,
    _profiles_root: &Path,
    id: &str,
) -> AppResult<Profile> {
    let (payload, trash_dir): (String, String) = conn
        .query_row(
            "SELECT payload, trash_dir FROM deleted_profiles WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::not_found(format!("No trashed profile with id {id}"))
            }
            other => other.into(),
        })?;

    let trashed: TrashedProfile = serde_json::from_str(&payload)
        .map_err(|e| AppError::internal(format!("Corrupted trash snapshot: {e}")))?;
    let mut profile = trashed.profile;

    // The name may have been re-used while the profile was in the trash.
    let name_taken: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM profiles WHERE name = ?1 COLLATE NOCASE",
        params![profile.name],
        |r| r.get(0),
    )?;
    if name_taken {
        return Err(AppError::validation(format!(
            "A profile named '{}' already exists — rename it before restoring",
            profile.name
        )));
    }

    // Move the data dir back if it survived; otherwise start a fresh one so
    // the restored profile is immediately launchable.
    let target = Path::new(&profile.user_data_dir);
    let trash_path = PathBuf::from(&trash_dir);
    if trash_path.exists() {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&trash_path, target)?;
    } else {
        launcher::ensure_user_data_dir(target)?;
    }

    // Reset transient state: a trashed profile is never "running".
    profile.status = "stopped".to_string();
    crate::commands::profile::insert_profile(conn, &profile)?;
    for cred in &trashed.credentials {
        conn.execute(
            "INSERT INTO credentials (id, profile_id, platform, label, username, password, seed_phrase, evm_address, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                uuid::Uuid::new_v4().to_string(),
                profile.id,
                cred.platform,
                cred.label,
                cred.username,
                cred.password,
                cred.seed_phrase,
                cred.evm_address,
                cred.notes,
                cred.created_at,
                cred.updated_at
            ],
        )?;
    }
    conn.execute("DELETE FROM deleted_profiles WHERE id = ?1", params![id])?;
    Ok(profile)
}

/// Deletes trash entries (dirs + snapshot rows) older than the retention
/// window. Called at startup. Returns the number of purged entries.
pub fn purge_expired_trash(conn: &Connection, profiles_root: &Path) -> AppResult<usize> {
    let cutoff = chrono::Utc::now().timestamp() - TRASH_RETENTION_DAYS * 24 * 3600;
    let rows: Vec<(String, String)> = {
        let mut stmt =
            conn.prepare("SELECT id, trash_dir FROM deleted_profiles WHERE deleted_at < ?1")?;
        let mapped = stmt.query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    let root = trash_root(profiles_root);
    for (_, dir) in &rows {
        let dir_path = PathBuf::from(dir);
        // Defense in depth: only ever delete inside the trash root.
        if dir_path.starts_with(&root) {
            let _ = std::fs::remove_dir_all(dir_path);
        }
    }
    for (id, _) in &rows {
        conn.execute("DELETE FROM deleted_profiles WHERE id = ?1", params![id])?;
    }
    Ok(rows.len())
}

/// Removes every trash entry now (Settings → "Empty trash").
pub fn empty_trash(conn: &Connection) -> AppResult<usize> {
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare("SELECT id, trash_dir FROM deleted_profiles")?;
        let mapped = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    for (_, dir) in &rows {
        let _ = std::fs::remove_dir_all(dir);
    }
    for (id, _) in &rows {
        conn.execute("DELETE FROM deleted_profiles WHERE id = ?1", params![id])?;
    }
    Ok(rows.len())
}

/// Number of currently trashed profiles (for the Settings footer).
pub fn trash_count(conn: &Connection) -> AppResult<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM deleted_profiles",
        [],
        |r| r.get(0),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn mem_conn() -> Connection {
        db::init(Path::new(":memory:")).expect("in-memory db")
    }

    fn tmp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("mbm-trash-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("profiles")).unwrap();
        root
    }

    fn sample(dir: &Path) -> Profile {
        let mut p = Profile::new("p1", "Work", "chromium", dir.to_string_lossy().to_string());
        p.notes = Some("keep me".into());
        p
    }

    #[test]
    fn delete_then_restore_round_trip() {
        let conn = mem_conn();
        let root = tmp_root("roundtrip");
        let data_dir = root.join("profiles").join("p1");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::write(data_dir.join("Cookies"), "data").unwrap();

        let info = move_profile_to_trash(&conn, &root.join("profiles"), &sample(&data_dir), vec![])
            .unwrap();
        assert_eq!(info.id, "p1");
        assert!(!data_dir.exists(), "data dir must be moved out of profiles/");
        assert!(trash_root(&root.join("profiles")).join("p1").exists());
        // The profiles row is gone (the caller deletes it); trash row exists.
        assert_eq!(trash_count(&conn).unwrap(), 1);

        let restored = restore_profile_from_trash(&conn, &root.join("profiles"), "p1").unwrap();
        assert_eq!(restored.id, "p1");
        assert_eq!(restored.notes.as_deref(), Some("keep me"));
        assert!(data_dir.join("Cookies").exists(), "data dir must be moved back");
        assert_eq!(trash_count(&conn).unwrap(), 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_refuses_taken_names() {
        let conn = mem_conn();
        let root = tmp_root("taken");
        let profiles_root = root.join("profiles");
        let p = sample(&root.join("profiles").join("p1"));
        std::fs::create_dir_all(&p.user_data_dir).unwrap();

        move_profile_to_trash(&conn, &profiles_root, &p, vec![]).unwrap();
        // A new profile took the name while p1 was in the trash.
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES ('other', 'WORK', 'chromium', '/tmp/other', 'stopped', 1, 1)",
            [],
        )
        .unwrap();

        let err = restore_profile_from_trash(&conn, &profiles_root, "p1").unwrap_err();
        assert!(err.to_string().contains("already exists"));
        // Trash entry is kept so the user can rename/retry later.
        assert_eq!(trash_count(&conn).unwrap(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn expired_trash_is_purged_with_dirs() {
        let conn = mem_conn();
        let root = tmp_root("purge");
        let profiles_root = root.join("profiles");
        let p = sample(&profiles_root.join("p1"));
        std::fs::create_dir_all(&p.user_data_dir).unwrap();

        move_profile_to_trash(&conn, &profiles_root, &p, vec![]).unwrap();
        // Backdate the snapshot beyond the retention window.
        conn.execute(
            "UPDATE deleted_profiles SET deleted_at = ?1 WHERE id = 'p1'",
            params![chrono::Utc::now().timestamp() - (TRASH_RETENTION_DAYS + 1) * 24 * 3600],
        )
        .unwrap();

        let purged = purge_expired_trash(&conn, &profiles_root).unwrap();
        assert_eq!(purged, 1);
        assert_eq!(trash_count(&conn).unwrap(), 0);
        assert!(!trash_root(&profiles_root).join("p1").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_trash_removes_everything() {
        let conn = mem_conn();
        let root = tmp_root("empty");
        let profiles_root = root.join("profiles");
        let mut p = sample(&profiles_root.join("p1"));
        std::fs::create_dir_all(&p.user_data_dir).unwrap();
        p.id = "p2".into();
        move_profile_to_trash(&conn, &profiles_root, &sample(&profiles_root.join("p1")), vec![]).unwrap();
        move_profile_to_trash(&conn, &profiles_root, &p, vec![]).unwrap();

        assert_eq!(empty_trash(&conn).unwrap(), 2);
        assert_eq!(trash_count(&conn).unwrap(), 0);
        assert!(trash_root(&profiles_root).read_dir().unwrap().next().is_none());

        let _ = std::fs::remove_dir_all(&root);
    }
}
