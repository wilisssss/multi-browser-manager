use crate::error::AppResult;
use std::path::Path;
use tokio::process::{Child, Command};

/// Everything needed to spawn one browser instance.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub executable_path: String,
    pub user_data_dir: String,
    /// Set for proxies WITHOUT authentication: `http://host:port` or `socks5://host:port`.
    pub proxy_server: Option<String>,
    /// Generated unpacked extensions for this launch: proxy auth and/or the
    /// fingerprint overrides. Joined into one `--load-extension` flag.
    pub extension_paths: Vec<String>,
    /// `--lang` value from the profile's fingerprint.
    pub language: Option<String>,
    /// `--user-agent` value from the profile's fingerprint.
    pub user_agent: Option<String>,
    /// Page opened at start, passed as the last positional argument.
    pub startup_url: Option<String>,
    /// Viewport `WIDTHxHEIGHT` for `--window-size` (phone user agents).
    pub window_size: Option<(u32, u32)>,
    /// Route WebRTC only through the proxy, so the real LAN/public IP is not
    /// exposed through ICE candidates.
    pub webrtc_proxy_only: bool,
    /// Block WebRTC entirely (no ICE servers, no candidates). Overrides
    /// `webrtc_proxy_only`.
    pub disable_webrtc: bool,
    /// Extra environment variables for the browser process (e.g. `TZ`).
    pub env: Vec<(String, String)>,
    /// Local port for `--remote-debugging-port`, used to apply identity over CDP.
    pub debug_port: Option<u16>,
    /// Memory-trim ("lightweight") mode: append the curated RAM-saving flags
    /// (see `MEMORY_TRIM_FLAGS`). Driven by the global settings toggle.
    pub trim_memory: bool,
}

/// Builds the Chromium command-line argument list for a launch spec.
pub fn build_args(spec: &LaunchSpec) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", spec.user_data_dir),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];

    if let Some(proxy) = &spec.proxy_server {
        args.push(format!("--proxy-server={proxy}"));
    }

    // Chromium does not merge repeated --disable-features (the last one
    // wins), so all feature toggles are collected into a single flag.
    let mut features: Vec<&str> = Vec::new();
    if spec.trim_memory {
        features.extend(MEMORY_TRIM_FEATURES);
    }
    if !spec.extension_paths.is_empty() {
        // Unpacked extensions require a non-default --user-data-dir (we always
        // set one) and won't show the developer-mode bubble on a fresh dir.
        features.push("DisableLoadExtensionCommandLineSwitch");
    }
    if !features.is_empty() {
        args.push(format!("--disable-features={}", features.join(",")));
    }

    if spec.trim_memory {
        args.extend(MEMORY_TRIM_FLAGS.iter().map(|s| s.to_string()));
        args.push(format!(
            "--enable-features={}",
            MEMORY_TRIM_ENABLE_FEATURES.join(",")
        ));
    }

    if !spec.extension_paths.is_empty() {
        args.push(format!("--load-extension={}", spec.extension_paths.join(",")));
    }

    if let Some(lang) = &spec.language {
        args.push(format!("--lang={lang}"));
    }
    if let Some(ua) = &spec.user_agent {
        args.push(format!("--user-agent={ua}"));
    }
    if let Some((w, h)) = spec.window_size {
        args.push(format!("--window-size={w},{h}"));
    }
    if let Some(port) = spec.debug_port {
        args.push(format!("--remote-debugging-port={port}"));
    }
    if spec.disable_webrtc {
        // Belt and braces with the CDP script: never gather a non-proxied UDP
        // candidate, so even a page that ignores the JS shim sees no host IP.
        // The flag name has no `--force-` prefix; that spelling is ignored.
        args.push("--webrtc-ip-handling-policy=disable_non_proxied_udp".into());
    } else if spec.webrtc_proxy_only {
        args.push("--webrtc-ip-handling-policy=disable_non_proxied_udp".into());
    }

    // Positional URL last: Chromium opens it in the first window.
    if let Some(url) = &spec.startup_url {
        args.push(url.clone());
    }

    args
}

/// Feature toggles for the memory-trim ("lightweight") launch mode. Merged
/// into the single `--disable-features` flag by `build_args`.
const MEMORY_TRIM_FEATURES: &[&str] = &[
    // Cast/media router service and its utility process.
    "MediaRouter",
    // Built-in translate UI + ML service.
    "Translate",
    // Remote optimization hints fetches.
    "OptimizationHints",
    // Content suggestions / feed prefetching.
    "InterestFeedContentSuggestions",
    // Back/forward cache keeps frozen pages alive in RAM.
    "BackForwardCache",
    // Audio utility process back into the browser process.
    "AudioServiceOutOfProcess",
];

