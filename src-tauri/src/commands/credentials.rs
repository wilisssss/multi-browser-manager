use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

const KEYRING_SERVICE: &str = "multi-browser-manager";

/// Legacy keychain entry (pre-DB-storage secrets). Only used to migrate old
/// installations into the database and to clean up leftovers.
fn keyring_entry(credential_id: &str, field: &str) -> AppResult<keyring::Entry> {
    Ok(keyring::Entry::new(
        KEYRING_SERVICE,
        &format!("mbm-cred-{credential_id}-{field}"),
    )?)
}

fn get_secret(credential_id: &str, field: &str) -> AppResult<Option<String>> {
    match keyring_entry(credential_id, field)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn delete_secret(credential_id: &str, field: &str) {
    // Ignore NoEntry — deleting twice must stay idempotent.
    if let Ok(entry) = keyring_entry(credential_id, field) {
        let _ = entry.delete_credential();
    }
}

/// One-time migration: secrets used to live in the OS keychain; move any
/// leftover values into the database and drop the keychain entries.
/// Idempotent — safe to call on every startup.
pub fn migrate_keychain_secrets_to_db(conn: &Connection) {
    let ids: Vec<String> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id FROM credentials WHERE password IS NULL OR seed_phrase IS NULL",
        ) else {
            return;
        };
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .and_then(|m| m.collect::<Result<Vec<_>, _>>());
        match rows {
            Ok(r) => r,
            Err(_) => return,
        }
    };

    for id in ids {
        let (db_pw, db_seed): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT password, seed_phrase FROM credentials WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((None, None));

        if db_pw.is_none() {
            if let Ok(Some(v)) = get_secret(&id, "password") {
                let _ = conn.execute(
                    "UPDATE credentials SET password = ?1 WHERE id = ?2",
                    params![v, id],
                );
            }
        }
        if db_seed.is_none() {
            if let Ok(Some(v)) = get_secret(&id, "seed") {
                let _ = conn.execute(
                    "UPDATE credentials SET seed_phrase = ?1 WHERE id = ?2",
                    params![v, id],
                );
            }
        }
        // The keychain copy is no longer the source of truth — remove it.
        delete_secret(&id, "password");
        delete_secret(&id, "seed");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Credential {
    pub id: String,
    pub profile_id: String,
    pub platform: String,
    pub label: String,
    pub username: Option<String>,
    /// Secrets live in the database (local-only usage; data dir is 0700) and
    /// are included in exports for device migration.
    pub password: Option<String>,
    pub seed_phrase: Option<String>,
    /// Public EVM address.
    pub evm_address: Option<String>,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCredentialInput {
    pub platform: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub seed_phrase: Option<String>,
    #[serde(default)]
    pub evm_address: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCredentialInput {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<Option<String>>,
    #[serde(default)]
    pub seed_phrase: Option<Option<String>>,
    #[serde(default)]
    pub evm_address: Option<Option<String>>,
    #[serde(default)]
    pub notes: Option<Option<String>>,
}

/// Deserializes JSON `null` as `Some(None)` while an absent field stays `None`,
/// so "clear this secret" can be distinguished from "leave it unchanged".
// Only exercised from tests right now (see double_option_distinguishes_null_from_absent);
// keep it available for future DTOs without tripping the dead_code lint.
#[cfg_attr(not(test), allow(dead_code))]
fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

fn row_to_credential(row: &rusqlite::Row<'_>) -> rusqlite::Result<Credential> {
    Ok(Credential {
        id: row.get("id")?,
        profile_id: row.get("profile_id")?,
        platform: row.get("platform")?,
        label: row.get("label")?,
        username: row.get("username")?,
        password: row.get("password")?,
        seed_phrase: row.get("seed_phrase")?,
        evm_address: row.get("evm_address")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

const CREDENTIAL_COLUMNS: &str =
    "id, profile_id, platform, label, username, password, seed_phrase, evm_address, notes, created_at, updated_at";

fn get_credential(conn: &Connection, id: &str) -> AppResult<Credential> {
    conn.query_row(
        &format!("SELECT {CREDENTIAL_COLUMNS} FROM credentials WHERE id = ?1"),
        params![id],
        row_to_credential,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Credential {id} not found"))
        }
        other => other.into(),
    })
}

pub(crate) fn list_profile_credentials(
    conn: &Connection,
    profile_id: &str,
) -> AppResult<Vec<Credential>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CREDENTIAL_COLUMNS} FROM credentials WHERE profile_id = ?1 ORDER BY created_at ASC"
    ))?;
    let rows = stmt
        .query_map(params![profile_id], row_to_credential)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn list_all_credentials(conn: &Connection) -> AppResult<Vec<Credential>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CREDENTIAL_COLUMNS} FROM credentials ORDER BY created_at ASC"
    ))?;
    let rows = stmt
        .query_map([], row_to_credential)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn get_credentials(
    state: State<'_, AppState>,
    profile_id: String,
) -> AppResult<Vec<Credential>> {
    let conn = db_lock(&state)?;
    list_profile_credentials(&conn, &profile_id)
}

