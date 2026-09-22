use serde::{Deserialize, Serialize};

use super::group::Group;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub browser_type: String,
    pub user_data_dir: String,
    pub proxy_id: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub browser_type: String,
    pub proxy_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileInput {
    pub name: Option<String>,
    pub browser_type: Option<String>,
    /// Double-Option semantics: absent = keep current, null = unassign,
    /// value = assign. (JSON can't express this with a single Option.)
    pub proxy_id: Option<Option<String>>,
    /// Same semantics: absent = keep, null = clear.
    pub notes: Option<Option<String>>,
}
