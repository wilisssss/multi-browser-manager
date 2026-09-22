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

    if let Some(ext) = &spec.extension_path {
        args.push(format!("--load-extension={ext}"));
        // Unpacked extensions require a non-default --user-data-dir (we always set
        // one) and won't show the developer-mode bubble on a fresh profile dir.
        args.push("--disable-features=DisableLoadExtensionCommandLineSwitch".to_string());
    }

    args
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
/// returns the pid of the browser instance that owns it (unix only).
pub fn singleton_pid(user_data_dir: &Path) -> Option<i32> {
    let lock = user_data_dir.join("SingletonLock");
    let target = std::fs::read_link(lock).ok()?;
    let target_str = target.to_string_lossy();
    // Target format: "<hostname>-<pid>"
    let pid_str = target_str.rsplit('-').next()?;
    pid_str.parse().ok()
}

/// Whether a process with this pid exists (unix, via /proc).
pub fn pid_alive(pid: i32) -> bool {
    #[cfg(unix)]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Whether the process's cmdline references `needle` — used to verify a pid
/// really belongs to the browser of a given user-data-dir before acting on it.
pub fn pid_cmdline_contains(pid: i32, needle: &str) -> bool {
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
}
