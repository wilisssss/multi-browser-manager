//! IPC layer for backups (A3): parse arguments, lock, delegate to
//! `services::backup`.

use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::AppState;
use std::path::Path;
use tauri::State;

pub use crate::services::backup::ImportResult;

#[tauri::command]
pub fn export_profiles(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let conn = db_lock(&state)?;
    crate::services::backup::write_backup_file(&conn, Path::new(&path))
}

/// Encrypted export (feature 3): same payload as the plaintext export, keyed
/// with a passphrase via Argon2id + AES-256-GCM.
#[tauri::command]
pub fn export_profiles_encrypted(
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> AppResult<()> {
    let conn = db_lock(&state)?;
    crate::services::backup::write_encrypted_backup_file(&conn, Path::new(&path), &passphrase)
}

#[tauri::command]
pub fn import_profiles(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
) -> AppResult<ImportResult> {
    let mut conn = db_lock(&state)?;
    crate::services::backup::import_backup_file(
        &mut conn,
        &state.profiles_root,
        Path::new(&path),
        passphrase.as_deref(),
    )
}

/// Exposes the auto-backup directory so the UI can display it.
#[tauri::command]
pub fn get_backup_dir(state: State<'_, AppState>) -> AppResult<String> {
    let dir = state
        .profiles_root
        .parent()
        .map(|p| p.join("backups"))
        .ok_or_else(|| AppError::internal("No data root configured"))?;
    Ok(dir.to_string_lossy().to_string())
}
