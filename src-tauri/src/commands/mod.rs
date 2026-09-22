pub mod backup;
pub mod credentials;
pub mod groups;
pub mod process;
pub mod profile;
pub mod proxy;
pub mod settings;
pub mod window_rules;

use crate::browser::detector::{self, BrowserInfo};

/// Re-export the raw command functions for convenience.
pub use backup::{export_profiles, import_profiles};
pub use process::{bulk_launch, bulk_stop, get_running_profiles, launch_profile, stop_profile};
pub use profile::{
    create_profile, delete_profile, duplicate_profile, get_profiles, update_profile,
};
pub use proxy::{create_proxy, delete_proxy, get_proxies, test_proxy, update_proxy};

/// Tauri command wrapper for browser detection.
#[tauri::command]
pub fn detect_browsers() -> Vec<BrowserInfo> {
    detector::detect_browsers()
}
