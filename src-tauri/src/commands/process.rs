use crate::browser::{detector::BrowserInfo, launcher};
use crate::commands::profile::{db_lock, get_profile_by_id, poisoned};
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use crate::proxy_manager;
use crate::AppState;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub profile_id: String,
    pub success: bool,
    pub message: String,
}

/// Fast DB-only part of a launch: fetch profile + proxy row. Deliberately does
/// NOT touch the keyring, the filesystem, or subprocesses — those happen
/// outside the DB lock (see build_launch_spec).
fn load_launch_context(
    conn: &rusqlite::Connection,
    profile_id: &str,
) -> AppResult<(Profile, Option<(String, String, i64, Option<String>)>)> {
    let profile = get_profile_by_id(conn, profile_id)?;
    let proxy_row = conn
        .query_row(
            "SELECT protocol, host, port, username FROM proxies WHERE id = ?1",
            params![profile.proxy_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .ok(); // None if no proxy assigned
    Ok((profile, proxy_row))
}

/// Slow, non-DB part of a launch: keyring read, extension generation and
/// browser detection. All inputs/outputs are owned; safe to run on a blocking
/// thread so the async runtime and the DB lock are never blocked by
/// gnome-keyring / `--version` subprocess probing.
fn build_launch_spec_blocking(
    state: &AppState,
    profile: &Profile,
    proxy_row: Option<(String, String, i64, Option<String>)>,
) -> AppResult<launcher::LaunchSpec> {
    let (proxy_server, extension_path) = match proxy_row {
        None => (None, None),
        Some((protocol, host, port, username)) => {
            proxy_manager::ensure_valid_protocol(&protocol)?;
            match username.filter(|u| !u.is_empty()) {
                Some(username) => {
                    let password = proxy_manager::get_proxy_password(profile.proxy_id.as_deref().unwrap_or(""))?;
                    let ext = proxy_manager::generate_auth_proxy_extension(
                        &profile.id, &protocol, &host, port, &username, &password,
                    )?;
                    (None, Some(ext.to_string_lossy().to_string()))
                }
                None => (
                    Some(format!("{protocol}://{host}:{port}")),
                    None,
                ),
            }
        }
    };

    let executable_path = detect_executable_for(state, &profile.browser_type)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "No installed browser found for type '{}'",
                profile.browser_type
            ))
        })?
        .executable_path;

    // Chromium enforces one process per user-data-dir: launching when an
    // instance is already alive just hands the window to that process and our
    // child exits instantly, leaving a phantom "stopped" status. Detect the
    // SingletonLock and refuse (or clean a stale one) instead.
    let user_dir = std::path::Path::new(&profile.user_data_dir);
    if let Some(pid) = launcher::singleton_pid(user_dir) {
        if launcher::pid_alive(pid) {
            return Err(AppError::Validation(format!(
                "A browser for this profile appears to be already running (pid {pid}). Stop it first or kill the process."
            )));
        }
        // Stale lock from a crashed browser — safe to remove.
        let _ = std::fs::remove_file(user_dir.join("SingletonLock"));
    }

    Ok(launcher::LaunchSpec {
        executable_path,
        user_data_dir: profile.user_data_dir.clone(),
        proxy_server,
        extension_path,
        window_class: Some(launcher::window_app_id(&profile.id, &profile.name)),
    })
}

/// Resolves the executable path for a browser type, using a process-lifetime
/// cache so repeated launches don't re-probe the filesystem.
fn detect_executable_for(state: &AppState, browser_type: &str) -> Option<BrowserInfo> {
    if let Ok(cache) = state.browser_cache.lock() {
        if let Some(path) = cache.get(browser_type) {
            return Some(BrowserInfo {
                browser_type: browser_type.to_string(),
                name: browser_type.to_string(),
                executable_path: path.clone(),
                version: None,
            });
        }
    }

    let found = crate::browser::detector::detect_browsers()
        .into_iter()
        .find(|b| b.browser_type == browser_type);

    if let Some(info) = &found {
        if let Ok(mut cache) = state.browser_cache.lock() {
            cache.insert(browser_type.to_string(), info.executable_path.clone());
        }
    }

    found
}

