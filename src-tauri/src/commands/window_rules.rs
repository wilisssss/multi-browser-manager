use crate::browser::launcher;
use crate::commands::profile::{db_lock, list_profiles};
use crate::error::AppResult;
use crate::AppState;
use serde::Serialize;
use tauri::State;

/// Per-profile window identity: every browser is launched with a unique
/// `--class` / `--wayland-app-id` so compositors (niri, etc.) can attach
/// window rules per profile, and workspaces can be assigned per tag.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRule {
    pub profile_id: String,
    pub profile_name: String,
    pub app_id: String,
    pub status: String,
    pub group_ids: Vec<String>,
}

#[tauri::command]
pub fn get_window_rules(state: State<'_, AppState>) -> AppResult<Vec<WindowRule>> {
    let conn = db_lock(&state)?;
    let profiles = list_profiles(&conn)?;
    Ok(profiles
        .into_iter()
        .map(|p| WindowRule {
            app_id: launcher::window_app_id(&p.id, &p.name),
            profile_id: p.id,
            profile_name: p.name,
            status: p.status,
            group_ids: p.groups.iter().map(|g| g.id.clone()).collect(),
        })
        .collect())
}