/// Command-line flags for the memory-trim launch mode (feature toggles live
/// in `MEMORY_TRIM_FEATURES`). Every profile is a full browser process tree
/// (browser + GPU + network + utility + renderer + crashpad ≈ 6–8
/// processes); these switch off what a farm/kiosk profile never uses and
/// merge renderers per site — typically cutting idle RAM by a third or more.
const MEMORY_TRIM_FLAGS: &[&str] = &[
    // One renderer per site instead of per tab-instance: the biggest saving
    // when a profile keeps several tabs of the same site open. Isolation
    // BETWEEN profiles (the MBM guarantee) is untouched — that comes from
    // separate user-data-dirs.
    "--process-per-site",
    // No hidden background pages from built-in component extensions.
    "--disable-component-extensions-with-background-pages",
    // No account sync, default apps, network prediction services.
    "--disable-sync",
    "--disable-background-networking",
    "--disable-default-apps",
];

/// Features ENABLED in trim mode: the network utility process merges into
/// the browser process (one fewer process per profile). `--enable-features`
/// is a separate flag from `--disable-features` and Chromium keeps only the
/// last occurrence of each, so both are assembled in one place.
const MEMORY_TRIM_ENABLE_FEATURES: &[&str] = &["NetworkServiceInProcess"];

/// Spawns the browser process detached from our lifecycle concerns beyond the
/// returned `Child` handle (which is polled by the orchestrator watcher).
pub fn spawn(spec: &LaunchSpec) -> AppResult<Child> {
    let args = build_args(spec);

    let mut cmd = Command::new(&spec.executable_path);
    cmd.args(&args);
    cmd.envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    // Detach from our stdio so browser logs don't pollute the app's output, and
    // so many concurrent browser instances don't hold our pipes open.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    Ok(cmd.spawn()?)
}

/// Parses the Chromium `SingletonLock` symlink inside a user-data-dir and
/// returns the pid of the browser instance that owns it.
///
/// Linux-only in practice: the lock is a symlink whose target encodes the pid,
/// and pid verification reads `/proc`. On other platforms this returns `None`,
/// so orphan-recovery features (startup reconciliation, stop-by-lock) degrade
/// to "not detected" there while launch/stop within a session stay accurate.
pub fn singleton_pid(user_data_dir: &Path) -> Option<u32> {
    let lock = user_data_dir.join("SingletonLock");
    let target = std::fs::read_link(lock).ok()?;
    let target_str = target.to_string_lossy();
    // Target format: "<hostname>-<pid>"
    let pid_str = target_str.rsplit('-').next()?;
    pid_str.parse().ok()
}

/// Whether a process with this pid exists.
/// Unix: check `/proc/<pid>`. Windows: OpenProcess + GetExitCodeProcess
/// (still-running processes report the dedicated STILL_ACTIVE sentinel).
/// Declared inline to avoid a windows crate dependency.
pub fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(windows)]
    {
        // Returns false on platforms without /proc-based helpers.
        #[allow(non_snake_case)]
        extern "system" {
            fn OpenProcess(desiredAccess: u32, inheritHandle: i32, processId: u32) -> *mut core::ffi::c_void;
            fn GetExitCodeProcess(process: *mut core::ffi::c_void, exitCode: *mut u32) -> i32;
            fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut exit_code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);
            ok != 0 && exit_code == STILL_ACTIVE
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

/// Whether the process's cmdline references `needle` — used to verify a pid
/// really belongs to the browser of a given user-data-dir before acting on it.
pub fn pid_cmdline_contains(pid: u32, needle: &str) -> bool {
    #[cfg(unix)]
    {
        std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
            .map(|c| c.replace('\0', " ").contains(needle))
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, needle);
        false
    }
}