/// Core launch logic shared by commands and CLI actions.
pub async fn do_launch(app: AppHandle, profile_id: String) -> LaunchResult {
    let state = app.state::<AppState>();

    // Serialize launch/stop per profile: two concurrent triggers (tray click +
    // `mbm --launch` via single-instance) must not both pass the status check
    // and double-spawn. The guard is held for the whole operation.
    let _action_guard = acquire_profile_action_lock(&state, &profile_id).await;

    // 1) Fast DB part — never holds the lock across slow work.
    let (profile, proxy_row) = {
        let conn = match state.db.lock() {
            Ok(c) => c,
            Err(_) => {
                return LaunchResult {
                    profile_id,
                    success: false,
                    message: poisoned().to_string(),
                }
            }
        };
        match load_launch_context(&conn, &profile_id) {
            Ok((p, proxy)) => {
                if p.status == "running" {
                    return LaunchResult {
                        profile_id,
                        success: false,
                        message: "Profile is already running".into(),
                    };
                }
                (p, proxy)
            }
            Err(e) => {
                return LaunchResult { profile_id, success: false, message: e.to_string() }
            }
        }
    };

    // 2) Slow part (keyring, extension generation, browser detection) runs on
    //    a blocking thread — outside the DB lock and off the async workers.
    let spec_state = state.inner().clone();
    let spec_profile = profile.clone();
    let spec = match tauri::async_runtime::spawn_blocking(move || {
        build_launch_spec_blocking(&spec_state, &spec_profile, proxy_row)
    })
    .await
    {
        Ok(Ok(spec)) => spec,
        Ok(Err(e)) => {
            return LaunchResult { profile_id, success: false, message: e.to_string() }
        }
        Err(e) => {
            return LaunchResult {
                profile_id,
                success: false,
                message: format!("Internal task error: {e}"),
            }
        }
    };

    launcher::ensure_user_data_dir(std::path::Path::new(&profile.user_data_dir)).ok();

    let child = match launcher::spawn(&spec) {
        Ok(c) => c,
        Err(e) => {
            let _ = app.emit(
                "profile-launch-failed",
                serde_json::json!({ "profileId": profile_id, "reason": e.to_string() }),
            );
            return LaunchResult { profile_id, success: false, message: e.to_string() };
        }
    };

    let pid = child.id().unwrap_or(0);
    state.running.lock().await.insert(profile_id.clone(), pid);

    let now = chrono::Utc::now().timestamp();
    {
        let conn = db_lock(&state).ok();
        if let Some(conn) = conn {
            if let Err(e) = conn.execute(
                "UPDATE profiles SET status = 'running', last_used_at = ?1 WHERE id = ?2",
                params![now, profile_id],
            ) {
                eprintln!("mbm: failed to mark profile running: {e}");
            }
            if let Err(e) = conn.execute(
                "INSERT INTO launch_history (profile_id, launched_at, pid) VALUES (?1, ?2, ?3)",
                params![profile_id, now, pid],
            ) {
                eprintln!("mbm: failed to record launch history: {e}");
            }
        }
    }

    // Notify the frontend (covers programmatic/CLI launches too).
    let _ = app.emit("profiles-changed", &profile_id);

    // Background watcher: owns the child and detects its exit instantly.
    let watcher_app = app.clone();
    let watcher_id = profile_id.clone();
    tauri::async_runtime::spawn(async move {
        watch_child(watcher_app, child, watcher_id).await;
    });

    LaunchResult {
        profile_id,
        success: true,
        message: format!("Launched (pid {pid})"),
    }
}

/// Owns the child process and awaits its exit. Replaces the old 500 ms polling
/// loop: the exit is detected the instant it happens, with zero idle wakeups,
/// and there is exactly one task per browser only while it runs.
async fn watch_child(app: AppHandle, mut child: tokio::process::Child, profile_id: String) {
    let state = app.state::<AppState>();
    let wait_result = child.wait().await;

    // Remove from the pid registry before doing any cleanup, so do_stop's
    // grace-poll sees the browser as gone as soon as it has exited.
    state.running.lock().await.remove(&profile_id);

    if let Err(e) = wait_result {
        eprintln!("mbm: waiting for browser of profile {profile_id} failed: {e}");
    }

    let now = chrono::Utc::now().timestamp();
    {
        let conn = db_lock(&state).ok();
        if let Some(conn) = conn {
            if let Err(e) = conn.execute(
                "UPDATE profiles SET status = 'stopped' WHERE id = ?1 AND status = 'running'",
                params![profile_id],
            ) {
                eprintln!("mbm: failed to mark profile stopped: {e}");
            }
            if let Err(e) = conn.execute(
                "UPDATE launch_history SET closed_at = ?1 WHERE profile_id = ?2 AND closed_at IS NULL",
                params![now, profile_id],
            ) {
                eprintln!("mbm: failed to close launch history entry: {e}");
            }
        }
    }
    proxy_manager::cleanup_extension(&profile_id);
    let _ = app.emit("profile-stopped", &profile_id);
}

/// Returns (and lazily creates) the per-profile operation lock, then waits for
/// exclusive access. Every launch/stop of the same profile is serialized.
async fn acquire_profile_action_lock(
    state: &AppState,
    profile_id: &str,
) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let mut locks = state
            .profile_locks
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        locks.entry(profile_id.to_string()).or_default().clone()
    };
    lock.lock_owned().await
}

