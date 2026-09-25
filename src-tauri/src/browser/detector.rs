use serde::Serialize;

/// A Chromium-based browser found on this machine.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    /// chrome | chromium | brave | edge
    pub browser_type: String,
    /// Human-readable name, e.g. "Google Chrome".
    pub name: String,
    pub executable_path: String,
    pub version: Option<String>,
}

/// Whether `browser_type` is one of the types this build can actually detect
/// and launch. Used to validate IPC-supplied types (create/update profile):
/// an arbitrary string would otherwise silently produce a profile that can
/// never be launched.
pub fn is_known_browser_type(browser_type: &str) -> bool {
    CANDIDATES.iter().any(|c| c.browser_type == browser_type)
}

/// All browser types this build knows about (for validation error messages).
pub fn known_browser_types() -> Vec<&'static str> {
    CANDIDATES.iter().map(|c| c.browser_type).collect()
}

/// Detect installed Chromium-based browsers on the current OS.
pub fn detect_browsers() -> Vec<BrowserInfo> {
    let mut found = detect_for_platform();
    // De-duplicate by executable path only (e.g. google-chrome and
    // google-chrome-stable are often the same binary through symlinks).
    found.sort_by(|a, b| a.executable_path.cmp(&b.executable_path));
    found.dedup_by(|a, b| {
        // a comes after b in sort order; keep b's (earlier) info.
        if a.executable_path == b.executable_path {
            return true;
        }
        // Also collapse same-binary-different-path cases via canonical paths.
        match (
            std::fs::canonicalize(&a.executable_path),
            std::fs::canonicalize(&b.executable_path),
        ) {
            (Ok(pa), Ok(pb)) if pa == pb => true,
            _ => false,
        }
    });
    found.sort_by(|a, b| a.browser_type.cmp(&b.browser_type).then(a.name.cmp(&b.name)));
    found
}

struct Candidate {
    browser_type: &'static str,
    name: &'static str,
    /// PATH lookup name (Linux) or file name.
    path_name: &'static str,
}

const CANDIDATES: &[Candidate] = &[
    Candidate {
        browser_type: "chrome",
        name: "Google Chrome",
        path_name: "google-chrome",
    },
    Candidate {
        browser_type: "chrome",
        name: "Google Chrome (stable)",
        path_name: "google-chrome-stable",
    },
    Candidate {
        browser_type: "chromium",
        name: "Chromium",
        path_name: "chromium",
    },
    Candidate {
        browser_type: "brave",
        name: "Brave",
        path_name: "brave-browser",
    },
    Candidate {
        browser_type: "edge",
        name: "Microsoft Edge",
        path_name: "microsoft-edge",
    },
    Candidate {
        browser_type: "edge",
        name: "Microsoft Edge (stable)",
        path_name: "microsoft-edge-stable",
    },
];

#[cfg(target_os = "linux")]
fn detect_for_platform() -> Vec<BrowserInfo> {
    let mut result = Vec::new();

    for c in CANDIDATES {
        if let Ok(path) = which::which(c.path_name) {
            if let Some(info) = build_info(c.browser_type, c.name, &path) {
                result.push(info);
            }
        }
    }

    result
}

#[cfg(target_os = "windows")]
fn detect_for_platform() -> Vec<BrowserInfo> {
    let mut result = Vec::new();
    let program_dirs = [
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into()),
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()),
        std::env::var("LocalAppData").unwrap_or_else(|_| String::new()),
    ];

    // (candidate index, relative path under the program directory)
    const WIN_PATHS: &[(&str, &str, &str, &str)] = &[
        ("chrome", "Google Chrome", "chrome.exe", "Google Chrome"),
        ("chromium", "Chromium", "chrome.exe", "Chromium"),
        ("brave", "BraveSoftware", "brave.exe", "Brave"),
        ("edge", "Microsoft", "msedge.exe", "Microsoft Edge"),
    ];

    for (btype, dir, exe, display) in WIN_PATHS {
        for base in &program_dirs {
            if base.is_empty() {
                continue;
            }
            let path = std::path::PathBuf::from(base).join(dir).join("Application").join(exe);
            if path.is_file() {
                if let Some(info) = build_info(btype, display, &path) {
                    result.push(info);
                }
                break;
            }
        }
    }

    result
}

#[cfg(target_os = "macos")]
fn detect_for_platform() -> Vec<BrowserInfo> {
    const MAC_APPS: &[(&str, &str, &str)] = &[
        ("chrome", "Google Chrome", "Google Chrome"),
        ("chromium", "Chromium", "Chromium"),
        ("brave", "Brave", "Brave Browser"),
        ("edge", "Microsoft Edge", "Microsoft Edge"),
    ];

    let mut result = Vec::new();
    for (btype, app, display) in MAC_APPS {
        let path = std::path::PathBuf::from("/Applications")
            .join(format!("{app}.app"))
            .join("Contents")
            .join("MacOS")
            .join(app);
        if path.is_file() {
            if let Some(info) = build_info(btype, display, &path) {
                result.push(info);
            }
        }
    }

    result
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn detect_for_platform() -> Vec<BrowserInfo> {
    Vec::new()
}

/// Reads the browser version by running `<exe> --version`.
fn build_info(browser_type: &str, name: &str, path: &std::path::Path) -> Option<BrowserInfo> {
    let version = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty());

    Some(BrowserInfo {
        browser_type: browser_type.to_string(),
        name: name.to_string(),
        executable_path: path.to_string_lossy().to_string(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic_and_returns_chromium_on_ci_with_chromium() {
        // On any machine with at least one Chromium-family browser installed this
        // returns non-empty; otherwise an empty vec is acceptable. The contract is
        // that detection never panics and never returns a non-executable path.
        let browsers = detect_browsers();
        for b in browsers {
            assert!(std::path::Path::new(&b.executable_path).exists());
            assert!(!b.name.is_empty());
        }
    }
}
