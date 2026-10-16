use crate::browser::{detector::BrowserInfo, launcher};
use crate::commands::profile::{db_lock, get_profile_by_id, poisoned};
use crate::error::{AppError, AppResult};
use crate::models::profile::Profile;
use crate::proxy_manager;
use crate::AppState;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use std::sync::Arc;
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
///   2. waits up to the grace period,
///   3. escalates via `Child::start_kill()` — cross-platform, in-process,
///      no `kill -9` / `taskkill /F` subprocess parsing.
/// `done_rx` lets `do_stop` await the actual exit without polling /proc.
#[derive(Clone)]
pub struct RunningProc {
    pub pid: u32,
    stop_tx: mpsc::UnboundedSender<()>,
    done_rx: watch::Receiver<bool>,
}

/// Graceful-stop window before the browser is force-killed.
const STOP_TIMEOUT_SECS: u64 = 3;
/// Max automatic restarts per launch session (crash-recovery policy).
const MAX_CRASH_RESTARTS: u32 = 3;

/// Fast DB-only part of a launch: fetch profile + proxy row. Deliberately does
/// NOT touch the keyring, the filesystem, or subprocesses — those happen
/// outside the DB lock (see build_launch_spec).
fn load_launch_context(
    conn: &rusqlite::Connection,
    profile_id: &str,
) -> AppResult<(Profile, Option<(String, String, i64, Option<String>)>, bool)> {
    let profile = get_profile_by_id(conn, profile_id)?;
    let trim_memory = crate::commands::settings::load_settings(conn).lightweight_browsers;
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
    Ok((profile, proxy_row, trim_memory))
}

/// Slow, non-DB part of a launch: keyring read, extension generation and
/// browser detection. All inputs/outputs are owned; safe to run on a blocking
/// thread so the async runtime and the DB lock are never blocked by
/// gnome-keyring / `--version` subprocess probing.
fn build_launch_spec_blocking(
    state: &AppState,
    profile: &Profile,
    proxy_row: Option<(String, String, i64, Option<String>)>,
    trim_memory: bool,
) -> AppResult<launcher::LaunchSpec> {
    let mut extension_paths: Vec<String> = Vec::new();
    let mut proxy_timezone: Option<String> = None;
    // Set when the profile uses an authenticated proxy: the browser then
    // points at the local forwarder (port filled in at spawn time).
    let mut proxy_forwarder: Option<crate::browser::proxy_forwarder::UpstreamConfig> = None;
    let proxy_server_set = proxy_row.is_some();
    let (proxy_server, proxy_extension) = match proxy_row {
        None => (None, None),
        Some((protocol, host, port, username)) => {
            proxy_manager::ensure_valid_protocol(&protocol)?;
            let username = username.filter(|u| !u.is_empty());
            let password = match &username {
                Some(_) => Some(proxy_manager::get_proxy_password(profile.proxy_id.as_deref().unwrap_or(""))?),
                None => None,
            };
            // Only needed when the profile does not set its own timezone.
            let needs_tz = profile.fingerprint.as_ref().and_then(|f| f.timezone.as_ref()).is_none();
            if needs_tz {
                proxy_timezone = proxy_manager::lookup_timezone_via_proxy(
                    &protocol,
                    &host,
                    port,
                    username.as_deref(),
                    password.as_deref(),
                );
            }
            match username {
                Some(username) => {
                    // Route through the local forwarder instead of loading an
                    // auth extension: MV3 extension auth cannot reliably wake
                    // a sleeping service worker, so a 407 would silently kill
                    // connectivity. The forwarder injects credentials before
                    // anything leaves the machine.
                    proxy_forwarder = Some(crate::browser::proxy_forwarder::UpstreamConfig {
                        protocol: protocol,
                        host: host,
                        port: port as u16,
                        username: username,
                        password: password.unwrap_or_default(),
                    });
                    (None, None)
                }
                None => (Some(format!("{protocol}://{host}:{port}")), None),
            }
        }
    };
    extension_paths.extend(proxy_extension);

    let mut fingerprint = profile.fingerprint.clone().unwrap_or_default();
    // Timezone: an explicit profile setting wins; otherwise a proxied profile
    // reports the zone of its exit IP.
    if fingerprint.timezone.is_none() {
        if let Some(tz) = proxy_timezone.as_deref() {
            fingerprint.timezone = Some(tz.to_string());
        }
    }
    // Phone UA: viewport and screen follow the device (portrait).
    let phone = profile
        .fingerprint
        .as_ref()
        .and_then(|f| f.user_agent.as_deref())
        .and_then(crate::browser::user_agent::phone_screen_for_ua);
    if let Some((w, h)) = phone {
        fingerprint.screen = Some(crate::models::profile::Screen { width: w, height: h });
    }
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

    // The proxy extension persists its chrome.proxy config in the profile's
    // Preferences and Chromium keeps honouring it — outranking even
    // command-line proxy flags — after the extension is gone, so a removed or
    // replaced proxy would silently stay in effect. Scrub it here, while the
    // browser is guaranteed to be stopped; a launch that does use the proxy
    // extension lets it rewrite its config.
    launcher::scrub_stale_proxy_prefs(user_dir);

    Ok(launcher::LaunchSpec {
        executable_path,
        user_data_dir: profile.user_data_dir.clone(),
        proxy_server,
        extension_paths,
        proxy_forwarder,
        trim_memory,
        language: fingerprint.language.clone(),
        user_agent: fingerprint.user_agent.clone(),
        startup_url: profile.startup_url.clone(),
        window_size: phone,
        disable_webrtc: fingerprint.disable_webrtc,
        proxy_enabled: proxy_server_set,
        webrtc_proxy_only: proxy_server_set,
        env: fingerprint
            .timezone
            .as_ref()
            .map(|tz| vec![("TZ".to_string(), tz.clone())])
            .unwrap_or_default(),
        debug_port: Some(crate::browser::cdp::pick_debug_port()?),
    })
}

