use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::models::folder::{CreateFolderInput, Folder, MoveFolderInput, MoveProfilesInput, RenameFolderInput};
use crate::AppState;
use rusqlite::{params, Connection};
use tauri::{AppHandle, Emitter, State};

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get("id")?,
        name: row.get("name")?,
        parent_id: row.get("parent_id")?,
        position: row.get("position")?,
        created_at: row.get("created_at")?,
        profile_count: row.get("profile_count")?,
    })
}

/// Lists every folder with the number of profiles directly inside it.
/// The client builds the tree / filters by parent — folders are few, so one
/// flat query is simpler and cheaper than per-level queries.
pub fn list_folders(conn: &Connection) -> AppResult<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.name, f.parent_id, f.position, f.created_at,
                (SELECT COUNT(*) FROM profiles p WHERE p.folder_id = f.id) AS profile_count
         FROM folders f
         ORDER BY f.position ASC, f.name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map([], row_to_folder)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn get_folders(state: State<'_, AppState>) -> AppResult<Vec<Folder>> {
    let conn = db_lock(&state)?;
    list_folders(&conn)
}

/// Case-insensitive unique name within the same parent (create + rename).
fn validate_name(conn: &Connection, name: &str, parent_id: Option<&str>, exclude_id: Option<&str>) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::validation("Folder name cannot be empty"));
    }
    if name.len() > 64 {
        return Err(AppError::validation("Folder name must be at most 64 characters"));
    }
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM folders
         WHERE name = ?1 COLLATE NOCASE
           AND ((parent_id IS NULL AND ?2 IS NULL) OR parent_id = ?2)
           AND (?3 IS NULL OR id != ?3)",
        params![name, parent_id, exclude_id],
        |r| r.get(0),
    )?;
    if exists {
        return Err(AppError::validation(format!(
            "A folder named '{name}' already exists here"
        )));
    }
    Ok(())
}

fn folder_exists(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row("SELECT COUNT(*) > 0 FROM folders WHERE id = ?1", params![id], |r| r.get(0))?)
}

#[tauri::command]
pub fn create_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateFolderInput,
) -> AppResult<Folder> {
    let conn = db_lock(&state)?;
    if let Some(parent) = &input.parent_id {
        if !folder_exists(&conn, parent)? {
            return Err(AppError::not_found(format!("Folder {parent} not found")));
        }
    }
    validate_name(&conn, &input.name, input.parent_id.as_deref(), None)?;

    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM folders WHERE ((parent_id IS NULL AND ?1 IS NULL) OR parent_id = ?1)",
        params![input.parent_id],
        |r| r.get(0),
    )?;

    let folder = Folder {
        id: uuid::Uuid::new_v4().to_string(),
        name: input.name.trim().to_string(),
        parent_id: input.parent_id,
        position,
        created_at: chrono::Utc::now().timestamp(),
        profile_count: 0,
    };
    conn.execute(
        "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![folder.id, folder.name, folder.parent_id, folder.position, folder.created_at],
    )?;
    drop(conn);
    // Folders don't appear in the tray, but the UI listens to this event.
    let _ = app.emit("profiles-changed", String::new());
    Ok(folder)
}

#[tauri::command]
pub fn rename_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: RenameFolderInput,
) -> AppResult<()> {
    let conn = db_lock(&state)?;
    if !folder_exists(&conn, &id)? {
        return Err(AppError::not_found(format!("Folder {id} not found")));
    }
    let parent_id: Option<String> = conn
        .query_row("SELECT parent_id FROM folders WHERE id = ?1", params![id], |r| r.get(0))?;
    validate_name(&conn, &input.name, parent_id.as_deref(), Some(&id))?;
    conn.execute("UPDATE folders SET name = ?1 WHERE id = ?2", params![input.name.trim(), id])?;
    drop(conn);
    let _ = app.emit("profiles-changed", String::new());
    Ok(())
}

#[tauri::command]
pub fn delete_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let conn = db_lock(&state)?;
    let parent_id: Option<String> = conn
        .query_row("SELECT parent_id FROM folders WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::not_found(format!("Folder {id} not found")),
            other => other.into(),
        })?;
    // File-manager-safe delete: contents survive, re-parented one level up.
    conn.execute(
        "UPDATE folders SET parent_id = ?1 WHERE parent_id = ?2",
        params![parent_id, id],
    )?;
    conn.execute(
        "UPDATE profiles SET folder_id = ?1 WHERE folder_id = ?2",
        params![parent_id, id],
    )?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
    drop(conn);
    let _ = app.emit("profiles-changed", String::new());
    Ok(())
}

/// Rejects moving a folder into itself or one of its descendants (which would
/// orphan the subtree from the root).
fn validate_move_target(conn: &Connection, id: &str, new_parent: Option<&str>) -> AppResult<()> {
    let Some(target) = new_parent else {
        return Ok(()); // root is always valid
    };
    if target == id {
        return Err(AppError::validation("Cannot move a folder into itself"));
    }
    // Walk up from the target; if we reach `id`, the target is a descendant.
    let mut current = Some(target.to_string());
    let mut hops = 0;
    while let Some(pid) = current {
        if pid == id {
            return Err(AppError::validation(
                "Cannot move a folder into one of its own subfolders",
            ));
        }
        current = conn
            .query_row("SELECT parent_id FROM folders WHERE id = ?1", params![pid], |r| r.get(0))
            .ok();
        hops += 1;
        if hops > 100 {
            // Corrupt chain guard; should be unreachable.
            break;
        }
    }
    if !folder_exists(conn, &target)? {
        return Err(AppError::not_found(format!("Folder {target} not found")));
    }
    Ok(())
}

