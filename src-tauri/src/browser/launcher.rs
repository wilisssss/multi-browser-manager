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
    /// Set for proxies WITH authentication: path to the generated unpacked
    /// extension that configures the proxy and injects credentials.
    pub extension_path: Option<String>,
    /// Unique per-profile window class / Wayland app-id (`mbm-<name>-<id4>`).
    pub window_class: Option<String>,
    /// Per-profile extra launch arguments (feature: launch-args editor),
    /// already split into tokens by `parse_extra_args`.
    pub extra_args: Vec<String>,
    /// Memory-trim ("lightweight") mode: append the curated RAM-saving flags
    /// (see `MEMORY_TRIM_FLAGS`). Driven by the global settings toggle.
    pub trim_memory: bool,
}

/// Stable, unique-per-profile window class / Wayland app-id:
/// `mbm-<slugified-name>-<first 4 chars of the profile id>`. The id suffix
/// keeps two profiles with the same name from colliding; the slug keeps the
/// value readable in compositor logs and window rules.
pub fn window_app_id(profile_id: &str, profile_name: &str) -> String {
    let mut slug = String::new();
    for ch in profile_name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "profile" } else { slug };
    let short: String = profile_id.chars().take(4).collect();
    format!("mbm-{slug}-{short}")
}

/// Builds the Chromium command-line argument list for a launch spec.
pub fn build_args(spec: &LaunchSpec) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", spec.user_data_dir),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ];

    // Unique identity so WM window rules can match this profile's windows.
    // `--class` covers X11, `--wayland-app-id` covers Wayland compositors.
    if let Some(class) = &spec.window_class {
        args.push(format!("--class={class}"));
        args.push(format!("--wayland-app-id={class}"));
    }

    if let Some(proxy) = &spec.proxy_server {
        args.push(format!("--proxy-server={proxy}"));
    }

    // Chromium does not merge repeated --disable-features (the last one
    // wins), so all feature toggles are collected into a single flag.
    let mut features: Vec<&str> = Vec::new();
    if spec.trim_memory {
        features.extend(MEMORY_TRIM_FEATURES);
    }
    if spec.extension_path.is_some() {
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

    if let Some(ext) = &spec.extension_path {
        args.push(format!("--load-extension={ext}"));
    }

    // Per-profile power-user arguments last, so users can see (but not
    // override — those are blocked at save time) MBM's own flags above.
    args.extend(spec.extra_args.iter().cloned());

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

/// Flags MBM must keep under its own control: they define the isolation and
/// identity of a profile, and letting a per-profile arg override them would
/// merge profiles' data or break window rules.
const BLOCKED_EXTRA_ARG_PREFIXES: &[&str] = &[
    "--user-data-dir",
    "--user-data",
    "--profile-directory",
    "--proxy-server",
    "--proxy-pac-url",
    "--proxy-bypass",
    "--load-extension",
    "--class",
    "--wayland-app-id",
    "--no-startup-window",
];

/// Parses a per-profile extra-args string into tokens. Splitting is
/// whitespace-based with double-quote grouping so values with spaces work:
/// `--proxy-server="http://a b"` … well, flags like these are blocked, but
/// e.g. `--host-resolver-rules="MAP * 1.2.3.4"` survives intact.
pub fn parse_extra_args(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_content = false;
    for ch in raw.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_content = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_content {
                    tokens.push(std::mem::take(&mut current));
                    has_content = false;
                }
            }
            c => {
                current.push(c);
                has_content = true;
            }
        }
    }
    if has_content {
        tokens.push(current);
    }
    tokens
}

/// Validates a per-profile extra-args string: parses it and rejects flags
/// that would break MBM's isolation/identity guarantees. Returns the parsed
/// tokens on success.
pub fn validate_extra_args(raw: &str) -> AppResult<Vec<String>> {
    let tokens = parse_extra_args(raw);
    for token in &tokens {
        let lowered = token.to_ascii_lowercase();
        if BLOCKED_EXTRA_ARG_PREFIXES
            .iter()
            .any(|p| lowered == *p || lowered.starts_with(&format!("{p}=")))
        {
            return Err(crate::error::AppError::validation(format!(
                "Argument '{token}' is managed by MBM and cannot be overridden"
            )));
        }
    }
    Ok(tokens)
}

