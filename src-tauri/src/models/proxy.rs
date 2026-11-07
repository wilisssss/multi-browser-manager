use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: String,
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProxyInput {
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    /// Held in memory only; persisted to the OS keychain, never to SQLite.
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProxyInput {
    pub label: Option<String>,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub port: Option<i64>,
    /// Absent = keep current; explicit null = remove the username.
    pub username: Option<Option<String>>,
    /// Absent = keep current; explicit null or empty = remove the password.
    pub password: Option<Option<String>>,
}

/// One successful proxy connection test: the exit identity the proxy showed.
///
/// Used both as the per-row history the Proxy Manager renders and as the
/// persisted record the freshness check consults. The table is the source of
/// truth; the UI keeps only the newest entry per proxy in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyExitTest {
    pub id: i64,
    pub proxy_id: String,
    pub ip: String,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub asn: Option<String>,
    pub asn_org: Option<String>,
    pub tested_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<u128>,
    pub ip: Option<String>,
    /// Country name, e.g. "Indonesia". Absent when the lookup fails.
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2, e.g. "ID".
    pub country_code: Option<String>,
    /// ASN number in `AS<digits>` notation, e.g. "AS138089".
    pub asn: Option<String>,
    /// ASN organization name, e.g. "Fiberbyte".
    pub asn_org: Option<String>,
    /// True when this exit IP has never been seen in a successful test before.
    pub fresh: bool,
    /// Previously used with the same exit IP: how many earlier tests, and the
    /// most recent of their timestamps. `None` exactly when `fresh`.
    pub previously_seen: Option<ProxySeenBefore>,
}

/// Freshness verdict for a rotating-proxy exit IP.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySeenBefore {
    /// How many earlier successful tests returned this same IP.
    pub count: i64,
    /// Unix timestamp of the most recent earlier test with this IP.
    pub last_seen_at: i64,
}

/// Clear a proxy's exit-IP history, so a rotating proxy can be re-tested
/// against an empty "used before" set (used by the "Reset freshness" button).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetProxyHistoryInput {
    pub proxy_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProxiesInput {
    /// URL whose response contains the proxy list (one per line, or a JSON
    /// array of objects). May itself carry credentials in the userinfo part.
    pub endpoint: String,
    /// Protocol used for list lines that do not carry a scheme.
    #[serde(default = "default_protocol")]
    pub default_protocol: String,
    /// Prepended to the `<host>:<port>` label (e.g. "provider — ").
    #[serde(default)]
    pub label_prefix: Option<String>,
}

fn default_protocol() -> String {
    "http".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyImportResult {
    pub imported: u32,
    pub duplicates: u32,
    pub invalid: u32,
    /// First parse failures, one line each ("line 3: host could not be parsed").
    pub errors: Vec<String>,
}
