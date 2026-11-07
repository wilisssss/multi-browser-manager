use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::models::proxy::{
    CreateProxyInput, Proxy, ProxyExitTest, ProxySeenBefore, ProxyTestResult,
    ResetProxyHistoryInput, UpdateProxyInput,
};
use crate::proxy_manager;
use crate::AppState;
use rusqlite::{params, Connection};
use tauri::State;

fn row_to_proxy(row: &rusqlite::Row<'_>) -> rusqlite::Result<Proxy> {
    Ok(Proxy {
        id: row.get("id")?,
        label: row.get("label")?,
        protocol: row.get("protocol")?,
        host: row.get("host")?,
        port: row.get("port")?,
        username: row.get("username")?,
        created_at: row.get("created_at")?,
    })
}

pub fn list_proxies(conn: &Connection) -> AppResult<Vec<Proxy>> {
    let mut stmt = conn.prepare("SELECT * FROM proxies ORDER BY created_at ASC")?;
    let rows = stmt
        .query_map([], row_to_proxy)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_proxy_by_id(conn: &Connection, id: &str) -> AppResult<Proxy> {
    conn.query_row("SELECT * FROM proxies WHERE id = ?1", params![id], row_to_proxy)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::not_found(format!("Proxy {id} not found"))
            }
            other => other.into(),
        })
}

fn validate_label(label: &str) -> AppResult<()> {
    if label.trim().is_empty() {
        return Err(AppError::validation("Proxy label cannot be empty"));
    }
    Ok(())
}

fn validate_port(port: i64) -> AppResult<()> {
    if !(1..=65535).contains(&port) {
        return Err(AppError::validation("Port must be between 1 and 65535"));
    }
    Ok(())
}

pub fn insert_proxy(conn: &Connection, proxy: &Proxy) -> AppResult<()> {
    conn.execute(
        "INSERT INTO proxies (id, label, protocol, host, port, username, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            proxy.id,
            proxy.label,
            proxy.protocol,
            proxy.host,
            proxy.port,
            proxy.username,
            proxy.created_at,
        ],
    )?;
    Ok(())
}

/// Inserts a proxy and persists its password to the OS keychain when present.
pub fn insert_proxy_with_password(
    conn: &Connection,
    proxy: &Proxy,
    password: Option<&str>,
) -> AppResult<()> {
    insert_proxy(conn, proxy)?;
    if let Some(pass) = password.filter(|p| !p.is_empty()) {
        proxy_manager::store_proxy_password(&proxy.id, pass)?;
    }
    Ok(())
}

#[tauri::command]
pub fn create_proxy(state: State<'_, AppState>, input: CreateProxyInput) -> AppResult<Proxy> {
    validate_label(&input.label)?;
    proxy_manager::ensure_valid_protocol(&input.protocol)?;
    validate_port(input.port)?;
    if input.host.trim().is_empty() {
        return Err(AppError::validation("Proxy host cannot be empty"));
    }

    let proxy = Proxy {
        id: uuid::Uuid::new_v4().to_string(),
        label: input.label.trim().to_string(),
        protocol: input.protocol,
        host: input.host.trim().to_string(),
        port: input.port,
        username: input.username.filter(|u| !u.trim().is_empty()),
        created_at: chrono::Utc::now().timestamp(),
    };

    // Row first (short DB lock), password second in the keychain.
    // The keychain call is a blocking D-Bus round-trip (gnome-keyring prompts
    // can hang for many seconds) — it must NEVER run while holding the shared
    // DB mutex, or every other IPC command queues behind it (same rule the
    // launch path follows via build_launch_spec_blocking).
    {
        let conn = db_lock(&state)?;
        insert_proxy(&conn, &proxy)?;
    }
    if let Some(pass) = input.password.as_deref().filter(|p| !p.is_empty()) {
        if let Err(e) = proxy_manager::store_proxy_password(&proxy.id, pass) {
            // Roll back the row so an auth proxy never exists without its
            // password (launch would fail mysteriously later).
            let conn = db_lock(&state)?;
            let _ = conn.execute("DELETE FROM proxies WHERE id = ?1", params![proxy.id]);
            return Err(e);
        }
    }
    Ok(proxy)
}

