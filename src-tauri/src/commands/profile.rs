use crate::browser::{detector, launcher};
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
        extra_args: row.get("extra_args")?,
        restart_on_crash: row.get::<_, i64>("restart_on_crash")? != 0,
        stop_timeout_secs: row.get("stop_timeout_secs")?,
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
                AppError::not_found(format!("Profile {id} not found"))
            }
            other => other.into(),
        })
}

fn validate_name(conn: &Connection, name: &str, exclude_id: Option<&str>) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::validation("Profile name cannot be empty"));
    }

    let exists: bool = conn.query_row(
        // Case-insensitive to match CLI resolution (LOWER(name)) and the
        // UI ordering — "Work" and "work" must not coexist.
        "SELECT COUNT(*) > 0 FROM profiles WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id != ?2)",
        params![name, exclude_id],
        |r| r.get(0),
    )?;

    if exists {
        return Err(AppError::validation(format!(
            "A profile named '{name}' already exists"
        )));
    }

    Ok(())
}

/// Validates fields shared by create and update (audit L6): browser_type must
/// be one this build can actually detect/launch, and extra args must parse
/// and not override MBM's own flags.
fn validate_launch_fields(browser_type: &str, extra_args: Option<&str>) -> AppResult<()> {
    if !detector::is_known_browser_type(browser_type) {
        return Err(AppError::validation(format!(
            "Unknown browser type '{browser_type}'. Supported: {}",
            detector::known_browser_types().join(", ")
        )));
    }
    launcher::validate_extra_args(extra_args.unwrap_or(""))?;
    Ok(())
}

/// Validates a per-profile stop timeout if present (1..=60 seconds).
fn validate_stop_timeout(secs: Option<i64>) -> AppResult<()> {
    if let Some(s) = secs {
        if !(1..=60).contains(&s) {
            return Err(AppError::validation(
                "Stop timeout must be between 1 and 60 seconds",
            ));
        }
    }
    Ok(())
}

pub fn insert_profile(conn: &Connection, profile: &Profile) -> AppResult<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, browser_type, user_data_dir, proxy_id, notes, status, created_at, updated_at, last_used_at, pinned, extra_args, restart_on_crash, stop_timeout_secs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
            profile.extra_args,
            profile.restart_on_crash as i64,
            profile.stop_timeout_secs,
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
    validate_launch_fields(&input.browser_type, input.extra_args.as_deref())?;
    validate_stop_timeout(input.stop_timeout_secs)?;

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
        extra_args: input.extra_args,
        restart_on_crash: input.restart_on_crash,
        stop_timeout_secs: input.stop_timeout_secs,
        groups: Vec::new(),
    };

    insert_profile(&conn, &profile)?;
    drop(conn); // release the DB lock before notifications fan out
    crate::sync::after_profile_change(&app, &profile.id);
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
        return Err(AppError::validation(
            "Cannot edit a profile while its browser is running",
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
            return Err(AppError::validation(format!("Proxy {proxy_id} not found")));
        }
    }

    let name = input.name.unwrap_or(existing.name).trim().to_string();
    let browser_type = input.browser_type.unwrap_or(existing.browser_type.clone());
    // Absent = keep current; explicit null = clear (unassign proxy / clear notes).
    let proxy_id = match input.proxy_id {
        Some(v) => v,
        None => existing.proxy_id,
    };
    let notes = match input.notes {
        Some(v) => v,
        None => existing.notes,
    };
    let extra_args = match input.extra_args {
        Some(v) => v,
        None => existing.extra_args.clone(),
    };
    let restart_on_crash = input.restart_on_crash.unwrap_or(existing.restart_on_crash);
    let stop_timeout_secs = match input.stop_timeout_secs {
        Some(v) => v,
        None => existing.stop_timeout_secs,
    };

    validate_launch_fields(&browser_type, extra_args.as_deref())?;
    validate_stop_timeout(stop_timeout_secs)?;

    conn.execute(
        "UPDATE profiles SET name = ?1, browser_type = ?2, proxy_id = ?3, notes = ?4, extra_args = ?5, restart_on_crash = ?6, stop_timeout_secs = ?7, updated_at = ?8 WHERE id = ?9",
        params![
            name,
            browser_type,
            proxy_id,
            notes,
            extra_args,
            restart_on_crash as i64,
            stop_timeout_secs,
            chrono::Utc::now().timestamp(),
            id
        ],
    )?;
    let updated = get_profile_by_id(&conn, &id)?;
    drop(conn); // release the DB lock before notifications fan out

    crate::sync::after_profile_change(&app, &id);
    Ok(updated)
}

/// What the UI needs to offer "Undo" right after a delete.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedProfileInfo {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<DeletedProfileInfo> {
    let conn = db_lock(&state)?;
    let info = delete_profile_inner(&conn, &state.profiles_root, &id)?;
    drop(conn); // release the DB lock before notifications fan out

    crate::sync::after_profile_change(&app, &id);
    Ok(info)
}

