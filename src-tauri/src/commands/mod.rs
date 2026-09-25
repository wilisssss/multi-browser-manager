pub mod backup;
pub mod credentials;
pub mod folders;
pub mod groups;
pub mod process;
pub mod profile;
pub mod proxy;
pub mod settings;

use crate::browser::detector::{self, BrowserInfo};
use crate::AppState;

/// Tauri command wrapper for browser detection.
///
/// Takes state so the executable cache (L5) is invalidated on every explicit
/// detection: a browser installed/uninstalled while MBM runs must be picked
/// up without an app restart. (The cache only accelerates launches between
/// such UI calls.)
#[tauri::command]
pub fn detect_browsers(state: tauri::State<'_, AppState>) -> Vec<BrowserInfo> {
    if let Ok(mut cache) = state.browser_cache.lock() {
        cache.clear();
    }
    detector::detect_browsers()
}
