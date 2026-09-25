use crate::browser::{detector::BrowserInfo, launcher};
use crate::commands::profile::{db_lock, get_profile_by_id, poisoned};
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use crate::proxy_manager;
use crate::AppState;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub profile_id: String,
    pub success: bool,
    pub message: String,
}

/// Registry entry for one running browser (architecture recommendation A1).
///
/// The watcher task exclusively owns the `tokio Child`; the registry stores a
/// control channel into that watcher plus an exit signal. Stop therefore
/// flows through the process that owns the handle:
///   1. `stop_tx` → watcher sends the graceful stop (SIGTERM / taskkill),
///   2. waits up to the profile's grace period,
///   3. escalates via `Child::start_kill()` — cross-platform, in-process,
///      no `kill -9` / `taskkill /F` subprocess parsing.
/// `done_rx` lets `do_stop` await the actual exit without polling /proc.
#[derive(Clone)]
pub struct RunningProc {
    pub pid: u32,
    stop_tx: mpsc::UnboundedSender<()>,
    done_rx: watch::Receiver<bool>,
}

/// Default graceful-stop window when the profile doesn't set one.
const DEFAULT_STOP_TIMEOUT_SECS: i64 = 3;
/// Max automatic restarts per launch session (crash-recovery policy).
const MAX_CRASH_RESTARTS: u32 = 3;

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
            AppError::not_found(format!(
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
            return Err(AppError::validation(format!(
                "A browser for this profile appears to be already running (pid {pid}). Stop it first or kill the process."
            )));
        }
        // Stale lock from a crashed browser — safe to remove.
        let _ = std::fs::remove_file(user_dir.join("SingletonLock"));
    }

    // Per-profile extra arguments (feature 1), validated at save time and
    // re-validated here so a hand-edited DB cannot inject blocked flags.
    let extra_args = launcher::validate_extra_args(profile.extra_args.as_deref().unwrap_or(""))?;

    Ok(launcher::LaunchSpec {
        executable_path,
        user_data_dir: profile.user_data_dir.clone(),
        proxy_server,
        extension_path,
        window_class: Some(launcher::window_app_id(&profile.id, &profile.name)),
        extra_args,
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

/// Core launch logic shared by commands, CLI actions, the tray and the crash
/// supervisor. `restart_attempt` > 0 means this call comes from the automatic
/// crash-restart policy and must not recurse forever.
///
/// Returns a boxed future on purpose: the crash supervisor (spawned by this
/// function) relaunches through it again — without the type erasure the two
/// futures would reference each other and the compiler hits an opaque-type
/// cycle.
pub fn do_launch_with_restarts(
    app: AppHandle,
    profile_id: String,
    restart_attempt: u32,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = LaunchResult> + Send>> {
    Box::pin(do_launch_inner(app, profile_id, restart_attempt))
}

pub async fn do_launch(app: AppHandle, profile_id: String) -> LaunchResult {
    do_launch_with_restarts(app, profile_id, 0).await
}

async fn do_launch_inner(
    app: AppHandle,
    profile_id: String,
    restart_attempt: u32,
) -> LaunchResult {
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

    // Register before any bookkeeping so a fast exit is still stoppable and
    // status queries see it.
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = watch::channel(false);
    state.running.lock().await.insert(
        profile_id.clone(),
        RunningProc { pid, stop_tx, done_rx },
    );

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

    // Notify UI + tray (covers programmatic/CLI launches too).
    crate::sync::after_profile_change(&app, &profile_id);

    // Background supervisor: owns the child, detects exit instantly, handles
    // stop requests and the crash-restart policy (feature 4).
    let watcher_app = app.clone();
    let watcher_id = profile_id.clone();
    let watcher_profile = profile;
    tauri::async_runtime::spawn(supervise(
        watcher_app,
        watcher_id,
        child,
        stop_rx,
        done_tx,
        watcher_profile.restart_on_crash,
        watcher_profile.stop_timeout_secs,
        restart_attempt,
    ));

    LaunchResult {
        profile_id,
        success: true,
        message: format!("Launched (pid {pid})"),
    }
}

/// Owns the browser process for as long as it runs.
///
/// Exit paths:
/// - the process exits on its own (clean or crashed),
/// - a stop request arrives → graceful stop (profile's `stop_timeout_secs`,
///   default 3 s) → escalate via `Child::start_kill()`.
///
/// After exit: registry cleanup, DB status/history bookkeeping, extension
/// sweep, `profile-stopped` event — and, when the profile opted into the
/// restart policy and the exit was NOT user-initiated, an automatic relaunch
/// (bounded by `MAX_CRASH_RESTARTS` to stop crash loops).
#[allow(clippy::too_many_arguments)]
async fn supervise(
    app: AppHandle,
    profile_id: String,
    mut child: tokio::process::Child,
    mut stop_rx: mpsc::UnboundedReceiver<()>,
    done_tx: watch::Sender<bool>,
    restart_on_crash: bool,
    stop_timeout_secs: Option<i64>,
    restart_attempt: u32,
) {
    let state = app.state::<AppState>();
    let mut intentional_stop = false;

    let wait_result = loop {
        tokio::select! {
            result = child.wait() => break result,
            _ = stop_rx.recv() => {
                intentional_stop = true;
                // Graceful shutdown first: SIGTERM (Unix) / window-close
                // request (Windows) lets Chromium flush session data; a hard
                // kill causes the "Restore pages?" bubble on next launch.
                send_signal(child.id().unwrap_or(0), "TERM").await;
                let grace = std::time::Duration::from_secs(
                    stop_timeout_secs
                        .unwrap_or(DEFAULT_STOP_TIMEOUT_SECS)
                        .clamp(1, 60) as u64,
                );
                match tokio::time::timeout(grace, child.wait()).await {
                    Ok(result) => break result,
                    Err(_) => {
                        // Escalate: in-process kill via the owned handle —
                        // cross-platform, no signal subprocesses.
                        let _ = child.start_kill();
                        break child.wait().await;
                    }
                }
            }
        }
    };

    // Signal waiters (do_stop) and drop the registry entry first so status
    // checks converge immediately.
    let _ = done_tx.send(true);
    state.running.lock().await.remove(&profile_id);

    if let Err(e) = wait_result.as_ref() {
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
    crate::sync::after_profile_change(&app, &profile_id);

    // Crash-restart policy (feature 4): unexpected, non-zero exits of a
    // profile with `restart_on_crash` are relaunched automatically, up to
    // MAX_CRASH_RESTARTS per launch session (the counter carries over each
    // restart, so a persistent crash loop stops after the cap).
    let crashed = !wait_result.map(|s| s.success()).unwrap_or(false);
    if !intentional_stop && restart_on_crash && crashed && restart_attempt < MAX_CRASH_RESTARTS {
        eprintln!(
            "mbm: profile {profile_id} exited unexpectedly — restarting ({}/{} allowed)",
            restart_attempt + 1,
            MAX_CRASH_RESTARTS
        );
        // Spawned as its own task (instead of awaited here) to break the
        // recursive future type: supervise's future would otherwise embed
        // do_launch's future, which spawns supervise, … — the compiler
        // cannot size (or prove Send for) such a cycle.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let relaunch =
                do_launch_with_restarts(app.clone(), profile_id.clone(), restart_attempt + 1)
                    .await;
            if !relaunch.success {
                eprintln!("mbm: auto-restart of {profile_id} failed: {}", relaunch.message);
                let _ = app.emit(
                    "profile-launch-failed",
                    serde_json::json!({ "profileId": profile_id, "reason": relaunch.message }),
                );
            }
        });
    }
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
        Err(AppError::internal(result.message))
    }
}

/// Core stop logic shared by commands and CLI actions.
pub async fn do_stop(app: AppHandle, id: String) -> AppResult<()> {
    let state = app.state::<AppState>();

    // Same per-profile lock as do_launch: a stop racing an in-flight launch
    // must not kill a freshly spawned browser or converge status wrongly.
    let _action_guard = acquire_profile_action_lock(&state, &id).await;

    // Tracked path (architecture A1): tell the supervisor to stop, then await
    // the exit signal. The supervisor performs graceful stop + escalation
    // itself; a generous ceiling here covers the longest allowed grace
    // period (60 s) plus kill time.
    let running = state.running.lock().await.get(&id).cloned();
    if let Some(proc) = running {
        let _ = proc.stop_tx.send(());
        let mut done = proc.done_rx.clone();
        let ceiling = std::time::Duration::from_secs(75);
        if tokio::time::timeout(ceiling, done.wait_for(|v| *v)).await.is_err() {
            eprintln!("mbm: stop of profile {id} timed out; browser may still be exiting");
        }
        return Ok(());
    }

    // Not tracked as running — but the browser might still be alive from
    // a previous MBM session (orphaned process with a SingletonLock).
    // Scope the DB lock so it is never held across the wait loop below.
    let user_data_dir = {
        let conn = db_lock(&state)?;
        get_profile_by_id(&conn, &id)?.user_data_dir
    };

    let mut killed_orphan = false;
    if let Some(pid) = launcher::singleton_pid(std::path::Path::new(&user_data_dir)) {
        if launcher::pid_alive(pid) && launcher::pid_cmdline_contains(pid, &user_data_dir) {
            terminate_pid_gracefully(pid).await;
            killed_orphan = true;
        }
        let _ =
            std::fs::remove_file(std::path::Path::new(&user_data_dir).join("SingletonLock"));
    }

    {
        let conn = db_lock(&state)?;
        conn.execute(
            "UPDATE profiles SET status = 'stopped' WHERE id = ?1",
            params![id],
        )?;
        if killed_orphan {
            // The watcher is gone (previous session); close the open history
            // entry ourselves so usage stats stay correct.
            let now = chrono::Utc::now().timestamp();
            let _ = conn.execute(
                "UPDATE launch_history SET closed_at = ?1 WHERE profile_id = ?2 AND closed_at IS NULL",
                params![now, id],
            );
        }
    }
    crate::sync::after_profile_change(&app, &id);
    Ok(())
}

/// TERM → wait up to 3 s → hard kill, for a pid we track only via the OS
/// (orphaned browsers from a previous session). Non-blocking: uses async
/// subprocesses and sleep.
async fn terminate_pid_gracefully(pid: u32) {
    send_signal(pid, "TERM").await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while launcher::pid_alive(pid) && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    if launcher::pid_alive(pid) {
        send_signal(pid, "KILL").await;
    }
}

/// Sends an OS-level stop signal to a pid we do NOT own a Child handle for
/// (orphaned browsers). Tracked processes never use this path: their stop
/// goes through the supervisor's channel and `Child::start_kill`.
///
/// Unix: `kill -<signal>`. Windows: "TERM" maps to `taskkill /PID x`
/// (graceful close request), "KILL" maps to `taskkill /PID x /T /F`.
async fn send_signal(pid: u32, signal: &str) {
    #[cfg(unix)]
    {
        let _ = tokio::process::Command::new("kill")
            .args([format!("-{signal}"), pid.to_string()])
            .output()
            .await;
    }
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("taskkill");
        cmd.arg("/PID").arg(pid.to_string());
        if signal == "KILL" || signal == "FORCE" {
            cmd.args(["/T", "/F"]);
        }
        let _ = cmd.output().await;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (pid, signal);
    }
}