/// Business logic of a delete (A3): move the data dir to the trash, snapshot
/// profile + credentials for undo, purge secrets, remove the row. Unit-tested
/// via `services::trash`.
pub(crate) fn delete_profile_inner(
    conn: &Connection,
    profiles_root: &std::path::Path,
    id: &str,
) -> AppResult<DeletedProfileInfo> {
    let profile = get_profile_by_id(conn, id)?;

    // A running browser must be stopped before its profile can be deleted.
    if profile.status == "running" {
        return Err(AppError::validation(
            "Stop the browser before deleting this profile",
        ));
    }

    // Snapshot credentials BEFORE purge so undo can restore them.
    let credentials = crate::commands::credentials::list_all_credentials(conn)?
        .into_iter()
        .filter(|c| c.profile_id == id)
        .collect();

    let info =
        crate::services::trash::move_profile_to_trash(conn, profiles_root, &profile, credentials)?;

    // Purge credentials (DB rows + legacy keychain secrets) together with the
    // data dir so no secret outlives its profile. The trash snapshot keeps a
    // copy for undo — the trash dir itself is 0700 under the private data root.
    crate::commands::credentials::purge_profile_credentials(conn, id)?;

    conn.execute("DELETE FROM profiles WHERE id = ?1", params![id])?;
    Ok(DeletedProfileInfo { id: info.id, name: info.name })
}

#[tauri::command]
pub fn restore_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Profile> {
    let conn = db_lock(&state)?;
    let profile =
        crate::services::trash::restore_profile_from_trash(&conn, &state.profiles_root, &id)?;
    drop(conn);
    crate::sync::after_profile_change(&app, &profile.id);
    Ok(profile)
}

#[tauri::command]
pub fn get_trash_count(state: State<'_, AppState>) -> AppResult<i64> {
    let conn = db_lock(&state)?;
    crate::services::trash::trash_count(&conn)
}

#[tauri::command]
pub fn empty_trash(app: AppHandle, state: State<'_, AppState>) -> AppResult<usize> {
    let conn = db_lock(&state)?;
    let removed = crate::services::trash::empty_trash(&conn)?;
    drop(conn);
    if removed > 0 {
        crate::tray::schedule_rebuild(&app);
    }
    Ok(removed)
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
        browser_type: source.browser_type.clone(),
        user_data_dir,
        proxy_id: source.proxy_id.clone(),
        notes: source.notes.clone(),
        status: "stopped".to_string(),
        created_at: now,
        updated_at: now,
        last_used_at: None,
        pinned: false,
        extra_args: source.extra_args.clone(),
        restart_on_crash: source.restart_on_crash,
        stop_timeout_secs: source.stop_timeout_secs,
        groups: Vec::new(),
    };

    insert_profile(&conn, &profile)?;
    // Duplicate carries the account credentials over (secrets re-keyed in the
    // keychain) — a copied farming profile starts with the same accounts.
    crate::commands::credentials::copy_profile_credentials(&conn, &id, &profile.id)?;
    drop(conn); // release the DB lock before notifications fan out
    crate::sync::after_profile_change(&app, &profile.id);
    Ok(profile)
}

#[tauri::command]
pub fn toggle_pin(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<bool> {
    let conn = db_lock(&state)?;
    let profile = get_profile_by_id(&conn, &id)?;
    let new_pinned = !profile.pinned;
    conn.execute(
        "UPDATE profiles SET pinned = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_pinned as i64, chrono::Utc::now().timestamp(), id],
    )?;
    drop(conn);
    // Pin order shows in the tray menu too — keep it in sync (A2).
    crate::sync::after_profile_change(&app, &id);
    Ok(new_pinned)
}

pub(crate) fn poisoned() -> AppError {
    AppError::internal("Database state poisoned")
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
        let mut p = Profile::new(
            uuid::Uuid::new_v4().to_string(),
            name,
            "chromium",
            format!("/tmp/mbm-test/{name}"),
        );
        p.created_at = 1;
        p.updated_at = 1;
        p.pinned = pinned;
        p
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

    #[test]
    fn new_fields_round_trip_through_the_db() {
        let conn = mem_conn();
        let mut p = test_profile("Tuned", false);
        p.extra_args = Some("--disable-gpu".into());
        p.restart_on_crash = true;
        p.stop_timeout_secs = Some(10);
        insert_profile(&conn, &p).unwrap();

        let loaded = get_profile_by_id(&conn, &p.id).unwrap();
        assert_eq!(loaded.extra_args.as_deref(), Some("--disable-gpu"));
        assert!(loaded.restart_on_crash);
        assert_eq!(loaded.stop_timeout_secs, Some(10));
    }

    #[test]
    fn delete_moves_data_to_trash_and_deletes_row() {
        let conn = mem_conn();
        let root = std::env::temp_dir().join(format!("mbm-del-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("profiles")).unwrap();

        let mut p = Profile::new("del-1", "Doomed", "chromium", "");
        p.user_data_dir = root.join("profiles").join("del-1").to_string_lossy().to_string();
        p.created_at = 1;
        p.updated_at = 1;
        insert_profile(&conn, &p).unwrap();
        std::fs::create_dir_all(&p.user_data_dir).unwrap();

        let info = delete_profile_inner(&conn, &root.join("profiles"), "del-1").unwrap();
        assert_eq!(info.name, "Doomed");
        assert!(!std::path::Path::new(&p.user_data_dir).exists());
        assert!(crate::services::trash::trash_count(&conn).unwrap() >= 1);

        // Restore again so trash doesn't leak between tests.
        crate::services::trash::empty_trash(&conn).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_launch_fields_rejects_unknown_browser() {
        assert!(validate_launch_fields("chrome", None).is_ok());
        assert!(validate_launch_fields("nobody-browser", None).is_err());
        assert!(validate_launch_fields("chromium", Some("--user-data-dir=/tmp")).is_err());
    }

    #[test]
    fn stop_timeout_bounds_are_enforced() {
        assert!(validate_stop_timeout(None).is_ok());
        assert!(validate_stop_timeout(Some(1)).is_ok());
        assert!(validate_stop_timeout(Some(60)).is_ok());
        assert!(validate_stop_timeout(Some(0)).is_err());
        assert!(validate_stop_timeout(Some(61)).is_err());
    }
}