#[tauri::command]
pub async fn launch_profile(app: AppHandle, id: String) -> AppResult<LaunchResult> {
    let result = do_launch(app, id).await;
    if result.success {
        Ok(result)
    } else {
        Err(AppError::Internal(result.message))
    }
}

/// Core stop logic shared by commands and CLI actions.
pub async fn do_stop(app: AppHandle, id: String) -> AppResult<()> {
    let state = app.state::<AppState>();

    // Same per-profile lock as do_launch: a stop racing an in-flight launch
    // must not kill a freshly spawned browser or converge status wrongly.
    let _action_guard = acquire_profile_action_lock(&state, &id).await;

    // The watcher task owns the Child and performs all cleanup as soon as the
    // process exits; the registry only stores the pid.
    let pid = state.running.lock().await.get(&id).copied();

    if let Some(pid) = pid {
        // Graceful shutdown first: SIGTERM lets Chromium flush session data
        // (a hard SIGKILL causes the "Restore pages?" bubble on next launch).
        #[cfg(unix)]
        {
            send_signal(pid as i32, "TERM").await;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                if !state.running.lock().await.contains_key(&id) {
                    break; // exited; watcher already removed it and cleaned up
                }
                if std::time::Instant::now() >= deadline {
                    send_signal(pid as i32, "KILL").await;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        // Windows has no POSIX signals — taskkill /F is the only option.
        #[cfg(windows)]
        send_signal(pid, "FORCE").await;

        return Ok(());
    }

    // Not tracked as running — but the browser might still be alive from
    // a previous MBM session (orphaned process with a SingletonLock).
    // Scope the DB lock so it is never held across the wait loop below.
    let user_data_dir = {
        let conn = db_lock(&state)?;
        get_profile_by_id(&conn, &id)?.user_data_dir
    };

    if let Some(pid) = launcher::singleton_pid(std::path::Path::new(&user_data_dir)) {
        if launcher::pid_alive(pid)
            && launcher::pid_cmdline_contains(pid, &user_data_dir)
        {
            terminate_pid_gracefully(pid).await;
        }
        let _ =
            std::fs::remove_file(std::path::Path::new(&user_data_dir).join("SingletonLock"));
    }

    let conn = db_lock(&state)?;
    conn.execute(
        "UPDATE profiles SET status = 'stopped' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// SIGTERM → wait up to 3 s → SIGKILL, for a pid we track only via /proc
/// (orphaned browsers). Non-blocking: uses tokio's process API for signals and
/// async sleep for the wait loop.
async fn terminate_pid_gracefully(pid: i32) {
    send_signal(pid, "TERM").await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while launcher::pid_alive(pid) && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if launcher::pid_alive(pid) {
        send_signal(pid, "KILL").await;
    }
}

/// Sends a signal without blocking the async runtime (`std::process::Command
/// .output()` would block a worker thread for the whole wait).
async fn send_signal(pid: i32, signal: &str) {
    let _ = tokio::process::Command::new("kill")
        .args([format!("-{signal}"), pid.to_string()])
        .output()
        .await;
}

#[tauri::command]
pub async fn stop_profile(app: AppHandle, id: String) -> AppResult<()> {
    do_stop(app, id).await
}

#[tauri::command]
pub async fn bulk_launch(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<LaunchResult>> {
    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        results.push(do_launch(app.clone(), id).await);
    }
    Ok(results)
}

#[tauri::command]
pub async fn bulk_stop(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<LaunchResult>> {
    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        let result = match do_stop(app.clone(), id.clone()).await {
            Ok(()) => LaunchResult {
                profile_id: id,
                success: true,
                message: "Stopped".into(),
            },
            Err(e) => LaunchResult {
                profile_id: id,
                success: false,
                message: e.to_string(),
            },
        };
        results.push(result);
    }
    Ok(results)
}

#[tauri::command]
pub async fn get_running_profiles(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let map = state.running.lock().await;
    Ok(map.keys().cloned().collect())
}

/// One entry of launch history for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub profile_name: Option<String>,
    pub launched_at: i64,
    pub closed_at: Option<i64>,
    pub pid: Option<i64>,
}

#[tauri::command]
pub fn get_launch_history(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<HistoryEntry>> {
    let conn = db_lock(&state)?;
    let limit = limit.unwrap_or(50).clamp(1, 500);

    let mut stmt = conn.prepare(
        "SELECT lh.id, p.name, lh.launched_at, lh.closed_at, lh.pid
         FROM launch_history lh
         LEFT JOIN profiles p ON p.id = lh.profile_id
         ORDER BY lh.launched_at DESC
         LIMIT ?1",
    )?;

    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                profile_name: r.get(1)?,
                launched_at: r.get(2)?,
                closed_at: r.get(3)?,
                pid: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}
