use crate::commands::profile::db_lock;
use crate::error::{AppError, AppResult};
use crate::models::proxy::{CreateProxyInput, Proxy, ProxyTestResult, UpdateProxyInput};
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

#[tauri::command]
pub async fn test_proxy(state: State<'_, AppState>, id: String) -> AppResult<ProxyTestResult> {
    let proxy = {
        let conn = db_lock(&state)?;
        get_proxy_by_id(&conn, &id)?
    };
    // Keychain read AFTER the DB lock is released (blocking D-Bus round trip,
    // see create_proxy for why it must never hold the shared mutex).
    let password = if proxy
        .username
        .as_deref()
        .map(|u| !u.is_empty())
        .unwrap_or(false)
    {
        Some(proxy_manager::get_proxy_password(&id)?)
    } else {
        None
    };

    proxy_manager::test_proxy_connection(
        &proxy.protocol,
        &proxy.host,
        proxy.port,
        proxy.username.as_deref(),
        password.as_deref(),
    )
    .await
}

#[tauri::command]
pub fn get_proxies(state: State<'_, AppState>) -> AppResult<Vec<Proxy>> {
    let conn = db_lock(&state)?;
    list_proxies(&conn)
}