#[tauri::command]
pub fn update_proxy(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProxyInput,
) -> AppResult<Proxy> {
    if let Some(label) = &input.label {
        validate_label(label)?;
    }
    if let Some(protocol) = &input.protocol {
        proxy_manager::ensure_valid_protocol(protocol)?;
    }
    if let Some(port) = input.port {
        validate_port(port)?;
    }

    // Read-modify-write under a short lock; NO keychain access in here (see
    // create_proxy for why).
    let updated = {
        let conn = db_lock(&state)?;
        let existing = get_proxy_by_id(&conn, &id)?;

        let label = input.label.map(|l| l.trim().to_string()).unwrap_or(existing.label);
        let protocol = input.protocol.unwrap_or(existing.protocol);
        let host = input.host.map(|h| h.trim().to_string()).unwrap_or(existing.host);
        let port = input.port.unwrap_or(existing.port);
        // Absent = keep current; explicit null = clear the username.
        let username = match input.username {
            Some(v) => v.filter(|u| !u.trim().is_empty()),
            None => existing.username,
        };

        conn.execute(
            "UPDATE proxies SET label = ?1, protocol = ?2, host = ?3, port = ?4, username = ?5 WHERE id = ?6",
            params![label, protocol, host, port, username, id],
        )?;
        get_proxy_by_id(&conn, &id)?
    };

    // Password: absent = keep; explicit null or empty = remove from keychain;
    // non-empty = replace. Runs after the DB lock is released.
    match input.password {
        Some(Some(pass)) if !pass.is_empty() => {
            proxy_manager::store_proxy_password(&id, &pass)?;
        }
        Some(_) => {
            proxy_manager::delete_proxy_password(&id)?;
        }
        None => {}
    }

    Ok(updated)
}

#[tauri::command]
pub fn delete_proxy(state: State<'_, AppState>, id: String) -> AppResult<()> {
    {
        let conn = db_lock(&state)?;

        // Refuse to delete a proxy that is still assigned to profiles.
        let in_use: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM profiles WHERE proxy_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if in_use {
            return Err(AppError::validation(
                "This proxy is still assigned to one or more profiles. Unassign it first.",
            ));
        }

        conn.execute("DELETE FROM proxies WHERE id = ?1", params![id])?;
    }
    // Keychain cleanup outside the DB lock (blocking D-Bus, see create_proxy).
    // A keychain outage must not surface as "delete failed" while the row is
    // already gone — log it and succeed; the leftover entry is orphaned but
    // harmless (and the next store for this id can't happen: ids are UUIDs).
    if let Err(e) = proxy_manager::delete_proxy_password(&id) {
        eprintln!("mbm: failed to delete proxy password from keychain: {e}");
    }
    Ok(())
}

/// Newest exit IPs a proxy has shown, for the freshness strip under each row.
#[tauri::command]
pub fn get_proxy_tests(
    state: State<'_, AppState>,
    proxy_id: String,
) -> AppResult<Vec<ProxyExitTest>> {
    let conn = db_lock(&state)?;
    list_proxy_exit_tests(&conn, &proxy_id)
}

