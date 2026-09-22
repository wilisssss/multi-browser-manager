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
                AppError::NotFound(format!("Proxy {id} not found"))
            }
            other => other.into(),
        })
}

fn validate_label(label: &str) -> AppResult<()> {
    if label.trim().is_empty() {
        return Err(AppError::Validation("Proxy label cannot be empty".into()));
    }
    Ok(())
}

fn validate_port(port: i64) -> AppResult<()> {
    if !(1..=65535).contains(&port) {
        return Err(AppError::Validation("Port must be between 1 and 65535".into()));
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
    let conn = db_lock(&state)?;

    validate_label(&input.label)?;
    proxy_manager::ensure_valid_protocol(&input.protocol)?;
    validate_port(input.port)?;
    if input.host.trim().is_empty() {
        return Err(AppError::Validation("Proxy host cannot be empty".into()));
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

    insert_proxy_with_password(&conn, &proxy, input.password.as_deref())?;
    Ok(proxy)
}

#[tauri::command]
pub fn update_proxy(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProxyInput,
) -> AppResult<Proxy> {
    let conn = db_lock(&state)?;
    let existing = get_proxy_by_id(&conn, &id)?;

    if let Some(label) = &input.label {
        validate_label(label)?;
    }
    if let Some(protocol) = &input.protocol {
        proxy_manager::ensure_valid_protocol(protocol)?;
    }
    if let Some(port) = input.port {
        validate_port(port)?;
    }

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

    // Password: absent = keep; explicit null or empty = remove from keychain;
    // non-empty = replace.
    match input.password {
        Some(Some(pass)) if !pass.is_empty() => {
            proxy_manager::store_proxy_password(&id, &pass)?;
        }
        Some(_) => {
            proxy_manager::delete_proxy_password(&id)?;
        }
        None => {}
    }

    get_proxy_by_id(&conn, &id)
}

#[tauri::command]
pub fn delete_proxy(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = db_lock(&state)?;

    // Refuse to delete a proxy that is still assigned to profiles.
    let in_use: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM profiles WHERE proxy_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if in_use {
        return Err(AppError::Validation(
            "This proxy is still assigned to one or more profiles. Unassign it first.".into(),
        ));
    }

    conn.execute("DELETE FROM proxies WHERE id = ?1", params![id])?;
    proxy_manager::delete_proxy_password(&id)?;
    Ok(())
}

#[tauri::command]
pub async fn test_proxy(state: State<'_, AppState>, id: String) -> AppResult<ProxyTestResult> {
    let (proxy, password) = {
        let conn = db_lock(&state)?;
        let proxy = get_proxy_by_id(&conn, &id)?;
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
        (proxy, password)
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
