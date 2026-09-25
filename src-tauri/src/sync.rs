//! Single mutation-notification path (architecture recommendation A2).
//!
//! Every command that changes profile data calls `after_profile_change`
//! instead of hand-wiring "emit event + rebuild tray" itself. This removes the
//! "forgot to rebuild the tray" bug class (e.g. imports never rebuilt it) and
//! keeps the tray's running-dots in sync with launches/stops from any source
//! (UI, CLI keybinds, tray itself).

use tauri::{AppHandle, Emitter};

/// Notify the rest of the app that profile data changed:
/// the UI refreshes via the `profiles-changed` event and the tray menu is
/// rebuilt through the debounced tray worker (see `tray::schedule_rebuild`).
pub fn after_profile_change(app: &AppHandle, profile_id: &str) {
    let _ = app.emit("profiles-changed", profile_id);
    crate::tray::schedule_rebuild(app);
}
