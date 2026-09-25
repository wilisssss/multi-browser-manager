use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

/// Key under which the whole settings JSON blob is stored.
pub const SETTINGS_KEY: &str = "app";

/// Application preferences. Serialized as one JSON value; `default` on the
/// container means older rows (missing fields) still deserialize cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Closed launch-history entries older than this are pruned at startup.
    pub history_retention_days: i64,
    /// Hard cap on launch-history rows (newest are kept).
    pub history_max_entries: i64,
    /// Automatic daily snapshot of profiles+proxies into the backups folder.
    pub auto_backup: bool,
    /// How many auto-snapshots to keep.
    pub auto_backup_keep: i64,
    /// Preselected browser type when creating a new profile.
    pub default_browser_type: String,
    /// Tag (group) id -> niri workspace number, used by the window-rules
    /// generator: profiles carrying the tag open on that workspace.
    pub group_workspaces: HashMap<String, i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_retention_days: 90,
            history_max_entries: 1000,
            auto_backup: true,
            auto_backup_keep: 10,
            default_browser_type: "chromium".into(),
            group_workspaces: HashMap::new(),
        }
    }
}

/// Loads settings, falling back to defaults for missing/corrupt rows.
pub fn load_settings(conn: &rusqlite::Connection) -> Settings {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![SETTINGS_KEY],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|json| serde_json::from_str(&json).ok())
    .unwrap_or_default()
}

pub fn save_settings(conn: &rusqlite::Connection, settings: &Settings) -> AppResult<()> {
    let json = serde_json::to_string(settings)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SETTINGS_KEY, json],
    )?;
    Ok(())
}

fn validate(settings: &Settings) -> AppResult<()> {
    if !(1..=3650).contains(&settings.history_retention_days) {
        return Err(AppError::validation(
            "History retention must be between 1 and 3650 days",
        ));
    }
    if !(10..=100_000).contains(&settings.history_max_entries) {
        return Err(AppError::validation(
            "History max entries must be between 10 and 100000",
        ));
    }
    if !(1..=100).contains(&settings.auto_backup_keep) {
        return Err(AppError::validation(
            "Snapshots to keep must be between 1 and 100",
        ));
    }
    for ws in settings.group_workspaces.values() {
        if !(1..=100).contains(ws) {
            return Err(AppError::validation(
                "Workspace numbers must be between 1 and 100",
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    let conn = db_lock(&state)?;
    Ok(load_settings(&conn))
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    validate(&settings)?;
    let conn = db_lock(&state)?;
    save_settings(&conn, &settings)?;
    Ok(settings)
}

// ---- Window rules / workspaces (moved from commands/window_rules.rs — A6:
// one "display preferences" domain, one module) ----

use crate::browser::launcher;

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
    let profiles = crate::commands::profile::list_profiles(&conn)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::path::Path;

    #[test]
    fn defaults_round_trip_and_backfill_missing_fields() {
        let conn = db::init(Path::new(":memory:")).unwrap();

        // Fresh DB → defaults.
        let s = load_settings(&conn);
        assert_eq!(s.history_retention_days, 90);
        assert!(s.auto_backup);
        assert_eq!(s.default_browser_type, "chromium");

        // Save + load round trip.
        let mut s2 = s.clone();
        s2.history_retention_days = 30;
        s2.auto_backup = false;
        s2.group_workspaces.insert("gid-1".into(), 2);
        save_settings(&conn, &s2).unwrap();
        let s3 = load_settings(&conn);
        assert_eq!(s3.history_retention_days, 30);
        assert!(!s3.auto_backup);
        assert_eq!(s3.group_workspaces.get("gid-1"), Some(&2));

        // A row missing newer fields backfills from defaults.
        conn.execute(
            "UPDATE settings SET value = '{\"historyRetentionDays\":7}' WHERE key = 'app'",
            [],
        )
        .unwrap();
        let s4 = load_settings(&conn);
        assert_eq!(s4.history_retention_days, 7);
        assert_eq!(s4.history_max_entries, 1000);
        assert!(s4.auto_backup);
    }

    #[test]
    fn validate_rejects_out_of_range_values() {
        let s = Settings { history_retention_days: 0, ..Default::default() };
        assert!(validate(&s).is_err());
        let s = Settings { auto_backup_keep: 0, ..Default::default() };
        assert!(validate(&s).is_err());
        let mut s = Settings::default();
        s.group_workspaces.insert("g".into(), 0);
        assert!(validate(&s).is_err());
        assert!(validate(&Settings::default()).is_ok());
    }
}
