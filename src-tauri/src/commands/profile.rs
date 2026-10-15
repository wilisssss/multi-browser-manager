use crate::browser::{detector, launcher};
use crate::error::{AppError, AppResult};
use crate::models::profile::{CreateProfileInput, Fingerprint, Profile, UpdateProfileInput};
use crate::AppState;
use rusqlite::{params, Connection};
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
        restart_on_crash: row.get::<_, i64>("restart_on_crash")? != 0,
        folder_id: row.get("folder_id")?,
        fingerprint: decode_fingerprint(row.get("fingerprint")?),
        startup_url: row.get("startup_url")?,
    })
}

/// Normalizes a startup URL: blank = none. Only `http`/`https` are accepted,
/// so a profile cannot be pointed at `file:`, `chrome:` or script-like schemes.
fn validate_startup_url(url: Option<&str>) -> AppResult<Option<String>> {
    let Some(raw) = url.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    if raw.len() > 2048 || raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::validation("Startup URL is too long or contains spaces"));
    }
    let parsed = url::Url::parse(raw)
        .map_err(|_| AppError::validation("Startup URL must be a valid http:// or https:// address"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(AppError::validation("Startup URL must start with http:// or https://"));
    }
    Ok(Some(raw.to_string()))
}

/// Stored fingerprint JSON → struct. A corrupt value is ignored (treated as
/// "no overrides") instead of making the whole profile unreadable.
fn decode_fingerprint(raw: Option<String>) -> Option<Fingerprint> {
    raw.and_then(|json| serde_json::from_str::<Fingerprint>(&json).ok())
        .filter(|fp| !fp.is_empty())
}

/// Struct → stored JSON. An empty fingerprint is stored as NULL.
fn encode_fingerprint(fp: &Option<Fingerprint>) -> AppResult<Option<String>> {
    match fp {
        Some(f) if !f.is_empty() => Ok(Some(serde_json::to_string(f)?)),
        _ => Ok(None),
    }
}

