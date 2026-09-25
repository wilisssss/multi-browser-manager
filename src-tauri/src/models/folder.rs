use serde::{Deserialize, Serialize};

/// A user-created folder that groups profiles (file-manager style). Folders
/// nest via `parent_id` (NULL = root level); the UI only lists a folder's
/// profiles after it is opened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// Parent folder id; `None` = the folder lives at the root.
    pub parent_id: Option<String>,
    /// Sort order within the parent (lower first).
    pub position: i64,
    pub created_at: i64,
    /// Number of profiles directly inside this folder (filled by the list
    /// query, not stored).
    #[serde(default)]
    pub profile_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderInput {
    pub name: String,
    /// `None` = create at the root.
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameFolderInput {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFolderInput {
    /// `None` = move to the root.
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveProfilesInput {
    pub profile_ids: Vec<String>,
    /// `None` = move out to "unfiled" (the root's pseudo-folder).
    #[serde(default)]
    pub folder_id: Option<String>,
}
