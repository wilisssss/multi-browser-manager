use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::models::group::Group;
use crate::AppState;
use rusqlite::{params, Connection};
use tauri::State;

fn row_to_group(row: &rusqlite::Row<'_>) -> rusqlite::Result<Group> {
    Ok(Group {
        id: row.get("id")?,
        name: row.get("name")?,
        color: row.get("color")?,
    })
}

pub fn list_groups(conn: &Connection) -> AppResult<Vec<Group>> {
    let mut stmt =
        conn.prepare("SELECT id, name, color FROM groups ORDER BY name COLLATE NOCASE ASC")?;
    let rows = stmt
        .query_map([], row_to_group)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn get_groups(state: State<'_, AppState>) -> AppResult<Vec<Group>> {
    let conn = db_lock(&state)?;
    list_groups(&conn)
}

/// Palette cycled automatically so users never have to pick a color.
const PALETTE: &[&str] = &[
    "#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#ec4899", "#06b6d4", "#84cc16",
];

#[tauri::command]
pub fn create_group(state: State<'_, AppState>, name: String) -> AppResult<Group> {
    let conn = db_lock(&state)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation("Tag name cannot be empty"));
    }

    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM groups WHERE LOWER(name) = LOWER(?1)",
        params![name],
        |r| r.get(0),
    )?;
    if exists {
        return Err(AppError::validation(format!("Tag '{name}' already exists")));
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM groups", [], |r| r.get(0))?;
    let color = PALETTE[(count as usize) % PALETTE.len()].to_string();

    let group = Group {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        color: Some(color),
    };

    conn.execute(
        "INSERT INTO groups (id, name, color) VALUES (?1, ?2, ?3)",
        params![group.id, group.name, group.color],
    )?;

    Ok(group)
}

/// Replaces the full tag set of a profile (diff handled client-side).
#[tauri::command]
pub fn set_profile_groups(
    state: State<'_, AppState>,
    profile_id: String,
    group_ids: Vec<String>,
) -> AppResult<()> {
    let conn = db_lock(&state)?;

    let profile_exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM profiles WHERE id = ?1",
        params![profile_id],
        |r| r.get(0),
    )?;
    if !profile_exists {
        return Err(AppError::not_found(format!("Profile {profile_id} not found")));
    }

    // Validate all group ids first so we never leave a partial assignment.
    for gid in &group_ids {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM groups WHERE id = ?1",
            params![gid],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::validation(format!("Tag {gid} not found")));
        }
    }

    conn.execute(
        "DELETE FROM profile_groups WHERE profile_id = ?1",
        params![profile_id],
    )?;
    for gid in &group_ids {
        conn.execute(
            "INSERT OR IGNORE INTO profile_groups (profile_id, group_id) VALUES (?1, ?2)",
            params![profile_id, gid],
        )?;
    }

    Ok(())
}
