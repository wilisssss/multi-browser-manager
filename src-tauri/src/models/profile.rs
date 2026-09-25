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
    /// Extra command-line arguments appended to the browser launch
    /// (`--disable-gpu`, `--start-maximized`, ...). Space-separated with
    /// double-quote support; parsed by `launcher::parse_extra_args`.
    #[serde(default)]
    pub extra_args: Option<String>,
    /// Re-launch the browser automatically after an unexpected exit.
    #[serde(default)]
    pub restart_on_crash: bool,
    /// Seconds to wait for a graceful stop before force-killing.
    /// `None` = the 3-second default.
    #[serde(default)]
    pub stop_timeout_secs: Option<i64>,
    /// Folder this profile lives in (`None` = unfiled, shown in the root's
    /// "unfiled" view).
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

impl Profile {
    /// Constructor for the common "new, stopped profile" shape. Every former
    /// 11-field struct literal (insert path, fixtures, backup tests) collapses
    /// to this; timestamps are stamped here and groups start empty.
    pub fn new(id: impl Into<String>, name: impl Into<String>, browser_type: impl Into<String>, user_data_dir: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Profile {
            id: id.into(),
            name: name.into(),
            browser_type: browser_type.into(),
            user_data_dir: user_data_dir.into(),
            proxy_id: None,
            notes: None,
            status: "stopped".to_string(),
            created_at: now,
            updated_at: now,
            last_used_at: None,
            pinned: false,
            extra_args: None,
            restart_on_crash: false,
            stop_timeout_secs: None,
            folder_id: None,
            groups: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub browser_type: String,
    pub proxy_id: Option<String>,
    pub notes: Option<String>,
    /// Space/double-quote separated launch arguments; validated against a
    /// blocklist of flags MBM must control itself (e.g. --user-data-dir).
    #[serde(default)]
    pub extra_args: Option<String>,
    #[serde(default)]
    pub restart_on_crash: bool,
    /// 1..=60, None = default.
    #[serde(default)]
    pub stop_timeout_secs: Option<i64>,
    /// Folder to file the new profile into.
    #[serde(default)]
    pub folder_id: Option<String>,
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
    pub extra_args: Option<Option<String>>,
    pub restart_on_crash: Option<bool>,
    pub stop_timeout_secs: Option<Option<i64>>,
    /// Same double-Option semantics: absent = keep, null = unfile, value = move.
    pub folder_id: Option<Option<String>>,
}
