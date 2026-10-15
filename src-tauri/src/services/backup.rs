//! Backup import/export engine (moved from `commands/backup.rs` — A3).
//!
//! Pure functions over `&Connection` + paths: no Tauri state, so the import
//! conflict rules are unit-testable against an in-memory database.

mod export;
mod import;

pub use export::{run_snapshot_if_due, write_backup_file, write_encrypted_backup_file};
pub use import::import_backup_file;

use crate::commands::profile::list_profiles;
use crate::error::AppResult;
use crate::models::profile::Profile;
use crate::models::proxy::Proxy;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;
    use super::import::apply_import;
    use crate::db;
    use rusqlite::params;
    use std::path::Path;

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
    }
}
