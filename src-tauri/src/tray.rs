//! Tray menu + debounced rebuild worker.
//!
//! L8: every profile mutation used to spawn a fresh OS thread and rebuild the
//! whole menu; bursts of CRUD actions queued duplicate rebuilds against the
//! DB lock. Now `schedule_rebuild` feeds a single long-lived worker through a
//! bounded channel — bursts coalesce — and a menu-signature check skips the
//! GTK round-trip when nothing actually changed (launch/stop/status events
//! fan in through the same path thanks to A2).

use std::sync::OnceLock;
use std::sync::mpsc::{SyncSender, TrySendError};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::error::AppError;

pub const TRAY_ID: &str = "mbm-tray";

static WORKER: OnceLock<SyncSender<()>> = OnceLock::new();

/// Request a tray rebuild. Cheap and non-blocking: bursts are coalesced by
/// the single worker thread. Safe to call from any thread/state context.
pub fn schedule_rebuild(app: &AppHandle) {
    let sender = WORKER.get_or_init(|| spawn_worker(app.clone()));
    match sender.try_send(()) {
        Ok(()) => {}
        // Channel full = a rebuild is already queued; the worker drains
        // everything that arrived before it starts, so nothing is lost.
        Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
    }
}

/// One worker thread for the app's whole lifetime: wakes on a request, lets
/// a short burst settle, then rebuilds at most once — skipping the rebuild
/// entirely when the menu signature is unchanged.
fn spawn_worker(app: AppHandle) -> SyncSender<()> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<()>(1);
    std::thread::spawn(move || {
        let mut last_signature: Option<String> = None;
        while rx.recv().is_ok() {
            // Coalesce everything that piled up while we were idle or busy.
            while rx.try_recv().is_ok() {}
            std::thread::sleep(std::time::Duration::from_millis(150));
            while rx.try_recv().is_ok() {}
            match rebuild_tray_inner(&app, last_signature.as_deref()) {
                Ok(Some(signature)) => last_signature = Some(signature),
                Ok(None) => {} // menu unchanged — nothing rebuilt
                Err(e) => eprintln!("mbm: tray unavailable ({e})"),
            }
        }
    });
    tx
}

/// Stable fingerprint of everything the tray displays. A matching signature
/// means "menu is already correct" and skips the rebuild.
fn menu_signature(profiles: &[crate::models::profile::Profile]) -> String {
    let mut parts: Vec<String> = profiles
        .iter()
        .map(|p| format!("{}:{}:{}", p.id, p.name, if p.status == "running" { "R" } else { "S" }))
        .collect();
    parts.sort();
    parts.join("|")
}

fn rebuild_tray_inner(app: &AppHandle, last_signature: Option<&str>) -> Result<Option<String>, AppError> {
    let profiles = {
        let state = app.state::<crate::AppState>();
        let Ok(conn) = state.db.lock() else {
            return Err(AppError::internal("database state poisoned"));
        };
        crate::commands::profile::list_profiles(&conn)?
    };

    let signature = menu_signature(&profiles);
    if Some(signature.as_str()) == last_signature {
        return Ok(None); // nothing changed — skip the GTK menu rebuild
    }

    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
    let stop_all = MenuItem::with_id(
        app,
        "stop_all",
        "Stop all browsers",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit MBM", true, None::<&str>)?;

    let mut profile_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for p in &profiles {
        let marker = if p.status == "running" { "●" } else { "○" };
        // One failing item (e.g. a name Tauri rejects as an id) must not drop
        // the rest of the submenu — skip it and keep going.
        if let Ok(item) = MenuItem::with_id(
            app,
            &format!("profile:{}", p.id),
            format!("{marker}  {}", p.name),
            true,
            None::<&str>,
        ) {
            profile_items.push(item);
        }
    }

    let profiles_item: Submenu<tauri::Wry> = if profile_items.is_empty() {
        let item = MenuItem::with_id(app, "no_profiles", "No profiles yet", false, None::<&str>)?;
        let sub = Submenu::with_id(app, "profiles", "Profiles", true)?;
        sub.append(&item)?;
        sub
    } else {
        let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = profile_items
            .iter()
            .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
            .collect();
        Submenu::with_items(app, "Profiles", true, &refs)?
    };

    let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    menu_items.push(&show);
    let sep1 = PredefinedMenuItem::separator(app)?;
    menu_items.push(&sep1);
    menu_items.push(&profiles_item);
    let sep2 = PredefinedMenuItem::separator(app)?;
    menu_items.push(&sep2);
    menu_items.push(&stop_all);
    let sep3 = PredefinedMenuItem::separator(app)?;
    menu_items.push(&sep3);
    menu_items.push(&quit);

    let menu = Menu::with_items(app, &menu_items)?;

    match app.tray_by_id(TRAY_ID) {
        Some(tray) => {
            let _ = tray.set_menu(Some(menu));
        }
        None => {
            // First build: a missing/broken libappindicator shows up here as an
            // Err (returned), not a panic.
            let builder = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Multi Browser Manager")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()));

            let builder = if let Some(icon) = app.default_window_icon() {
                builder.icon(icon.clone())
            } else {
                builder
            };

            builder.on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(tray.app_handle());
                }
            }).build(app)?;
        }
    }
    Ok(Some(signature))
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => show_window(app),
        "quit" => app.exit(0),
        // Feature 7: "stop all" straight from the tray — completes the CLI's
        // capabilities for long farming sessions.
        "stop_all" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<crate::AppState>();
                let ids: Vec<String> = state.running.lock().await.keys().cloned().collect();
                drop(state);
                for id in ids {
                    if let Err(e) = crate::commands::process::do_stop(app.clone(), id.clone()).await {
                        eprintln!("mbm: tray stop-all failed for {id}: {e}");
                    }
                }
            });
        }
        other => {
            if let Some(profile_id) = other.strip_prefix("profile:") {
                let app = app.clone();
                let profile_id = profile_id.to_string();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<crate::AppState>();
                    let running = {
                        let conn = match state.db.lock() {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        conn.query_row(
                            "SELECT status FROM profiles WHERE id = ?1",
                            rusqlite::params![profile_id],
                            |r| r.get::<_, String>(0),
                        )
                        .unwrap_or_else(|_| "stopped".into())
                            == "running"
                    };
                    drop(state);
                    let result = if running {
                        crate::commands::process::do_stop(app.clone(), profile_id.clone())
                            .await
                            .err()
                    } else {
                        let r =
                            crate::commands::process::do_launch(app.clone(), profile_id.clone())
                                .await;
                        if r.success {
                            None
                        } else {
                            Some(AppError::internal(r.message))
                        }
                    };
                    if let Some(e) = result {
                        eprintln!("mbm: tray profile action failed: {e}");
                    }
                });
            }
        }
    }
}