/// Spawns the browser process detached from our lifecycle concerns beyond the
/// returned `Child` handle (which is polled by the orchestrator watcher).
pub fn spawn(spec: &LaunchSpec) -> AppResult<Child> {
    let args = build_args(spec);

    let mut cmd = Command::new(&spec.executable_path);
    cmd.args(&args);
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
            extension_path: None,
            window_class: None,
            extra_args: vec![],
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
            extension_path: None,
            window_class: None,
            extra_args: vec![],
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
            extension_path: Some("/tmp/mbm-ext/p1".into()),
            window_class: None,
            extra_args: vec![],
            trim_memory: false,
        };
        let args = build_args(&spec);
        assert!(args.contains(&"--load-extension=/tmp/mbm-ext/p1".to_string()));
        // Auth proxies are configured by the extension, NOT the CLI flag.
        assert!(!args.iter().any(|a| a.starts_with("--proxy-server")));
    }

    #[test]
    fn builds_window_class_args() {
        let spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_path: None,
            window_class: Some("mbm-work-ab12".into()),
            extra_args: vec![],
            trim_memory: false,
        };
        let args = build_args(&spec);
        assert!(args.contains(&"--class=mbm-work-ab12".to_string()));
        assert!(args.contains(&"--wayland-app-id=mbm-work-ab12".to_string()));
    }

    #[test]
    fn window_app_id_is_slugged_unique_and_stable() {
        let id = "a1b2c3d4-e5f6";
        assert_eq!(window_app_id(id, "Work"), "mbm-work-a1b2");
        assert_eq!(window_app_id(id, "My Tags & Spaces!"), "mbm-my-tags-spaces-a1b2");
        // Same name, different id → different app id.
        assert_ne!(window_app_id(id, "Work"), window_app_id("ffff0000", "Work"));
        // Unusable name → placeholder slug.
        assert_eq!(window_app_id(id, "中文"), "mbm-profile-a1b2");
        // Stable across calls.
        assert_eq!(window_app_id(id, "Work"), window_app_id(id, "Work"));
    }

    #[test]
    fn extra_args_are_parsed_with_quote_support() {
        assert!(parse_extra_args("").is_empty());
        assert!(parse_extra_args("   ").is_empty());
        assert_eq!(parse_extra_args("--disable-gpu"), vec!["--disable-gpu"]);
        assert_eq!(
            parse_extra_args("--disable-gpu  --start-maximized"),
            vec!["--disable-gpu", "--start-maximized"]
        );
        assert_eq!(
            parse_extra_args("--host-resolver-rules=\"MAP * 1.2.3.4\""),
            vec!["--host-resolver-rules=MAP * 1.2.3.4"]
        );
        // Unterminated quote: the rest is one token, no panic.
        assert_eq!(parse_extra_args("--flag=\"open"), vec!["--flag=open"]);
    }

    #[test]
    fn extra_args_append_to_built_args() {
        let spec = LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_path: None,
            window_class: None,
            extra_args: vec!["--disable-gpu".into(), "--start-maximized".into()],
            trim_memory: false,
        };
        let args = build_args(&spec);
        let pos = args.iter().position(|a| a == "--disable-gpu").unwrap();
        assert!(args[pos..].contains(&"--start-maximized".to_string()));
        assert!(pos > 0, "extra args come after MBM's own flags");
    }

    #[test]
    fn extra_args_blocklist_protects_mbm_invariants() {
        assert!(validate_extra_args("--disable-gpu").is_ok());
        for blocked in [
            "--user-data-dir=/tmp/evil",
            "--proxy-server=1.2.3.4",
            "--load-extension=/tmp/ext",
            "--class=rogue",
            "--wayland-app-id=rogue",
        ] {
            assert!(validate_extra_args(blocked).is_err(), "{blocked} must be blocked");
        }
    }

    fn base_spec() -> LaunchSpec {
        LaunchSpec {
            executable_path: "/usr/bin/chromium".into(),
            user_data_dir: "/tmp/mbm/p1".into(),
            proxy_server: None,
            extension_path: None,
            window_class: None,
            extra_args: vec![],
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
        spec.extension_path = Some("/tmp/ext".into());
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