/// Forgets a proxy's exit-IP history, so a rotating proxy can be re-tested
/// against an empty "used before" set (and its strip cleared).
#[tauri::command]
pub fn reset_proxy_history(
    state: State<'_, AppState>,
    input: ResetProxyHistoryInput,
) -> AppResult<()> {
    let conn = db_lock(&state)?;
    conn.execute(
        "DELETE FROM proxy_test_history WHERE proxy_id = ?1",
        params![input.proxy_id],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn test_proxy(state: State<'_, AppState>, id: String) -> AppResult<ProxyTestResult> {
    test_proxy_impl(&state, &id).await
}

/// The test itself, split from the Tauri command so the freshness flow can be
/// exercised by the ignored e2e probe below without a Tauri runtime.
async fn test_proxy_impl(state: &AppState, id: &str) -> AppResult<ProxyTestResult> {
    let proxy = {
        let conn = db_lock(state)?;
        get_proxy_by_id(&conn, id)?
    };
    // Keychain read AFTER the DB lock is released (blocking D-Bus round trip,
    // see create_proxy for why it must never hold the shared mutex).
    let password = if proxy
        .username
        .as_deref()
        .map(|u| !u.is_empty())
        .unwrap_or(false)
    {
        Some(proxy_manager::get_proxy_password(id)?)
    } else {
        None
    };

    let mut result = proxy_manager::test_proxy_connection(
        &proxy.protocol,
        &proxy.host,
        proxy.port,
        proxy.username.as_deref(),
        password.as_deref(),
    )
    .await?;

    // Freshness: judge the exit IP against every earlier successful test,
    // then record this one. Failed tests saw no exit IP, so they never
    // enter the history.
    if result.success {
        if let Some(exit_ip) = result.ip.clone() {
            let conn = db_lock(state)?;
            match exit_ip_seen_before(&conn, &exit_ip)? {
                None => {
                    result.fresh = true;
                    result.previously_seen = None;
                }
                Some((count, last_seen_at)) => {
                    result.fresh = false;
                    result.previously_seen = Some(ProxySeenBefore { count, last_seen_at });
                }
            }
            record_exit_test(
                &conn,
                id,
                &exit_ip,
                result.country.as_deref(),
                result.country_code.as_deref(),
                result.asn.as_deref(),
                result.asn_org.as_deref(),
            )?;
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn get_proxies(state: State<'_, AppState>) -> AppResult<Vec<Proxy>> {
    let conn = db_lock(&state)?;
    list_proxies(&conn)
}

/// Fetches a proxy list from an HTTP(S) endpoint (one proxy per line, or a
/// JSON array of objects) and imports every entry, storing passwords in the
/// OS keychain. Entries already in the table (same protocol/host/port/user)
/// are skipped as duplicates; credentials do not have to be typed by hand.
#[tauri::command]
pub async fn import_proxies_from_url(
    state: State<'_, AppState>,
    input: crate::models::proxy::ImportProxiesInput,
) -> AppResult<crate::models::proxy::ProxyImportResult> {
    let endpoint = input.endpoint.trim().to_string();
    if endpoint.is_empty() {
        return Err(AppError::validation("Endpoint URL cannot be empty"));
    }
    let default_protocol = input.default_protocol.trim().to_lowercase();
    crate::proxy_manager::ensure_valid_protocol(&default_protocol)?;
    let label_prefix = input
        .label_prefix
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    // Only http(s) endpoints, 20 s, 4 MiB body.
    let parsed_url = url::Url::parse(&endpoint)
        .map_err(|e| AppError::validation(format!("Endpoint URL is not valid: {e}")))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(AppError::validation("Endpoint URL must start with http:// or https://"));
    }
    // The endpoint itself may carry basic auth in the userinfo
    // (https://user:pass@host/api/list) — pass it explicitly and keep it out
    // of the request URL.
    let endpoint_auth = {
        let user = parsed_url.username();
        if user.is_empty() {
            None
        } else {
            Some((user.to_string(), parsed_url.password().map(str::to_string)))
        }
    };
    let fetch_url = if endpoint_auth.is_some() {
        let mut clean = parsed_url.clone();
        let _ = clean.set_username("");
        let _ = clean.set_password(None);
        clean
    } else {
        parsed_url
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(AppError::from)?;
    let mut req = client
        .get(fetch_url)
        .header(reqwest::header::ACCEPT, "text/plain, application/json, */*");
    if let Some((user, pass)) = endpoint_auth {
        req = req.basic_auth(user, pass);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::validation(format!("Could not fetch the endpoint: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::validation(format!(
            "Endpoint returned HTTP {}",
            status.as_u16()
        )));
    }
    if let Some(len) = resp.content_length() {
        if len > 4 * 1024 * 1024 {
            return Err(AppError::validation("Endpoint response is larger than 4 MiB"));
        }
    }
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::validation(format!("Could not read the endpoint response: {e}")))?;
    if body.len() > 4 * 1024 * 1024 {
        return Err(AppError::validation("Endpoint response is larger than 4 MiB"));
    }

    let (parsed, failures) = crate::proxy_import::parse_body(&body, &default_protocol);
    let mut errors: Vec<String> = failures
        .iter()
        .map(|f| {
            if f.line == 0 {
                f.reason.clone()
            } else {
                format!("line {}: {}", f.line, f.reason)
            }
        })
        .collect();
    if parsed.is_empty() && errors.is_empty() {
        return Err(AppError::validation(
            "The endpoint returned an empty proxy list (and it was not valid JSON either)",
        ));
    }

    // Dedupe against what is already in the table.
    let existing: std::collections::HashSet<(String, String, i64, Option<String>)> = {
        let conn = db_lock(&state)?;
        let mut stmt = conn.prepare("SELECT protocol, host, port, username FROM proxies")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect::<Result<std::collections::HashSet<_>, _>>()?
    };

    let (rows, duplicates) = {
        let conn = db_lock(&state)?;
        import_insert_rows(&conn, parsed, label_prefix.as_deref(), &existing)?
    };

    let mut imported = 0u32;
    let mut failures_count = failures.len() as u32;
    for (proxy, password) in rows {
        if let Some(pass) = password.filter(|pw| !pw.is_empty()) {
            if let Err(e) = proxy_manager::store_proxy_password(&proxy.id, &pass) {
                // Never leave an auth proxy without its password.
                let conn = db_lock(&state)?;
                let _ = conn.execute("DELETE FROM proxies WHERE id = ?1", params![proxy.id]);
                eprintln!(
                    "mbm: failed to store imported proxy password for {}: {e}",
                    proxy.label
                );
                errors.push(format!(
                    "{}: could not store the password in the OS keychain",
                    proxy.label
                ));
                failures_count += 1;
                continue;
            }
        }
        imported += 1;
    }

    if errors.len() > 20 {
        let rest = errors.len() - 20;
        errors.truncate(20);
        errors.push(format!("… and {rest} more"));
    }

    Ok(crate::models::proxy::ProxyImportResult {
        imported,
        duplicates,
        invalid: failures_count,
        errors,
    })
}

/// Parsed proxies not present in `existing`: builds labelled rows and inserts
/// them. Split out from the command so it can be unit-tested against an
/// in-memory SQLite connection. Returns (proxy, password to store) in
/// insertion order and the number of duplicates skipped.
fn import_insert_rows(
    conn: &Connection,
    parsed: Vec<crate::proxy_import::ParsedProxy>,
    label_prefix: Option<&str>,
    existing: &std::collections::HashSet<(String, String, i64, Option<String>)>,
) -> AppResult<(Vec<(Proxy, Option<String>)>, u32)> {
    let mut duplicates = 0u32;
    let mut rows = Vec::with_capacity(parsed.len());
    for p in parsed {
        let key = (p.protocol.clone(), p.host.clone(), p.port, p.username.clone());
        if existing.contains(&key) {
            duplicates += 1;
            continue;
        }
        let proxy = Proxy {
            id: uuid::Uuid::new_v4().to_string(),
            label: format!("{}{}:{}", label_prefix.unwrap_or(""), p.host, p.port),
            protocol: p.protocol,
            host: p.host,
            port: p.port,
            username: p.username,
            created_at: chrono::Utc::now().timestamp(),
        };
        insert_proxy(conn, &proxy)?;
        rows.push((proxy, p.password));
    }
    Ok((rows, duplicates))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE proxies (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                protocol TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT,
                created_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn parsed(host: &str, port: i64, user: Option<&str>) -> crate::proxy_import::ParsedProxy {
        crate::proxy_import::ParsedProxy {
            protocol: "socks5".into(),
            host: host.into(),
            port,
            username: user.map(str::to_string),
            password: None,
        }
    }

    #[test]
    fn import_inserts_rows_and_counts_duplicates() {
        let conn = memory_db();
        let list = vec![parsed("1.2.3.4", 3128, None), parsed("5.6.7.8", 3128, Some("u"))];
        let (rows, dup) = import_insert_rows(&conn, list.clone(), Some("ISP — "), &Default::default()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(dup, 0);
        assert_eq!(rows[0].0.label, "ISP — 1.2.3.4:3128");
        assert_eq!(rows[1].0.username.as_deref(), Some("u"));
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM proxies", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        // Re-importing the same list is all duplicates.
        let existing: std::collections::HashSet<_> = conn
            .prepare("SELECT protocol, host, port, username FROM proxies")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let (rows, dup) = import_insert_rows(&conn, list, Some("ISP — "), &existing).unwrap();
        assert!(rows.is_empty());
        assert_eq!(dup, 2);
    }
}

/// The previous uses of an exit IP, newest first. Pure SQL (no Tauri), so it
/// is unit-tested against an in-memory connection.
pub fn list_proxy_exit_tests(conn: &Connection, proxy_id: &str) -> AppResult<Vec<ProxyExitTest>> {
    let mut stmt = conn.prepare(
        "SELECT id, proxy_id, ip, country, country_code, asn, asn_org, tested_at
         FROM proxy_test_history
         WHERE proxy_id = ?1
         ORDER BY tested_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map(params![proxy_id], |r| {
            Ok(ProxyExitTest {
                id: r.get("id")?,
                proxy_id: r.get("proxy_id")?,
                ip: r.get("ip")?,
                country: r.get("country")?,
                country_code: r.get("country_code")?,
                asn: r.get("asn")?,
                asn_org: r.get("asn_org")?,
                tested_at: r.get("tested_at")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// True when this exact IP was never recorded by an earlier successful test,
/// plus (count, latest `tested_at`) when it was. Pure SQL, unit-tested.
fn exit_ip_seen_before(conn: &Connection, ip: &str) -> AppResult<Option<(i64, i64)>> {
    let seen: i64 = conn.query_row(
        "SELECT COUNT(*) FROM proxy_test_history WHERE ip = ?1",
        params![ip],
        |r| r.get(0),
    )?;
    if seen == 0 {
        return Ok(None);
    }
    let last_seen_at: i64 = conn.query_row(
        "SELECT MAX(tested_at) FROM proxy_test_history WHERE ip = ?1",
        params![ip],
        |r| r.get(0),
    )?;
    Ok(Some((seen, last_seen_at)))
}

/// Persists one successful test's exit identity. Called only for successes:
/// a failed test saw no exit IP, so it must not enter the "used before" set.
fn record_exit_test(
    conn: &Connection,
    proxy_id: &str,
    ip: &str,
    country: Option<&str>,
    country_code: Option<&str>,
    asn: Option<&str>,
    asn_org: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO proxy_test_history (proxy_id, ip, country, country_code, asn, asn_org, tested_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            proxy_id,
            ip,
            country,
            country_code,
            asn,
            asn_org,
            chrono::Utc::now().timestamp()
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod test_history_tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE proxies (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                protocol TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE proxy_test_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                proxy_id TEXT NOT NULL REFERENCES proxies(id) ON DELETE CASCADE,
                ip TEXT NOT NULL,
                country TEXT,
                country_code TEXT,
                asn TEXT,
                asn_org TEXT,
                tested_at INTEGER NOT NULL
            );
            CREATE INDEX idx_proxy_test_history_ip ON proxy_test_history(ip);
            CREATE INDEX idx_proxy_test_history_proxy ON proxy_test_history(proxy_id);",
        )
        .unwrap();
        conn
    }

    fn proxy(id: &str) -> Proxy {
        Proxy {
            id: id.to_string(),
            label: "test".to_string(),
            protocol: "socks5".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: None,
            created_at: 0,
        }
    }

    #[test]
    fn unseen_ip_is_fresh_and_seen_ip_reports_count() {
        let conn = memory_db();
        let p = proxy("p1");
        insert_proxy(&conn, &p).unwrap();

        assert_eq!(exit_ip_seen_before(&conn, "203.0.113.7").unwrap(), None);

        record_exit_test(&conn, "p1", "203.0.113.7", Some("Indonesia"), Some("ID"), Some("AS138089"), Some("Fiberbyte")).unwrap();
        let (count, last) = exit_ip_seen_before(&conn, "203.0.113.7").unwrap().unwrap();
        assert_eq!(count, 1);
        assert!(last > 0);

        record_exit_test(&conn, "p1", "203.0.113.7", Some("Indonesia"), Some("ID"), None, None).unwrap();
        let (count, _) = exit_ip_seen_before(&conn, "203.0.113.7").unwrap().unwrap();
        assert_eq!(count, 2);

        // The recording is per exit IP, not per proxy: another proxy that
        // lands on the same IP is seen as "previously used" too.
        let p2 = proxy("p2");
        insert_proxy(&conn, &p2).unwrap();
        record_exit_test(&conn, "p2", "198.51.100.9", None, None, None, None).unwrap();
        assert_eq!(exit_ip_seen_before(&conn, "198.51.100.9").unwrap().unwrap().0, 1);
    }

    #[test]
    fn list_returns_newest_first_and_cascade_delete_removes_them() {
        let conn = memory_db();
        let p = proxy("p1");
        insert_proxy(&conn, &p).unwrap();

        record_exit_test(&conn, "p1", "203.0.113.1", Some("A"), None, None, None).unwrap();
        record_exit_test(&conn, "p1", "203.0.113.2", Some("B"), None, None, None).unwrap();

        let tests = list_proxy_exit_tests(&conn, "p1").unwrap();
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].ip, "203.0.113.2");
        assert_eq!(tests[1].ip, "203.0.113.1");
        assert_eq!(tests[0].country.as_deref(), Some("B"));

        conn.execute("DELETE FROM proxies WHERE id = 'p1'", []).unwrap();
        assert!(list_proxy_exit_tests(&conn, "p1").unwrap().is_empty());
    }

}
