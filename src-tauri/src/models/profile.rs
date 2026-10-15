use serde::{Deserialize, Serialize};

/// Per-profile identity overrides. Every field is optional: `None` keeps the
/// browser's own value. Applied at launch via command-line flags (language,
/// user agent) and a generated extension (the rest).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    /// BCP-47 tag, e.g. `en-US`. Sets `--lang` and `navigator.language(s)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Full User-Agent string. Sets `--user-agent` and `navigator.userAgent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// IANA zone, e.g. `Europe/Berlin`. Overrides `Intl` and `Date` output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// `navigator.platform`, e.g. `Win32`, `MacIntel`, `Linux x86_64`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Fixed coordinates reported to `navigator.geolocation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<Geolocation>,
    /// Font families the page may detect. Empty/None = no spoofing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonts: Option<Vec<String>>,
    /// Reported screen size in CSS pixels (`screen.width` / `height`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<Screen>,
    /// Block WebRTC from revealing the host IP: ICE servers are dropped and
    /// candidates are never surfaced. Video/voice calls will not connect.
    #[serde(default)]
    pub disable_webrtc: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Geolocation {
    pub latitude: f64,
    pub longitude: f64,
    /// Accuracy radius in meters.
    #[serde(default = "default_accuracy")]
    pub accuracy: f64,
}

fn default_accuracy() -> f64 {
    50.0
}

impl Fingerprint {
    /// True when no field is set, so no extension or flags are needed.
    pub fn is_empty(&self) -> bool {
        self.language.is_none()
            && self.user_agent.is_none()
            && self.timezone.is_none()
            && self.platform.is_none()
            && self.geolocation.is_none()
            && self.fonts.is_none()
            && self.screen.is_none()
            && !self.disable_webrtc
    }
}

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
    /// Re-launch the browser automatically after an unexpected exit.
    #[serde(default)]
    pub restart_on_crash: bool,
    /// Folder this profile lives in (`None` = unfiled, shown in the root's
    /// "unfiled" view).
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Identity overrides; `None` when the profile uses browser defaults.
    #[serde(default)]
    pub fingerprint: Option<Fingerprint>,
    /// Page opened on the first start of the browser (`http`/`https` only).
    #[serde(default)]
    pub startup_url: Option<String>,
}

impl Profile {
    /// Constructor for the common "new, stopped profile" shape. Every former
    /// struct literal (insert path, fixtures, backup tests) collapses to this;
    /// timestamps are stamped here.
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
            restart_on_crash: false,
            folder_id: None,
            fingerprint: None,
            startup_url: None,
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
    #[serde(default)]
    pub restart_on_crash: bool,
    /// Folder to file the new profile into.
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<Fingerprint>,
    #[serde(default)]
    pub startup_url: Option<String>,
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
    pub restart_on_crash: Option<bool>,
    /// Same double-Option semantics: absent = keep, null = unfile, value = move.
    pub folder_id: Option<Option<String>>,
    /// Absent = keep, null = clear all overrides, value = replace.
    pub fingerprint: Option<Option<Fingerprint>>,
    /// Absent = keep, null = clear, value = replace.
    pub startup_url: Option<Option<String>>,
}
