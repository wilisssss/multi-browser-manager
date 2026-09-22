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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: Option<u128>,
    pub ip: Option<String>,
}