#[tauri::command]
pub fn create_credential(
    state: State<'_, AppState>,
    profile_id: String,
    input: CreateCredentialInput,
) -> AppResult<Credential> {
    let platform = input.platform.trim().to_string();
    if platform.is_empty() {
        return Err(AppError::Validation("Platform is required".into()));
    }
    // Label falls back to the platform name (the UI already shows icons).
    let label = {
        let l = input.label.trim();
        if l.is_empty() { platform.clone() } else { l.to_string() }
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    let conn = db_lock(&state)?;
    // Existence check gives a clean error instead of an FK violation.
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM profiles WHERE id = ?1",
        params![profile_id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound(format!("Profile {profile_id} not found")));
    }

    conn.execute(
        "INSERT INTO credentials (id, profile_id, platform, label, username, password, seed_phrase, evm_address, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            id,
            profile_id,
            platform,
            label,
            input.username.as_deref().map(str::trim).filter(|u| !u.is_empty()),
            input.password.as_deref().filter(|p| !p.is_empty()),
            input.seed_phrase.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.evm_address.as_deref().map(str::trim).filter(|a| !a.is_empty()),
            input.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()),
            now
        ],
    )?;

    get_credential(&conn, &id)
}

#[tauri::command]
pub fn update_credential(
    state: State<'_, AppState>,
    id: String,
    input: UpdateCredentialInput,
) -> AppResult<Credential> {
    let conn = db_lock(&state)?;
    let existing = get_credential(&conn, &id)?;

    let label = input
        .label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or(existing.label);
    let username = input
        .username
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    let evm_address = input
        .evm_address
        .unwrap_or(existing.evm_address)
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty());
    let notes = input
        .notes
        .unwrap_or(existing.notes)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());

    // Secrets: absent = keep, null/empty = clear, value = replace.
    let password = match input.password {
        Some(v) => v.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()),
        None => existing.password,
    };
    let seed_phrase = match input.seed_phrase {
        Some(v) => v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        None => existing.seed_phrase,
    };

    conn.execute(
        "UPDATE credentials SET label = ?1, username = ?2, password = ?3, seed_phrase = ?4, evm_address = ?5, notes = ?6, updated_at = ?7 WHERE id = ?8",
        params![label, username, password, seed_phrase, evm_address, notes, chrono::Utc::now().timestamp(), id],
    )?;

    get_credential(&conn, &id)
}

#[tauri::command]
pub fn delete_credential(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = db_lock(&state)?;
    let _ = get_credential(&conn, &id)?;
    conn.execute("DELETE FROM credentials WHERE id = ?1", params![id])?;
    // Clean up any pre-migration keychain leftovers.
    delete_secret(&id, "password");
    delete_secret(&id, "seed");
    Ok(())
}

