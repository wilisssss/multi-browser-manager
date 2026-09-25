pub mod browser;
pub mod commands;
pub mod db;
pub mod error;
pub mod models;
pub mod proxy_manager;
pub mod services;
pub mod startup;
pub mod sync;
pub mod tray;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex as AsyncMutex;

use commands::process::RunningProc;

/// CLI actions usable from WM keybinds (e.g. niri):
///   mbm --launch <profile-name-or-id>   launch a profile's browser
///   mbm --stop <profile-name-or-id>     stop a profile's browser
///   mbm --list                          print profiles as JSON and exit
mod cli {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub enum CliAction {
        Launch(String),
        Stop(String),
    }

    /// Scans raw argv (with or without program name) for a supported action.
    /// Returns Err when a flag is present but its value is missing — otherwise
    /// `mbm --launch` (no profile) would fail silently from a WM keybind.
    pub fn find_action(args: &[String]) -> Result<Option<CliAction>, String> {
        let mut iter = args.iter();
        while let Some(a) = iter.next() {
            match a.as_str() {
                "--launch" | "-l" => {
                    return iter.next().map(|n| Some(CliAction::Launch(n.clone()))).ok_or_else(
                        || "--launch requires a profile name (e.g. `mbm --launch work`)".into(),
                    );
                }
                "--stop" | "-s" => {
                    return iter.next().map(|n| Some(CliAction::Stop(n.clone()))).ok_or_else(
                        || "--stop requires a profile name (e.g. `mbm --stop work`)".into(),
                    );
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Resolves a profile by name (exact, case-insensitive) or id.
    pub fn resolve_profile(
        conn: &rusqlite::Connection,
        name_or_id: &str,
    ) -> Option<String> {
        conn.query_row(
            "SELECT id FROM profiles WHERE id = ?1 OR LOWER(name) = LOWER(?2) LIMIT 1",
            rusqlite::params![name_or_id, name_or_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
    }

    /// Executes a CLI action in the running (or starting) app instance.
    pub fn execute(app: &AppHandle, action: CliAction) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let (action_name, target, profile_id) = {
                let Ok(conn) = state.db.lock() else { return };
                let (action_name, target) = match &action {
                    CliAction::Launch(t) => ("launch", t),
                    CliAction::Stop(t) => ("stop", t),
                };
                match resolve_profile(&conn, target) {
                    Some(id) => (action_name, target.clone(), id),
                    None => {
                        notify_failure(&format!(
                            "No profile named '{target}' (action: {action_name})"
                        ));
                        return;
                    }
                }
            };

            let result = match action {
                CliAction::Launch(_) => {
                    let r = crate::commands::process::do_launch(app.clone(), profile_id).await;
                    if r.success { None } else { Some(r.message) }
                }
                CliAction::Stop(_) => {
                    crate::commands::process::do_stop(app.clone(), profile_id)
                        .await
                        .err()
                        .map(|e| e.to_string())
                }
            };

            if let Some(err) = result {
                notify_failure(&format!("Failed to {action_name} '{target}': {err}"));
            }
        });
    }

    /// Desktop notification for CLI failures — keybind errors are otherwise
    /// invisible (stderr goes nowhere when spawned by a WM).
    fn notify_failure(message: &str) {
        eprintln!("mbm: {message}");
        let _ = notify_rust::Notification::new()
            .summary("Multi Browser Manager")
            .body(message)
            .show();
    }

    /// Notifies about malformed CLI usage (e.g. missing flag value).
    pub fn report_invalid_usage(message: &str) {
        notify_failure(message);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn finds_launch_and_stop_actions() {
            let args: Vec<String> = vec!["mbm".into(), "--launch".into(), "work".into()];
            assert_eq!(
                find_action(&args),
                Ok(Some(CliAction::Launch("work".into())))
            );
            let args2: Vec<String> = vec!["--stop".into(), "personal".into()];
            assert_eq!(find_action(&args2), Ok(Some(CliAction::Stop("personal".into()))));
            let none: Vec<String> = vec!["mbm".into()];
            assert_eq!(find_action(&none), Ok(None));
        }

        #[test]
        fn rejects_missing_flag_values() {
            let bad: Vec<String> = vec!["mbm".into(), "--launch".into()];
            assert!(find_action(&bad).is_err());
            let bad2: Vec<String> = vec!["--stop".into()];
            assert!(find_action(&bad2).is_err());
        }

        #[test]
        fn resolves_profile_by_name_and_id_case_insensitively() {
            let conn = crate::db::init(std::path::Path::new(":memory:")).unwrap();
            crate::commands::profile::insert_profile(
                &conn,
                &crate::models::profile::Profile::new("fixed-id", "Work", "chromium", "/tmp/mbm/fixed-id"),
            )
            .unwrap();

            assert_eq!(resolve_profile(&conn, "work").as_deref(), Some("fixed-id"));
            assert_eq!(resolve_profile(&conn, "WORK").as_deref(), Some("fixed-id"));
            assert_eq!(resolve_profile(&conn, "fixed-id").as_deref(), Some("fixed-id"));
            assert_eq!(resolve_profile(&conn, "nope"), None);
        }
    }
}

pub use cli::CliAction;

/// Shared application state managed by Tauri.
#[derive(Clone)]
pub struct AppState {
    /// SQLite connection (rusqlite is sync; guarded by a std Mutex).
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// Root directory that holds one sub-directory per profile.
    pub profiles_root: PathBuf,
    /// Process registry (A1): one entry per running browser. The tokio Child
    /// is owned by the per-profile supervisor task; the entry exposes a stop
    /// channel into it and an exit signal out of it — no pid polling.
    pub running: Arc<AsyncMutex<HashMap<String, RunningProc>>>,
    /// browser_type -> executable path cache, filled on demand. Cleared on
    /// every explicit `detect_browsers` IPC call so installs/uninstalls made
    /// while MBM runs are picked up without a restart (L5).
    pub browser_cache: Arc<Mutex<HashMap<String, String>>>,
    /// Per-profile operation locks: serialize launch/stop of the same profile
    /// so concurrent triggers (tray + CLI keybind) can't double-spawn.
    pub profile_locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
    /// CPU-usage baselines for the resource panel (F2): pid →
    /// (total cpu ticks at last sample, when it was taken).
    pub usage_samples: Arc<Mutex<HashMap<u32, (u64, std::time::Instant)>>>,
}

fn app_data_root() -> PathBuf {
    directories::ProjectDirs::from("com", "mbm", "Multi Browser Manager")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            std::env::temp_dir().join("multi-browser-manager")
        })
}

/// Handles `--list`: prints profiles as JSON to stdout and exits.
/// Designed for WM keybinds / scripts (e.g. rofi, fuzzel menus in niri).
fn handle_list_command() {
    let db_path = app_data_root().join("mbm.sqlite3");
    // Read-only: this process must never run migrations (that races with the
    // app's startup migration pass) nor touch the file at all. A DB that does
    // not exist yet simply means "no profiles".
    let conn = match db::open_readonly(&db_path) {
        Ok(Some(c)) => c,
        Ok(None) => {
            println!("[]");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("mbm: failed to open database: {e}");
            std::process::exit(1);
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT name, status FROM profiles ORDER BY name ASC",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mbm: {e}");
            std::process::exit(1);
        }
    };