/// Makes sure the per-profile data directory exists with restrictive permissions.
pub fn ensure_user_data_dir(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_base_args() {
        let spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_paths: Vec::new(),
            language: None,
            user_agent: None,
            startup_url: None,
            window_size: None,
            webrtc_proxy_only: false,
            disable_webrtc: false,
            env: Vec::new(),
            debug_port: None,
            trim_memory: false,
        };
        let args = build_args(&spec);
        assert!(args.contains(&"--user-data-dir=/tmp/mbm/p1".to_string()));
        assert!(args.contains(&"--no-first-run".to_string()));
        assert!(args.contains(&"--no-default-browser-check".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("--proxy-server")));
        assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    }

    #[test]
    fn builds_proxy_args_without_auth() {
        let spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: Some("socks5://10.0.0.1:1080".into()),
            extension_paths: Vec::new(),
            language: None,
            user_agent: None,
            startup_url: None,
            window_size: None,
            webrtc_proxy_only: false,
            disable_webrtc: false,
            env: Vec::new(),
            debug_port: None,
            trim_memory: false,
        };
        let args = build_args(&spec);
        assert!(args.contains(&"--proxy-server=socks5://10.0.0.1:1080".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("--load-extension")));
    }

    #[test]
    fn builds_extension_args_with_auth() {
        let spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_paths: vec!["/tmp/mbm-ext/p1".into()],
            language: None,
            user_agent: None,
            startup_url: None,
            window_size: None,
            webrtc_proxy_only: false,
            disable_webrtc: false,
            env: Vec::new(),
            debug_port: None,
            trim_memory: false,
        };
        let args = build_args(&spec);
        assert!(args.contains(&"--load-extension=/tmp/mbm-ext/p1".to_string()));
        // Auth proxies are configured by the extension, NOT the CLI flag.
        assert!(!args.iter().any(|a| a.starts_with("--proxy-server")));
    }

    #[test]
    fn startup_url_is_the_last_argument() {
        let mut spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/p".into(),
            proxy_server: None,
            extension_paths: Vec::new(),
            trim_memory: false,
            language: None,
            user_agent: None,
            startup_url: Some("https://example.com".into()),
            window_size: None,
            webrtc_proxy_only: false,
            disable_webrtc: false,
            env: Vec::new(),
            debug_port: None,
        };
        let args = build_args(&spec);
        assert_eq!(args.last().map(String::as_str), Some("https://example.com"));
        spec.startup_url = None;
        assert!(!build_args(&spec).iter().any(|a| a.contains("example.com")));
    }

    fn base_spec() -> LaunchSpec {
        LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_paths: Vec::new(),
            language: None,
            user_agent: None,
            startup_url: None,
            window_size: None,
            webrtc_proxy_only: false,
            disable_webrtc: false,
            env: Vec::new(),
            debug_port: None,
            trim_memory: false,
        }
    }

    #[test]
    fn memory_trim_appends_flags_and_is_off_by_default() {
        let off = build_args(&base_spec());
        assert!(!off.iter().any(|a| a.contains("process-per-site")));
        assert!(!off.iter().any(|a| a.starts_with("--disable-features")));
        assert!(!off.iter().any(|a| a.starts_with("--enable-features")));

        let mut spec = base_spec();
        spec.trim_memory = true;
        let on = build_args(&spec);
        assert!(on.contains(&"--process-per-site".to_string()));
        assert!(on.contains(&"--disable-sync".to_string()));
        let features = on
            .iter()
            .find(|a| a.starts_with("--disable-features="))
            .unwrap();
        for f in MEMORY_TRIM_FEATURES {
            assert!(features.contains(f), "{f} must be in the merged flag");
        }
        let enabled = on
            .iter()
            .find(|a| a.starts_with("--enable-features="))
            .unwrap();
        for f in MEMORY_TRIM_ENABLE_FEATURES {
            assert!(enabled.contains(f), "{f} must be in the enable flag");
        }
    }

    #[test]
    fn memory_trim_merges_with_extension_feature_flag() {
        // Without trim: exactly one --disable-features for the extension switch.
        let mut spec = base_spec();
        spec.extension_paths = vec!["/tmp/ext".into()];
        let plain = build_args(&spec);
        assert_eq!(
            plain.iter().filter(|a| a.starts_with("--disable-features")).count(),
            1,
            "repeated --disable-features must never appear (last one would win)"
        );

        // With trim: still one flag, containing BOTH feature sets.
        spec.trim_memory = true;
        let merged = build_args(&spec);
        let features = merged
            .iter()
            .find(|a| a.starts_with("--disable-features="))
            .unwrap();
        assert!(features.contains("DisableLoadExtensionCommandLineSwitch"));
        assert!(features.contains("MediaRouter"));
    }
}