/// Purges every credential of a profile (DB rows + keychain leftovers).
/// Called before a profile row is deleted.
pub fn purge_profile_credentials(conn: &Connection, profile_id: &str) -> AppResult<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM credentials WHERE profile_id = ?1")?;
        let rows = stmt
            .query_map(params![profile_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for id in ids {
        conn.execute("DELETE FROM credentials WHERE id = ?1", params![id])?;
        delete_secret(&id, "password");
        delete_secret(&id, "seed");
    }
    Ok(())
}

/// Copies credentials (including secrets) to a duplicated profile.
pub fn copy_profile_credentials(
    conn: &Connection,
    source_profile_id: &str,
    target_profile_id: &str,
) -> AppResult<()> {
    let sources = list_profile_credentials(conn, source_profile_id)?;
    let now = chrono::Utc::now().timestamp();
    for src in sources {
        let new_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO credentials (id, profile_id, platform, label, username, password, seed_phrase, evm_address, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                new_id,
                target_profile_id,
                src.platform,
                src.label,
                src.username,
                src.password,
                src.seed_phrase,
                src.evm_address,
                src.notes,
                now
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::Path;

    fn conn_with_profile() -> Connection {
        let conn = db::init(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES ('p1', 'Work', 'chromium', '/tmp/mbm/p1', 'stopped', 1, 1)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn double_option_distinguishes_null_from_absent() {
        #[derive(Deserialize)]
        struct In {
            #[serde(default, deserialize_with = "double_option")]
            password: Option<Option<String>>,
        }
        // Absent → keep (None).
        let i: In = serde_json::from_str("{}").unwrap();
        assert!(i.password.is_none());
        // null → clear (Some(None)).
        let i: In = serde_json::from_str(r#"{"password": null}"#).unwrap();
        assert_eq!(i.password, Some(None));
        // Value → replace.
        let i: In = serde_json::from_str(r#"{"password": "s3cret"}"#).unwrap();
        assert_eq!(i.password, Some(Some("s3cret".into())));
    }

    #[test]
    fn secrets_round_trip_in_database() {
        let conn = conn_with_profile();
        conn.execute(
            "INSERT INTO credentials (id, profile_id, platform, label, username, password, seed_phrase, evm_address, created_at, updated_at)
             VALUES ('c1', 'p1', 'wallet', 'Main', 'acct', 'hunter2', 'word1 word2 word3', '0xabc', 1, 1)",
            [],
        )
        .unwrap();

        let creds = list_profile_credentials(&conn, "p1").unwrap();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].password.as_deref(), Some("hunter2"));
        assert_eq!(creds[0].seed_phrase.as_deref(), Some("word1 word2 word3"));
        assert_eq!(creds[0].evm_address.as_deref(), Some("0xabc"));
    }

    #[test]
    fn purge_and_copy_credentials() {
        let conn = conn_with_profile();
        conn.execute(
            "INSERT INTO credentials (id, profile_id, platform, label, username, password, created_at, updated_at)
             VALUES ('c1', 'p1', 'x', 'X', 'user', 'pw', 1, 1)",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES ('p2', 'Work2', 'chromium', '/tmp/mbm/p2', 'stopped', 1, 1)",
            [],
        )
        .unwrap();
        copy_profile_credentials(&conn, "p1", "p2").unwrap();
        let copies = list_profile_credentials(&conn, "p2").unwrap();
        assert_eq!(copies.len(), 1);
        // The secret travels with the copy.
        assert_eq!(copies[0].password.as_deref(), Some("pw"));

        purge_profile_credentials(&conn, "p1").unwrap();
        assert!(list_profile_credentials(&conn, "p1").unwrap().is_empty());
        assert_eq!(list_profile_credentials(&conn, "p2").unwrap().len(), 1);
    }
}