    let profiles: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "name": r.get::<_, String>(0)?,
                "status": r.get::<_, String>(1)?,
            }))
        })
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        .unwrap_or_default();

    println!("{}", serde_json::to_string(&profiles).unwrap_or_else(|_| "[]".into()));
    std::process::exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let own_args: Vec<String> = std::env::args().collect();
    let own_args = std::sync::Arc::new(own_args);

    // `--list` never opens a window: print profiles and exit (for scripts/WMs).
    if own_args.iter().any(|a| a == "--list") {
        handle_list_command();
    }

    // A malformed CLI action (e.g. `mbm --launch` without a name) must not fail
    // silently — as the first instance we can still show a notification.
    let pending_action = match cli::find_action(&own_args) {
        Ok(action) => action,
        Err(message) => {
            cli::report_invalid_usage(&message);
            return;
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A second invocation happened: unhide + focus the existing window and
            // run any CLI action it carried (e.g. `mbm --launch work` from a keybind).
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            match cli::find_action(&args) {
                Ok(Some(action)) => cli::execute(app, action),
                Ok(None) => {}
                Err(message) => cli::report_invalid_usage(&message),
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().with_state_flags(
            // The niri window rule (app-id ^mbm$) opens the dashboard maximized
            // at 100% width; don't let a stale saved size fight the compositor.
            tauri_plugin_window_state::StateFlags::all()
                & !tauri_plugin_window_state::StateFlags::SIZE,
        ).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            let data_root = app_data_root();
            std::fs::create_dir_all(&data_root)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&data_root, std::fs::Permissions::from_mode(0o700));
            }

            let profiles_root = data_root.join("profiles");
            std::fs::create_dir_all(&profiles_root)?;

            let conn = db::init(&data_root.join("mbm.sqlite3"))?;

            // One-time migration: move any pre-DB-storage credential secrets
            // out of the OS keychain into the database.
            commands::credentials::migrate_keychain_secrets_to_db(&conn);

            // Reconcile DB status with reality (orphans from a previous session
            // back to 'running', stale rows to 'stopped'). See startup.rs for
            // the full rules and per-platform limitations.
            if let Err(e) = startup::reconcile_statuses(&conn) {
                eprintln!("mbm: status reconciliation failed: {e}");
            }

            // Sweep leftover proxy-extension credential dirs from previous
            // sessions (e.g. MBM crashed while browsers were still open).
            // Runs AFTER reconciliation: orphaned-but-live profiles are now
            // 'running' with a verified SingletonLock, so the sweep keeps
            // their extension files (live browsers still reference them) and
            // deletes the rest.
            proxy_manager::cleanup_all_extensions(&[], &conn);

            // Prune launch history per user settings.
            startup::prune_launch_history(&conn, &commands::settings::load_settings(&conn));

            // Purge trash snapshots older than the retention window (F6) so
            // deleted profile data does not linger forever.
            match startup::purge_expired_trash(&conn, &profiles_root) {
                Ok(0) => {}
                Ok(n) => eprintln!("mbm: purged {n} expired trash snapshot(s)"),
                Err(e) => eprintln!("mbm: trash purge failed: {e}"),
            }

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                profiles_root,
                running: Arc::new(AsyncMutex::new(HashMap::new())),
                browser_cache: Arc::new(Mutex::new(HashMap::new())),
                profile_locks: Arc::new(Mutex::new(HashMap::new())),
                usage_samples: Arc::new(Mutex::new(HashMap::new())),
            });

            // Belt and braces: strip client-side decorations at runtime too
            // (some GTK/WebKit builds ignore the config-only setting).
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = window.set_decorations(false) {
                    eprintln!("mbm: failed to unset decorations: {e}");
                }
            }

            // Handle CLI actions from this first instance (e.g. niri keybind that
            // starts the app and immediately launches a profile).
            if let Some(action) = pending_action {
                cli::execute(app.handle(), action);
            }

            // System tray: dashboard access + per-profile launch/stop toggles.
            // schedule_rebuild lazily spawns the single debounce worker (L8);
            // its first pass performs the initial build.
            tray::schedule_rebuild(app.handle());

            // Automatic snapshot backups: check shortly after startup, then
            // every 6 hours (run_snapshot_if_due enforces a 24 h interval and
            // is a no-op when the feature is disabled in settings).
            let snapshot_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    crate::services::backup::run_snapshot_if_due(&snapshot_app.state::<AppState>());
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Profile
            commands::profile::get_profiles,
            commands::profile::create_profile,
            commands::profile::update_profile,
            commands::profile::delete_profile,
            commands::profile::restore_profile,
            commands::profile::get_trash_count,
            commands::profile::empty_trash,
            commands::profile::duplicate_profile,
            commands::profile::toggle_pin,
            // Groups / tags
            commands::groups::get_groups,
            commands::groups::create_group,
            commands::groups::set_profile_groups,
            // Folders (file-manager style organization)
            commands::folders::get_folders,
            commands::folders::create_folder,
            commands::folders::rename_folder,
            commands::folders::delete_folder,
            commands::folders::move_folder,
            commands::folders::move_profiles_to_folder,
            // Process
            commands::process::launch_profile,
            commands::process::stop_profile,
            commands::process::bulk_launch,
            commands::process::bulk_stop,
            commands::process::get_resource_usage,
            commands::process::get_usage_stats,
            commands::process::get_launch_history,
            // Browser
            commands::detect_browsers,
            // Proxy
            commands::proxy::get_proxies,
            commands::proxy::create_proxy,
            commands::proxy::update_proxy,
            commands::proxy::delete_proxy,
            commands::proxy::test_proxy,
            // Backup
            commands::backup::export_profiles,
            commands::backup::export_profiles_encrypted,
            commands::backup::import_profiles,
            commands::backup::get_backup_dir,
            // Settings + window rules / workspaces
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_window_rules,
            // Credentials (per-profile accounts; secrets in the local DB)
            commands::credentials::get_credentials,
            commands::credentials::create_credential,
            commands::credentials::update_credential,
            commands::credentials::delete_credential,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Multi Browser Manager")
        .run(|app_handle, event| {
            // On any exit path (tray quit, SIGTERM, updater restart): remove
            // leftover proxy extension dirs — they contain credentials.
            // Profiles whose browser is still running in THIS session are
            // skipped: Chromium loads the extension lazily (service-worker
            // reload), so deleting it under a live browser breaks its proxy
            // auth mid-session. The watcher cleans those when they exit; the
            // next MBM startup sweeps anything still behind.
            if let tauri::RunEvent::Exit = event {
                let app = app_handle.clone();
                tauri::async_runtime::block_on(async move {
                    let state = app.state::<AppState>();
                    let running: Vec<String> = state.running.lock().await.keys().cloned().collect();
                    let db = state.db.clone();
                    drop(state);
                    let Ok(conn) = db.lock() else { return };
                    proxy_manager::cleanup_all_extensions(&running, &conn);
                });
            }
        });
}
