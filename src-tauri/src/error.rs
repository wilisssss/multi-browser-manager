use serde::Serialize;

/// Unified application error type, serializable across the Tauri IPC boundary.
///
/// The three "plain message" situations (validation failures, not-found,
/// internal errors) used to be three identical `String` variants; they are now
/// one variant plus tiny constructor helpers so call sites keep reading
/// naturally while the enum carries no redundant arms.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("Tauri error: {0}")]
    Tauri(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Message(String),
}

impl AppError {
    /// Invalid input from IPC/CLI/backup file.
    pub fn validation(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }

    /// Referenced entity does not exist.
    pub fn not_found(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }

    /// Everything else that is not a wrapped foreign error.
    pub fn internal(message: impl Into<String>) -> Self {
        AppError::Message(message.into())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keyring(e.to_string())
    }
}

// Menu/tray builders (MenuItem, Submenu, Menu, TrayIconBuilder) all fail with
// this error type; tray.rs propagates it instead of catching panics.
impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Tauri(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<getrandom::Error> for AppError {
    fn from(e: getrandom::Error) -> Self {
        AppError::internal(format!("Random generation failed: {e}"))
    }
}

impl From<aes_gcm::Error> for AppError {
    fn from(e: aes_gcm::Error) -> Self {
        AppError::internal(format!("AES-GCM operation failed: {e}"))
    }
}
