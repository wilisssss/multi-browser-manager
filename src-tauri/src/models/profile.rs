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
    /// value = assign. Plain serde maps JSON `null` to the outer `None`,
    /// which would make "clear" indistinguishable from "keep" — hence the
    /// `double_option` deserializer below.
    #[serde(default, deserialize_with = "double_option")]
    pub proxy_id: Option<Option<String>>,
    /// Same semantics: absent = keep, null = clear.
    #[serde(default, deserialize_with = "double_option")]
    pub notes: Option<Option<String>>,
    pub restart_on_crash: Option<bool>,
    /// Same double-Option semantics: absent = keep, null = unfile, value = move.
    #[serde(default, deserialize_with = "double_option")]
    pub folder_id: Option<Option<String>>,
    /// Absent = keep, null = clear all overrides, value = replace.
    #[serde(default, deserialize_with = "double_option")]
    pub fingerprint: Option<Option<Fingerprint>>,
    /// Absent = keep, null = clear, value = replace.
    #[serde(default, deserialize_with = "double_option")]
    pub startup_url: Option<Option<String>>,
}

/// Turns an explicit JSON `null` into `Some(None)` so a field can be cleared.
/// Without it, `Option<Option<T>>` reads `null` as `None` — the same as an
/// absent field — and "No proxy" (or any cleared field) silently keeps the
/// old value. An absent field never reaches this: `#[serde(default)]` applies.
fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(de).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The edit form's checkbox must survive the JSON round trip. The field
    /// name is serde's camelCase (`disableWebrtc`) — a key spelled
    /// differently (e.g. `disableWebRTC`) is silently ignored on
    /// deserialize, so a checked box would come back unchecked.
    #[test]
    fn disable_webrtc_roundtrips_as_camel_case() {
        let fp: Fingerprint =
            serde_json::from_str(r#"{"userAgent":"u","disableWebrtc":true}"#).unwrap();
        assert!(fp.disable_webrtc, "disableWebrtc:true must deserialize to true");

        let out = serde_json::to_string(&fp).unwrap();
        assert!(out.contains(r#""disableWebrtc":true"#), "serialize must emit disableWebrtc: {out}");
    }

    /// The exact payload the edit form sends when the user picks "No proxy":
    /// every cleared field arrives as an explicit null and must actually
    /// clear, not fall through to "keep the old value".
    #[test]
    fn explicit_null_clears_and_absent_keeps() {
        let keep: UpdateProfileInput = serde_json::from_str("{}").unwrap();
        assert!(keep.proxy_id.is_none(), "absent = keep current");

        let clear: UpdateProfileInput =
            serde_json::from_str(r#"{"proxyId": null}"#).unwrap();
        assert_eq!(clear.proxy_id, Some(None), "explicit null = clear");

        let set: UpdateProfileInput =
            serde_json::from_str(r#"{"proxyId": "abc"}"#).unwrap();
        assert_eq!(set.proxy_id, Some(Some("abc".into())));
    }

    #[test]
    fn form_payload_with_all_fields_cleared_still_parses() {
        let payload = r#"{
            "name": "Utama",
            "browserType": "chromium",
            "proxyId": null,
            "notes": null,
            "restartOnCrash": false,
            "folderId": null,
            "fingerprint": null,
            "startupUrl": null
        }"#;
        let input: UpdateProfileInput = serde_json::from_str(payload).unwrap();
        assert_eq!(input.proxy_id, Some(None));
        assert_eq!(input.notes, Some(None));
        assert_eq!(input.folder_id, Some(None));
        assert_eq!(input.fingerprint, Some(None));
        assert_eq!(input.startup_url, Some(None));
        assert_eq!(input.restart_on_crash, Some(false));
        assert_eq!(input.name.as_deref(), Some("Utama"));
    }

    #[test]
    fn form_payload_with_values_keeps_them() {
        let payload = r#"{
            "proxyId": "f08a2adf-c52d-4adb-a6a7-07f0603ad6f0",
            "startupUrl": "https://myip.link"
        }"#;
        let input: UpdateProfileInput = serde_json::from_str(payload).unwrap();
        assert_eq!(input.proxy_id, Some(Some("f08a2adf-c52d-4adb-a6a7-07f0603ad6f0".into())));
        assert_eq!(input.startup_url, Some(Some("https://myip.link".into())));
        // Untouched fields stay "keep".
        assert!(input.name.is_none() && input.fingerprint.is_none());
    }
}