#[tauri::command]
pub async fn stop_profile(app: AppHandle, id: String) -> AppResult<()> {
    do_stop(app, id).await
}

#[tauri::command]
pub async fn bulk_launch(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<LaunchResult>> {
    run_bulk(app, ids, "launch").await
}

#[tauri::command]
pub async fn bulk_stop(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<LaunchResult>> {
    run_bulk(app, ids, "stop").await
}

/// Runs one action over many profiles, emitting `bulk-progress` after each so
/// the UI can show "3/10 launched" instead of a silent busy spinner.
/// Deliberately serial: parallel launches would storm the keyring and spawn
/// dozens of browsers at once.
async fn run_bulk(app: AppHandle, ids: Vec<String>, action: &str) -> AppResult<Vec<LaunchResult>> {
    let total = ids.len();
    let mut results = Vec::with_capacity(total);
    for (i, id) in ids.into_iter().enumerate() {
        let result = match action {
            "launch" => do_launch(app.clone(), id).await,
            _ => match do_stop(app.clone(), id.clone()).await {
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
            },
        };
        let _ = app.emit(
            "bulk-progress",
            serde_json::json!({
                "action": action,
                "done": i + 1,
                "total": total,
                "profileId": result.profile_id,
                "success": result.success,
            }),
        );
        results.push(result);
    }
    Ok(results)
}

/// Per-profile resource usage (feature 2) for the running browsers.
/// Linux-only: on other platforms the list is empty and the UI hides the
/// panel. See `services::resources` for the /proc mechanics.
#[tauri::command]
pub async fn get_resource_usage(app: AppHandle) -> AppResult<Vec<crate::services::resources::ResourceUsage>> {
    #[cfg(not(unix))]
    {
        let _ = app;
        Ok(Vec::new())
    }
    #[cfg(unix)]
    {
        let state = app.state::<AppState>();
        let running: Vec<(String, u32)> = state
            .running
            .lock()
            .await
            .iter()
            .map(|(id, p)| (id.clone(), p.pid))
            .collect();
        Ok(crate::services::resources::compute_usage(&state, running))
    }
}

/// Aggregate usage statistics per profile (feature 5).
#[tauri::command]
pub fn get_usage_stats(state: State<'_, AppState>) -> AppResult<Vec<crate::services::usage::UsageStat>> {
    let conn = db_lock(&state)?;
    crate::services::usage::usage_stats(&conn)
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