/// Identity applied over CDP once the browser is up. Mirrors the launch
/// flags so the page sees the same profile everywhere.
fn cdp_plan(profile: &Profile, spec: &launcher::LaunchSpec) -> AppResult<crate::browser::cdp::Plan> {
    use crate::browser::cdp::{ClientHints, Plan};
    let fp = profile.fingerprint.clone().unwrap_or_default();
    let phone = spec.window_size;
    let client_hints = fp.user_agent.as_deref().map(|ua| {
        let id = crate::browser::fingerprint::UaIdentity::from_ua(ua);
        ClientHints {
            platform: id.ua_platform.to_string(),
            platform_version: id.platform_version,
            architecture: id.architecture.to_string(),
            model: id.model,
            mobile: id.mobile,
            full_version: id.full_version,
        }
    });
    let init_script = crate::browser::fingerprint::script_with_seed(
        &fp,
        crate::browser::fingerprint::profile_seed(&profile.id),
    )?;
    Ok(Plan {
        init_script: Some(init_script),
        user_agent: spec.user_agent.clone(),
        client_hints,
        viewport: phone,
        mobile: phone.is_some(),
        timezone: spec.env.iter().find(|(k, _)| k == "TZ").map(|(_, v)| v.clone()),
        locale: fp.language.clone(),
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
    // and double-spawn.
    let result = {
        let _action_guard = acquire_profile_action_lock(&state, &profile_id).await;
        launch_locked(&app, profile_id.clone(), restart_attempt).await
    };
    release_profile_action_lock(&state, &profile_id);
    result
}

async fn launch_locked(
    app: &AppHandle,
    profile_id: String,
    restart_attempt: u32,
) -> LaunchResult {
    let state = app.state::<AppState>();

    // The registry is the source of truth for live processes; the DB `status`
    // can lag (crash, missed exit event) and must not block a relaunch.
    if state.running.lock().await.contains_key(&profile_id) {
        return LaunchResult {
            profile_id,
            success: false,
            message: "Profile is already running".into(),
        };
    }

    // 1) Fast DB part — never holds the lock across slow work.
    let (profile, proxy_row, trim_memory) = {
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
            Ok((p, proxy, trim)) => {
                if p.status == "running" {
                    return LaunchResult {
                        profile_id,
                        success: false,
                        message: "Profile is already running".into(),
                    };
                }
                (p, proxy, trim)
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
        build_launch_spec_blocking(&spec_state, &spec_profile, proxy_row, trim_memory)
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

    // 2.5) Authenticated proxy → start the local forwarder and point the
    // browser at it. No credentials ever reach the browser process.
    let mut spec = spec;
    let mut forwarder_guard = None;
    if let Some(upstream) = spec.proxy_forwarder.take() {
        match crate::browser::proxy_forwarder::spawn(upstream).await {
            Ok(guard) => {
                spec.proxy_server = Some(format!("http://127.0.0.1:{}", guard.local_port));
                forwarder_guard = Some(guard);
            }
            Err(e) => {
                return LaunchResult {
                    profile_id,
                    success: false,
                    message: format!("Failed to start local proxy forwarder: {e}"),
                }
            }
        }
    }

    let plan = match cdp_plan(&profile, &spec) {
        Ok(plan) => plan,
        Err(e) => return LaunchResult { profile_id, success: false, message: e.to_string() },
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

    // Identity over CDP: wait for the page, then apply viewport, UA, timezone
    // and the fingerprint script. Runs in the background so launch returns
    // immediately; failures are logged, not fatal.
    if let Some(port) = spec.debug_port {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::browser::cdp::apply(port, &plan).await {
                eprintln!("mbm: CDP identity for profile failed: {e}");
            }
        });
    }

    // Register before any bookkeeping so a fast exit is still stoppable and
    // status queries see it.
    let (stop_tx, stop_rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = watch::channel(false);
    state.running.lock().await.insert(
        profile_id.clone(),
        RunningProc { pid, stop_tx: stop_tx.clone(), done_rx },
    );
    if let Some(guard) = forwarder_guard {
        state.proxy_forwarders.lock().await.insert(profile_id.clone(), guard);
    }

    let now = chrono::Utc::now().timestamp();
    let db_result: AppResult<()> = match db_lock(&state) {
        Ok(conn) => conn
            .execute(
                "UPDATE profiles SET status = 'running', last_used_at = ?1 WHERE id = ?2",
                params![now, profile_id],
            )
            .and_then(|_| {
                conn.execute(
                    "INSERT INTO launch_history (profile_id, launched_at, pid) VALUES (?1, ?2, ?3)",
                    params![profile_id, now, pid],
                )
            })
            .map(|_| ())
            .map_err(AppError::from),
        Err(e) => Err(e),
    };
    // The supervisor must run in every case: it owns the child process. If the
    // status write failed, we ask it to stop the browser we just started (so
    // nothing is left running untracked) instead of returning and orphaning it.
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
        restart_attempt,
    ));

    if let Err(e) = db_result {
        eprintln!("mbm: failed to record launch for {profile_id}: {e}");
        let _ = stop_tx.send(());
        return LaunchResult { profile_id, success: false, message: e.to_string() };
    }

    // Notify UI + tray (covers programmatic/CLI launches too).
    crate::sync::after_profile_change(&app, &profile_id);

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
/// - a stop request arrives → graceful stop (`STOP_TIMEOUT_SECS`) → escalate
///   via `Child::start_kill()`.
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
    restart_attempt: u32,
) {
    let state = app.state::<AppState>();
    let mut intentional_stop = false;

    let wait_result = tokio::select! {
        result = child.wait() => result,
        _ = stop_rx.recv() => {
            intentional_stop = true;
            // Graceful shutdown first: SIGTERM (Unix) / window-close
            // request (Windows) lets Chromium flush session data; a hard
            // kill causes the "Restore pages?" bubble on next launch.
            send_signal(child.id().unwrap_or(0), "TERM").await;
            let grace = std::time::Duration::from_secs(STOP_TIMEOUT_SECS);
            match tokio::time::timeout(grace, child.wait()).await {
                Ok(result) => result,
                Err(_) => {
                    // Escalate: in-process kill via the owned handle —
                    // cross-platform, no signal subprocesses.
                    let _ = child.start_kill();
                    child.wait().await
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
    // Stop the profile's local proxy forwarder (if any) — the browser is gone.
    state
        .proxy_forwarders
        .lock()
        .await
        .remove(&profile_id); // Drop runs shutdown()
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

/// Drops a profile's operation lock entry once nobody else holds or waits on
/// it, so `profile_locks` does not grow with every profile ever touched.
fn release_profile_action_lock(state: &AppState, profile_id: &str) {
    let mut locks = state
        .profile_locks
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(lock) = locks.get(profile_id) {
        // strong_count == 1 means only the map holds it: no guard, no waiter.
        if Arc::strong_count(lock) == 1 {
            locks.remove(profile_id);
        }
    }
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
    let result = {
        let _action_guard = acquire_profile_action_lock(&state, &id).await;
        stop_locked(&app, &id).await
    };
    release_profile_action_lock(&state, &id);
    result
}

async fn stop_locked(app: &AppHandle, id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let id = id.to_string();

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
