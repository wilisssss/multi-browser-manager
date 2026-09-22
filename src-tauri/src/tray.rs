use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;

pub const TRAY_ID: &str = "mbm-tray";

/// (Re)builds the tray menu with the current profile list.
/// Called at startup and whenever profiles are created/deleted/imported.
///
/// Clicking a profile in the menu toggles it: running → stop, stopped → launch.
pub fn rebuild_tray(app: &AppHandle) {
    let app = app.clone();
    // Rebuild on a detached background thread. Callers (create/update/delete/
    // duplicate) may still hold the non-reentrant DB mutex when they call us —
    // locking it again on the calling thread would deadlock, and on the main
    // thread it freezes the entire window (this is exactly the bug where
    // "duplicate did nothing and Mod+Q stopped working"). Detached means we
    // return immediately and the tray thread waits for the lock like everyone
    // else.
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rebuild_tray_inner(&app)));
        if result.is_err() {
            eprintln!("mbm: tray unavailable (is libayatana-appindicator installed?)");
        }
    });
}

fn rebuild_tray_inner(app: &AppHandle) {
    let profiles = {
        let state = app.state::<crate::AppState>();
        let Ok(conn) = state.db.lock() else { return };
        crate::commands::profile::list_profiles(&conn).unwrap_or_default()
    };

    let show = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>).ok();
    let quit = MenuItem::with_id(app, "quit", "Quit MBM", true, None::<&str>).ok();

    let mut profile_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for p in &profiles {
        let marker = if p.status == "running" { "●" } else { "○" };
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
        let disabled = MenuItem::with_id(app, "no_profiles", "No profiles yet", false, None::<&str>);
        match (Submenu::with_id(app, "profiles", "Profiles", true), disabled) {
            (Ok(sub), Ok(item)) => {
                let _ = sub.append(&item);
                sub
            }
            _ => return,
        }
    } else {
        let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = profile_items
            .iter()
            .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
            .collect();
        match Submenu::with_items(app, "Profiles", true, &refs) {
            Ok(sub) => sub,
            Err(_) => return,
        }
    };

    let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    if let Some(ref s) = show {
        menu_items.push(s);
    }
    let sep1 = PredefinedMenuItem::separator(app).ok();
    if let Some(ref s) = sep1 {
        menu_items.push(s);
    }
    menu_items.push(&profiles_item);
    let sep2 = PredefinedMenuItem::separator(app).ok();
    if let Some(ref s) = sep2 {
        menu_items.push(s);
    }
    if let Some(ref q) = quit {
        menu_items.push(q);
    }

    let menu = match Menu::with_items(app, &menu_items) {
        Ok(m) => m,
        Err(_) => return,
    };

    match app.tray_by_id(TRAY_ID) {
        Some(tray) => {
            let _ = tray.set_menu(Some(menu));
        }
        None => {
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

            if let Err(e) = builder.on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    show_window(tray.app_handle());
                }
            }).build(app) {
                eprintln!("mbm: failed to create tray icon: {e}");
            }
        }
    }
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
                            Some(AppError::Internal(r.message))
                        }
                    };
                    if let Some(e) = result {
                        eprintln!("mbm: tray profile action failed: {e}");
                    }
                    let _ = app.emit("profiles-changed", &profile_id);
                });
            }
        }
    }
}
