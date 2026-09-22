use crate::browser::launcher;
use crate::error::{AppError, AppResult};
use crate::models::group::Group;
use crate::models::profile::{CreateProfileInput, Profile, UpdateProfileInput};
use crate::AppState;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use tauri::{AppHandle, State};

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get("id")?,
        name: row.get("name")?,
        browser_type: row.get("browser_type")?,
        user_data_dir: row.get("user_data_dir")?,
        proxy_id: row.get("proxy_id")?,
        notes: row.get("notes")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        last_used_at: row.get("last_used_at")?,
        pinned: row.get::<_, i64>("pinned")? != 0,
        groups: Vec::new(),
    })
}

/// Attaches group tags to each profile in the list (single extra query).
fn attach_groups(conn: &Connection, profiles: &mut [Profile]) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT pg.profile_id, g.id, g.name, g.color
         FROM profile_groups pg JOIN groups g ON g.id = pg.group_id
         ORDER BY g.name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Group {
                    id: r.get(1)?,
                    name: r.get(2)?,
                    color: r.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Index by profile id once — O(P + G) instead of O(P × G).
    let mut by_profile: HashMap<String, Vec<Group>> = HashMap::new();
    for (profile_id, group) in rows {
        by_profile.entry(profile_id).or_default().push(group);
    }
    for p in profiles.iter_mut() {
        if let Some(groups) = by_profile.remove(&p.id) {
            p.groups = groups;
        }
    }
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> AppResult<Vec<Profile>> {
    let mut stmt =
        conn.prepare("SELECT * FROM profiles ORDER BY pinned DESC, name COLLATE NOCASE ASC")?;
    let mut profiles = stmt
        .query_map([], row_to_profile)?
        .collect::<Result<Vec<_>, _>>()?;
    attach_groups(conn, &mut profiles)?;
    Ok(profiles)
}

pub fn get_profile_by_id(conn: &Connection, id: &str) -> AppResult<Profile> {
    conn.query_row("SELECT * FROM profiles WHERE id = ?1", params![id], row_to_profile)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("Profile {id} not found"))
            }
            other => other.into(),
        })
}

fn validate_name(conn: &Connection, name: &str, exclude_id: Option<&str>) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Profile name cannot be empty".into()));
    }

    let exists: bool = conn.query_row(
        // Case-insensitive to match CLI resolution (LOWER(name)) and the
        // UI ordering — "Work" and "work" must not coexist.
        "SELECT COUNT(*) > 0 FROM profiles WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id != ?2)",
        params![name, exclude_id],
        |r| r.get(0),
    )?;

    if exists {
        return Err(AppError::Validation(format!(
            "A profile named '{name}' already exists"
        )));
    }

    Ok(())
}

pub fn insert_profile(conn: &Connection, profile: &Profile) -> AppResult<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, browser_type, user_data_dir, proxy_id, notes, status, created_at, updated_at, last_used_at, pinned)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            profile.id,
            profile.name,
            profile.browser_type,
            profile.user_data_dir,
            profile.proxy_id,
            profile.notes,
            profile.status,
            profile.created_at,
            profile.updated_at,
            profile.last_used_at,
            profile.pinned as i64,
        ],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> AppResult<Vec<Profile>> {
    let conn = db_lock(&state)?;
    list_profiles(&conn)
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> AppResult<Profile> {
    let conn = db_lock(&state)?;
    validate_name(&conn, &input.name, None)?;

    let id = uuid::Uuid::new_v4().to_string();
    let user_data_dir = state
        .profiles_root
        .join(&id)
        .to_string_lossy()
        .to_string();

    launcher::ensure_user_data_dir(std::path::Path::new(&user_data_dir))?;

    let now = chrono::Utc::now().timestamp();
    let profile = Profile {
        id: id.clone(),
        name: input.name.trim().to_string(),
        browser_type: input.browser_type,
        user_data_dir,
        proxy_id: input.proxy_id,
        notes: input.notes,
        status: "stopped".to_string(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        pinned: false,
        groups: Vec::new(),
    };

    insert_profile(&conn, &profile)?;
    drop(conn); // release the DB lock before the tray rebuilds itself
    crate::tray::rebuild_tray(&app);
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: UpdateProfileInput,
) -> AppResult<Profile> {
    let conn = db_lock(&state)?;
    let existing = get_profile_by_id(&conn, &id)?;

    if existing.status == "running" {
        return Err(AppError::Validation(
            "Cannot edit a profile while its browser is running".into(),
        ));
    }

    if let Some(name) = &input.name {
        validate_name(&conn, name, Some(&id))?;
    }
    if let Some(Some(proxy_id)) = &input.proxy_id {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM proxies WHERE id = ?1",
            params![proxy_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation(format!("Proxy {proxy_id} not found")));
        }
    }

    let name = input.name.unwrap_or(existing.name).trim().to_string();
    let browser_type = input.browser_type.unwrap_or(existing.browser_type);
    // Absent = keep current; explicit null = clear (unassign proxy / clear notes).
    let proxy_id = match input.proxy_id {
        Some(v) => v,
        None => existing.proxy_id,
    };
    let notes = match input.notes {
        Some(v) => v,
        None => existing.notes,
    };

    conn.execute(
        "UPDATE profiles SET name = ?1, browser_type = ?2, proxy_id = ?3, notes = ?4, updated_at = ?5 WHERE id = ?6",
        params![name, browser_type, proxy_id, notes, chrono::Utc::now().timestamp(), id],
    )?;
    let updated = get_profile_by_id(&conn, &id)?;
    drop(conn); // release the DB lock before the tray rebuilds itself

    crate::tray::rebuild_tray(&app); // keep the tray menu name in sync
    Ok(updated)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = db_lock(&state)?;
    let profile = get_profile_by_id(&conn, &id)?;

    // A running browser must be stopped before its profile can be deleted.
    if profile.status == "running" {
        return Err(AppError::Validation(
            "Stop the browser before deleting this profile".into(),
        ));
    }

    // Wipe the profile's data directory BEFORE removing the DB row: if the
    // directory removal fails, the row stays and the user can retry — instead
    // of an orphaned data folder with no way to delete it from the UI.
    let dir = std::path::Path::new(&profile.user_data_dir);
    if dir.starts_with(&state.profiles_root) {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.into());
            }
        }
    }

    // Purge credentials (DB rows + keychain secrets) together with the data
    // dir so no secret outlives its profile.
    crate::commands::credentials::purge_profile_credentials(&conn, &id)?;

    conn.execute("DELETE FROM profiles WHERE id = ?1", params![id])?;
    drop(conn); // release the DB lock before the tray rebuilds itself

    crate::tray::rebuild_tray(&app);
    Ok(())
}