#[tauri::command]
pub fn move_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: MoveFolderInput,
) -> AppResult<()> {
    let conn = db_lock(&state)?;
    if !folder_exists(&conn, &id)? {
        return Err(AppError::not_found(format!("Folder {id} not found")));
    }
    validate_move_target(&conn, &id, input.parent_id.as_deref())?;
    conn.execute(
        "UPDATE folders SET parent_id = ?1 WHERE id = ?2",
        params![input.parent_id, id],
    )?;
    drop(conn);
    let _ = app.emit("profiles-changed", String::new());
    Ok(())
}

#[tauri::command]
pub fn move_profiles_to_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    input: MoveProfilesInput,
) -> AppResult<()> {
    if input.profile_ids.is_empty() {
        return Ok(());
    }
    let conn = db_lock(&state)?;
    if let Some(fid) = &input.folder_id {
        if !folder_exists(&conn, fid)? {
            return Err(AppError::not_found(format!("Folder {fid} not found")));
        }
    }
    let now = chrono::Utc::now().timestamp();
    for pid in &input.profile_ids {
        let changed = conn.execute(
            "UPDATE profiles SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![input.folder_id, now, pid],
        )?;
        if changed == 0 {
            return Err(AppError::not_found(format!("Profile {pid} not found")));
        }
    }
    drop(conn);
    crate::sync::after_profile_change(&app, &input.profile_ids[0]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn mem_db() -> Connection {
        let conn = crate::db::init(Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO folders (id, name, parent_id, position, created_at) VALUES ('f1', 'Work', NULL, 0, 1)", [])
            .unwrap();
        conn
    }

    fn make_profile(conn: &Connection, id: &str, name: &str, folder: Option<&str>) {
        let now = 1;
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, proxy_id, notes, status, created_at, updated_at, last_used_at, pinned, extra_args, restart_on_crash, stop_timeout_secs, folder_id)
             VALUES (?1, ?2, 'chromium', ?3, NULL, NULL, 'stopped', ?4, ?4, NULL, 0, NULL, 0, NULL, ?5)",
            rusqlite::params![id, name, format!("/tmp/mbm-test-{id}"), now, folder],
        )
        .unwrap();
    }

    #[test]
    fn create_renames_and_lists_with_counts() {
        let conn = mem_db();
        validate_name(&conn, "Sub", Some("f1"), None).unwrap();
        conn.execute(
            "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES ('f2', 'Sub', 'f1', 0, 2)",
            [],
        )
        .unwrap();
        make_profile(&conn, "p1", "alpha", Some("f2"));
        make_profile(&conn, "p2", "beta", None);

        let folders = list_folders(&conn).unwrap();
        assert_eq!(folders.len(), 2);
        let f2 = folders.iter().find(|f| f.id == "f2").unwrap();
        assert_eq!(f2.profile_count, 1);
        assert_eq!(f2.parent_id.as_deref(), Some("f1"));

        // Duplicate name under the same parent is rejected; other parents ok.
        assert!(validate_name(&conn, "SUB", Some("f1"), None).is_err());
        assert!(validate_name(&conn, "Sub", Some("f1"), Some("f2")).is_ok());
        assert!(validate_name(&conn, "Work", None, None).is_err());
        assert!(validate_name(&conn, "", None, None).is_err());
    }

    /// Mirrors delete_folder's SQL (the command itself needs an AppHandle):
    /// subfolders re-parent to the deleted folder's parent; profiles move to
    /// that same parent — nothing is ever dropped.
    #[test]
    fn delete_folder_moves_contents_up_never_drops() {
        let conn = mem_db();
        // Root folder f1 contains subfolder f2; p1 sits directly in f1,
        // p2 sits in the subfolder.
        conn.execute(
            "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES ('f2', 'Sub', 'f1', 0, 2)",
            [],
        )
        .unwrap();
        make_profile(&conn, "p1", "alpha", Some("f1"));
        make_profile(&conn, "p2", "beta", Some("f2"));

        conn.execute("UPDATE folders SET parent_id = (SELECT parent_id FROM folders WHERE id = 'f1') WHERE parent_id = 'f1'", []).unwrap();
        conn.execute("UPDATE profiles SET folder_id = (SELECT parent_id FROM folders WHERE id = 'f1') WHERE folder_id = 'f1'", []).unwrap();
        conn.execute("DELETE FROM folders WHERE id = 'f1'", []).unwrap();

        // Subfolder survives at root; both profiles survive with valid homes.
        let f2_parent: Option<String> = conn
            .query_row("SELECT parent_id FROM folders WHERE id = 'f2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(f2_parent, None);
        let p1: Option<String> = conn
            .query_row("SELECT folder_id FROM profiles WHERE id = 'p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(p1, None, "profile directly in the deleted folder goes to unfiled (root)");
        let p2: Option<String> = conn
            .query_row("SELECT folder_id FROM profiles WHERE id = 'p2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(p2.as_deref(), Some("f2"), "profile in a surviving subfolder stays there");
    }

    #[test]
    fn move_target_rejects_cycles() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES ('f2', 'Sub', 'f1', 0, 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES ('f3', 'Deep', 'f2', 0, 3)",
            [],
        )
        .unwrap();
        // f2 into f3 (its own child) is a cycle.
        assert!(validate_move_target(&conn, "f2", Some("f3")).is_err());
        // f2 into itself is a cycle.
        assert!(validate_move_target(&conn, "f2", Some("f2")).is_err());
        // f2 into f1 (its current parent) is fine, as is root.
        assert!(validate_move_target(&conn, "f2", Some("f1")).is_ok());
        assert!(validate_move_target(&conn, "f2", None).is_ok());
        // Nonexistent target.
        assert!(validate_move_target(&conn, "f2", Some("nope")).is_err());
    }
}
