use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
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
    /// Launch browsers in memory-trim ("lightweight") mode: append curated
    /// RAM-saving flags (process-per-site, no media router/translate/sync
    /// services, ...). Default on — farm/kiosk profiles rarely want them.
    /// Field-level default (not container-level): container serde default
    /// fills a missing bool with `false`, silently switching the mode off
    /// for settings rows saved before this field existed.
    #[serde(default = "default_true")]
    pub lightweight_browsers: bool,
}

/// Serde default for opt-out-by-default booleans (`bool::default` is false,
/// which is the wrong polarity for a default-on setting).
fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_retention_days: 90,
            history_max_entries: 1000,
            auto_backup: true,
            auto_backup_keep: 10,
            default_browser_type: "chromium".into(),
            lightweight_browsers: true,
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
        save_settings(&conn, &s2).unwrap();
        let s3 = load_settings(&conn);
        assert_eq!(s3.history_retention_days, 30);
        assert!(!s3.auto_backup);

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
        // Regression: a pre-lightweight row must default the mode ON
        // (container-level serde default would yield false for a bool).
        assert!(s4.lightweight_browsers);
    }

    #[test]
    fn validate_rejects_out_of_range_values() {
        let s = Settings { history_retention_days: 0, ..Default::default() };
        assert!(validate(&s).is_err());
        let s = Settings { auto_backup_keep: 0, ..Default::default() };
        assert!(validate(&s).is_err());
        assert!(validate(&Settings::default()).is_ok());
    }
}