#[tauri::command]
pub fn duplicate_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Profile> {
    let conn = db_lock(&state)?;
    let source = get_profile_by_id(&conn, &id)?;

    // Cold duplicate: same metadata, brand-new empty data dir.
    let new_id = uuid::Uuid::new_v4().to_string();
    let base_name = format!("{} (copy)", source.name);

    // Guarantee uniqueness even after repeated duplicates (case-insensitive,
    // matching validate_name and CLI resolution).
    let mut name = base_name.clone();
    let mut suffix = 2;
    loop {
        let taken: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM profiles WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |r| r.get(0),
        )?;
        if !taken {
            break;
        }
        name = format!("{base_name} {suffix}");
        suffix += 1;
    }

    let user_data_dir = state
        .profiles_root
        .join(&new_id)
        .to_string_lossy()
        .to_string();
    launcher::ensure_user_data_dir(std::path::Path::new(&user_data_dir))?;

    let now = chrono::Utc::now().timestamp();
    let profile = Profile {
        id: new_id,
        name,
        browser_type: source.browser_type,
        user_data_dir,
        proxy_id: source.proxy_id,
        notes: source.notes,
        status: "stopped".to_string(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        pinned: false,
        groups: Vec::new(),
    };

    insert_profile(&conn, &profile)?;
    // Duplicate carries the account credentials over (secrets re-keyed in the
    // keychain) — a copied farming profile starts with the same accounts.
    crate::commands::credentials::copy_profile_credentials(&conn, &id, &profile.id)?;
    drop(conn); // release the DB lock before the tray rebuilds itself
    crate::tray::rebuild_tray(&app);
    Ok(profile)
}

#[tauri::command]
pub fn toggle_pin(state: State<'_, AppState>, id: String) -> AppResult<bool> {
    let conn = db_lock(&state)?;
    let profile = get_profile_by_id(&conn, &id)?;
    let new_pinned = !profile.pinned;
    conn.execute(
        "UPDATE profiles SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_pinned as i64, chrono::Utc::now().timestamp(), id],
    )?;
    Ok(new_pinned)
}

pub(crate) fn poisoned() -> AppError {
    AppError::Internal("Database state poisoned".into())
}

/// Locks the shared DB connection, mapping a poisoned mutex to a clean error.
/// Replaces the copy-pasted `state.db.lock().map_err(|_| poisoned())?` pattern.
pub(crate) fn db_lock(state: &AppState) -> AppResult<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    state.db.lock().map_err(|_| poisoned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::Path;

    fn mem_conn() -> Connection {
        db::init(Path::new(":memory:")).expect("in-memory db")
    }

    fn test_profile(name: &str, pinned: bool) -> Profile {
        Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            browser_type: "chromium".to_string(),
            user_data_dir: format!("/tmp/mbm-test/{name}"),
            proxy_id: None,
            notes: None,
            status: "stopped".to_string(),
            created_at: 1,
            updated_at: 1,
            last_used_at: None,
            pinned,
            groups: vec![],
        }
    }

    #[test]
    fn rejects_duplicate_name_case_insensitively() {
        let conn = mem_conn();
        insert_profile(&conn, &test_profile("Work", false)).unwrap();
        assert!(validate_name(&conn, "work", None).is_err());
        assert!(validate_name(&conn, "WORK", None).is_err());
        assert!(validate_name(&conn, "Personal", None).is_ok());
    }

    #[test]
    fn update_may_keep_own_name() {
        let conn = mem_conn();
        let p = test_profile("Work", false);
        insert_profile(&conn, &p).unwrap();
        // Renaming to its own name (e.g. edit form resubmitted) is allowed.
        assert!(validate_name(&conn, "work", Some(&p.id)).is_ok());
    }

    #[test]
    fn rejects_empty_and_whitespace_names() {
        let conn = mem_conn();
        assert!(validate_name(&conn, "", None).is_err());
        assert!(validate_name(&conn, "   ", None).is_err());
    }

    #[test]
    fn list_orders_pinned_first_then_name() {
        let conn = mem_conn();
        insert_profile(&conn, &test_profile("zeta", false)).unwrap();
        insert_profile(&conn, &test_profile("alpha", true)).unwrap();
        insert_profile(&conn, &test_profile("Beta", true)).unwrap();

        let names: Vec<String> = list_profiles(&conn)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["alpha", "Beta", "zeta"]);
    }
}