/// Rejects values that would produce broken flags or script in the launch.
fn validate_fingerprint(fp: Option<&Fingerprint>) -> AppResult<()> {
    let Some(fp) = fp else { return Ok(()) };
    let bad = |field: &str| AppError::validation(format!("Invalid fingerprint {field}"));
    if let Some(lang) = &fp.language {
        if lang.is_empty()
            || lang.len() > 35
            || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(bad("language (expected a BCP-47 tag like en-US)"));
        }
    }
    if let Some(ua) = &fp.user_agent {
        if ua.len() > 512 || ua.chars().any(|c| c.is_control()) {
            return Err(bad("user agent"));
        }
    }
    if let Some(tz) = &fp.timezone {
        if tz.is_empty()
            || tz.len() > 64
            || !tz.chars().all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c))
        {
            return Err(bad("timezone (expected an IANA name like Europe/Berlin)"));
        }
    }
    if let Some(p) = &fp.platform {
        if p.len() > 64 || p.chars().any(|c| c.is_control()) {
            return Err(bad("platform"));
        }
    }
    if let Some(g) = &fp.geolocation {
        if !(-90.0..=90.0).contains(&g.latitude)
            || !(-180.0..=180.0).contains(&g.longitude)
            || !(1.0..=100_000.0).contains(&g.accuracy)
        {
            return Err(bad("geolocation (latitude −90..90, longitude −180..180)"));
        }
    }
    if let Some(s) = &fp.screen {
        if !(100..=10_000).contains(&s.width) || !(100..=10_000).contains(&s.height) {
            return Err(bad("screen size (each side 100..10000 px)"));
        }
    }
    if let Some(fonts) = &fp.fonts {
        if fonts.len() > 500 || fonts.iter().any(|f| f.is_empty() || f.len() > 100 || f.chars().any(|c| c.is_control())) {
            return Err(bad("fonts"));
        }
    }
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> AppResult<Vec<Profile>> {
    let mut stmt =
        conn.prepare("SELECT * FROM profiles ORDER BY pinned DESC, name COLLATE NOCASE ASC")?;
    let profiles = stmt
        .query_map([], row_to_profile)?
        .collect::<Result<Vec<_>, _>>()?;
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
/// be one this build can actually detect/launch.
fn validate_launch_fields(browser_type: &str) -> AppResult<()> {
    if !detector::is_known_browser_type(browser_type) {
        return Err(AppError::validation(format!(
            "Unknown browser type '{browser_type}'. Supported: {}",
            detector::known_browser_types().join(", ")
        )));
    }
    Ok(())
}

pub fn insert_profile(conn: &Connection, profile: &Profile) -> AppResult<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, browser_type, user_data_dir, proxy_id, notes, status, created_at, updated_at, last_used_at, pinned, restart_on_crash, folder_id, fingerprint, startup_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            profile.restart_on_crash as i64,
            profile.folder_id,
            encode_fingerprint(&profile.fingerprint)?,
            profile.startup_url,
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
    validate_launch_fields(&input.browser_type)?;
    validate_fingerprint(input.fingerprint.as_ref())?;
    let startup_url = validate_startup_url(input.startup_url.as_deref())?;
    if let Some(fid) = &input.folder_id {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM folders WHERE id = ?1",
            params![fid],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::validation(format!("Folder {fid} not found")));
        }
    }

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
        restart_on_crash: input.restart_on_crash,
        folder_id: input.folder_id,
        fingerprint: input.fingerprint.filter(|f| !f.is_empty()),
        startup_url,
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
    let restart_on_crash = input.restart_on_crash.unwrap_or(existing.restart_on_crash);
    let folder_id = match input.folder_id {
        Some(v) => v,
        None => existing.folder_id.clone(),
    };
    let fingerprint = match input.fingerprint {
        Some(v) => v.filter(|f| !f.is_empty()),
        None => existing.fingerprint.clone(),
    };
    let startup_url = match input.startup_url {
        Some(v) => validate_startup_url(v.as_deref())?,
        None => existing.startup_url.clone(),
    };

    validate_launch_fields(&browser_type)?;
    validate_fingerprint(fingerprint.as_ref())?;
    if let Some(fid) = &folder_id {
        if existing.folder_id.as_deref() != Some(fid.as_str()) {
            // Only validate when actually changing — the current value may
            // reference a folder deleted with FKs off.
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM folders WHERE id = ?1",
                params![fid],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(AppError::validation(format!("Folder {fid} not found")));
            }
        }
    }

    conn.execute(
        "UPDATE profiles SET name = ?1, browser_type = ?2, proxy_id = ?3, notes = ?4, restart_on_crash = ?5, folder_id = ?6, fingerprint = ?7, startup_url = ?8, updated_at = ?9 WHERE id = ?10",
        params![
            name,
            browser_type,
            proxy_id,
            notes,
            restart_on_crash as i64,
            folder_id,
            encode_fingerprint(&fingerprint)?,
            startup_url,
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
        restart_on_crash: source.restart_on_crash,
        folder_id: source.folder_id.clone(),
        fingerprint: source.fingerprint.clone(),
        startup_url: source.startup_url.clone(),
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
        p.restart_on_crash = true;
        insert_profile(&conn, &p).unwrap();

        let loaded = get_profile_by_id(&conn, &p.id).unwrap();
        assert!(loaded.restart_on_crash);
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
        assert!(validate_launch_fields("chrome").is_ok());
        assert!(validate_launch_fields("nobody-browser").is_err());
    }

    #[test]
    fn fingerprint_validation_rejects_unsafe_values() {
        let ok = Fingerprint {
            language: Some("en-US".into()),
            timezone: Some("Europe/Berlin".into()),
            ..Default::default()
        };
        assert!(validate_fingerprint(Some(&ok)).is_ok());
        assert!(validate_fingerprint(None).is_ok());

        let bad_lang = Fingerprint { language: Some("en US\"".into()), ..Default::default() };
        assert!(validate_fingerprint(Some(&bad_lang)).is_err());
        let bad_tz = Fingerprint { timezone: Some("Europe/<script>".into()), ..Default::default() };
        assert!(validate_fingerprint(Some(&bad_tz)).is_err());
        let bad_ua = Fingerprint { user_agent: Some("a\nb".into()), ..Default::default() };
        assert!(validate_fingerprint(Some(&bad_ua)).is_err());
        let bad_geo = Fingerprint {
            geolocation: Some(crate::models::profile::Geolocation {
                latitude: 91.0,
                longitude: 0.0,
                accuracy: 50.0,
            }),
            ..Default::default()
        };
        assert!(validate_fingerprint(Some(&bad_geo)).is_err());
    }

    #[test]
    fn startup_url_accepts_only_http_and_https() {
        assert_eq!(validate_startup_url(None).unwrap(), None);
        assert_eq!(validate_startup_url(Some("   ")).unwrap(), None);
        assert_eq!(
            validate_startup_url(Some(" https://example.com/a ")).unwrap(),
            Some("https://example.com/a".into())
        );
        assert!(validate_startup_url(Some("file:///etc/passwd")).is_err());
        assert!(validate_startup_url(Some("javascript:alert(1)")).is_err());
        assert!(validate_startup_url(Some("https://exa mple.com")).is_err());
        assert!(validate_startup_url(Some("not a url")).is_err());
    }

    #[test]
    fn fingerprint_round_trips_and_ignores_corrupt_json() {
        let fp = Fingerprint {
            language: Some("de-DE".into()),
            fonts: Some(vec!["Arial".into()]),
            ..Default::default()
        };
        let stored = encode_fingerprint(&Some(fp.clone())).unwrap();
        assert_eq!(decode_fingerprint(stored), Some(fp));

        // An empty fingerprint is stored as NULL and read back as None.
        assert_eq!(encode_fingerprint(&Some(Fingerprint::default())).unwrap(), None);
        // Corrupt stored JSON must not make the profile unreadable.
        assert_eq!(decode_fingerprint(Some("{not json".into())), None);
    }
}
