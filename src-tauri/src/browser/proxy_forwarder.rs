//! Local authenticated-proxy forwarder.
//!
//! Chromium cannot carry proxy credentials on `--proxy-server`, and its MV3
//! extension auth (`onAuthRequired`) is unreliable: synchronous `blocking`
//! handlers cannot wake a sleeping service worker, and `asyncBlocking`
//! delivery is flaky for CONNECT tunnels. Both failure modes leave the
//! browser unable to reach the internet through an authenticated proxy.
//!
//! This forwarder removes the auth problem entirely: the browser talks to
//! `127.0.0.1:<port>` with no credentials, and every request is relayed to
//! the upstream proxy with a `Proxy-Authorization` header (HTTP upstream) or
//! a SOCKS5 user/pass handshake (SOCKS5 upstream) injected BEFORE the first
//! byte leaves the machine. The upstream therefore always sees an
//! authenticated request — exit selection honors the username's filters and
//! no 407 round-trip ever happens inside the browser.
//!
//! Credentials live only in memory (never on disk, never in argv).

use base64::Engine as _;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// Upstream proxy the forwarder relays to.
#[derive(Debug, Clone)]
pub struct UpstreamConfig {
    /// `http` or `socks5` (same vocabulary as the proxies table).
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

/// Owns a running forwarder. Dropping it (or calling `shutdown`) stops the
/// listener and tears down active relay tasks.
pub struct ForwarderGuard {
    pub local_port: u16,
    shutdown: Arc<tokio::sync::Notify>,
    conns: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    listener_task: tokio::task::JoinHandle<()>,
}

impl ForwarderGuard {
    /// Stop accepting and drop all active relays. Idempotent.
    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
        if let Ok(mut conns) = self.conns.try_lock() {
            for t in conns.drain(..) {
                t.abort();
            }
        }
        self.listener_task.abort();
    }
}

impl Drop for ForwarderGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Bind a forwarder on an ephemeral `127.0.0.1` port.
pub async fn spawn(upstream: UpstreamConfig) -> std::io::Result<ForwarderGuard> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let local_port = listener.local_addr()?.port();
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let conns: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
    let upstream = Arc::new(upstream);

    let shutdown_loop = shutdown.clone();
    let conns_loop = conns.clone();
    let listener_task = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = shutdown_loop.notified() => break,
                accepted = listener.accept() => accepted,
            };
            let Ok((client, _)) = accepted else { break };
            let upstream = upstream.clone();
            let conns = conns_loop.clone();
            let task = tokio::spawn(async move {
                let _ = handle_conn(client, &upstream).await;
            });
            // Reap finished tasks, keep the live ones.
            let mut live = conns.lock().await;
            live.retain(|t| !t.is_finished());
            live.push(task);
        }
    });

    Ok(ForwarderGuard { local_port, shutdown, conns, listener_task })
}

async fn handle_conn(mut client: TcpStream, upstream: &UpstreamConfig) -> std::io::Result<()> {
    let head = read_head(&mut client).await?;
    let first_line = first_line(&head);
    if first_line.starts_with("CONNECT ") {
        let target = first_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad CONNECT"))?;
        tunnel(&mut client, target, upstream).await
    } else {
        plain_http(&mut client, &head, upstream, &first_line).await
    }
}

/// HTTPS (and other tunneled) traffic: `CONNECT host:port` from the browser.
async fn tunnel(client: &mut TcpStream, target: &str, up: &UpstreamConfig) -> std::io::Result<()> {
    let mut upstream = match up.protocol.as_str() {
        "socks5" => socks5_connect(up, target).await?,
        _ => {
            let mut s = TcpStream::connect((up.host.as_str(), up.port)).await?;
            let req = format!(
                "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Authorization: {auth}\r\nProxy-Connection: keep-alive\r\n\r\n",
                auth = basic_auth(up),
            );
            s.write_all(req.as_bytes()).await?;
            let head = read_head(&mut s).await?;
            let ok = head.starts_with(b"HTTP/1.1 200") || head.starts_with(b"HTTP/1.0 200");
            if !ok {
                // Surface the proxy's rejection to the browser verbatim.
                client.write_all(&head).await?;
                return Ok(());
            }
            s
        }
    };
    client
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await?;
    let _ = tokio::io::copy_bidirectional(client, &mut upstream).await;
    Ok(())
}

/// Plain-HTTP proxying: the browser sends an absolute-form request. We force
/// `Connection: close` so exactly one request flows per client connection,
/// which keeps the auth injection and response framing trivially correct.
async fn plain_http(
    client: &mut TcpStream,
    head: &[u8],
    up: &UpstreamConfig,
    first_line: &str,
) -> std::io::Result<()> {
    let mut upstream = match up.protocol.as_str() {
        "socks5" => {
            let target = absolute_url_target(first_line)?;
            socks5_connect(up, &target).await?
        }
        _ => TcpStream::connect((up.host.as_str(), up.port)).await?,
    };

    let auth = if up.protocol == "socks5" { None } else { Some(basic_auth(up)) };
    let modified = rewrite_head(
        head,
        auth.as_deref(),
        true,
    );
    upstream.write_all(&modified).await?;
    // Request body, if any.
    if let Some(len) = content_length(head) {
        let mut left = len;
        let mut buf = vec![0u8; 16 * 1024];
        while left > 0 {
            let want = buf.len().min(left);
            let n = client.read(&mut buf[..want]).await?;
            if n == 0 {
                break;
            }
            upstream.write_all(&buf[..n]).await?;
            left -= n;
        }
    }
    // Response: with Connection: close the upstream closes at the end.
    let _ = tokio::io::copy(&mut upstream, client).await;
    Ok(())
}

fn basic_auth(up: &UpstreamConfig) -> String {
    let raw = format!("{}:{}", up.username, up.password);
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw.as_bytes()))
}

/// SOCKS5 (RFC 1928) connect with optional user/pass auth (RFC 1929).
async fn socks5_connect(up: &UpstreamConfig, target: &str) -> std::io::Result<TcpStream> {
    let (host, port) = target
        .rsplit_once(':')
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad target"))?;
    let port: u16 = port.parse().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "bad target port")
    })?;
    let host = host.trim_matches(|c| c == '[' || c == ']');

    let mut s = TcpStream::connect((up.host.as_str(), up.port)).await?;
    let wants_auth = !up.username.is_empty();
    s.write_all(&[0x05, 0x01, if wants_auth { 0x02 } else { 0x00 }]).await?;
    let mut chosen = [0u8; 2];
    s.read_exact(&mut chosen).await?;
    if chosen[0] != 0x05 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a SOCKS5 proxy",
        ));
    }
    if chosen[1] == 0x02 {
        let mut handshake = Vec::with_capacity(3 + up.username.len() + up.password.len());
        handshake.push(0x01);
        handshake.push(up.username.len() as u8);
        handshake.extend_from_slice(up.username.as_bytes());
        handshake.push(up.password.len() as u8);
        handshake.extend_from_slice(up.password.as_bytes());
        s.write_all(&handshake).await?;
        let mut verdict = [0u8; 2];
        s.read_exact(&mut verdict).await?;
        if verdict[1] != 0x00 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "SOCKS5 upstream rejected credentials",
            ));
        }
    } else if chosen[1] != 0x00 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "SOCKS5 upstream requires an unsupported auth method",
        ));
    }

    let mut req = Vec::with_capacity(7 + host.len());
    req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, host.len() as u8]);
    req.extend_from_slice(host.as_bytes());
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await?;
    let mut reply = [0u8; 4];
    s.read_exact(&mut reply).await?;
    if reply[1] != 0x00 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("SOCKS5 upstream connect failed (code {})", reply[1]),
        ));
    }
    // Drain the bound address.
    let skip = match reply[3] {
        0x01 => 4 + 2,
        0x03 => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            1 + l[0] as usize + 2
        }
        0x04 => 16 + 2,
        _ => 0,
    };
    let mut sink = vec![0u8; skip];
    if skip > 0 {
        s.read_exact(&mut sink).await?;
    }
    Ok(s)
}

/// Reads until the end of the header block (`\r\n\r\n`), inclusive.
async fn read_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::with_capacity(1024);
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Ok(head);
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            return Ok(head);
        }
        if head.len() > 128 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "header block too large",
            ));
        }
    }
}

fn first_line(head: &[u8]) -> String {
    let end = head.windows(2).position(|w| w == b"\r\n").unwrap_or(head.len());
    String::from_utf8_lossy(&head[..end]).into_owned()
}

/// `CONNECT host:port` → `host:port`; `METHOD http://host:port/…` → `host:port`.
fn absolute_url_target(first_line: &str) -> std::io::Result<String> {
    let raw = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad request line"))?;
    if raw.contains("://") {
        let rest = raw.split("://").nth(1).unwrap_or(raw);
        let authority = rest.split(['/']).next().unwrap_or(rest);
        if authority.contains(':') {
            return Ok(authority.to_string());
        }
        // Default ports for plain http.
        return Ok(format!("{authority}:80"));
    }
    Ok(raw.to_string())
}

/// Rebuilds a header block: drops `Connection`/`Proxy-Connection`, optionally
/// appends a `Proxy-Authorization`, forces `Connection: close`.
fn rewrite_head(head: &[u8], proxy_auth: Option<&str>, force_close: bool) -> Vec<u8> {
    let text_end = head
        .windows(4)
        .rposition(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(head.len());
    let body = head.get(text_end..).unwrap_or(&[]);
    let mut out = Vec::with_capacity(head.len() + 64);
    let mut first = true;
    for line in String::from_utf8_lossy(&head[..text_end])
        .lines()
        .filter(|l| !l.is_empty())
    {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("connection:") || lower.starts_with("proxy-connection:") {
            continue;
        }
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(b"\r\n");
        if first {
            first = false;
            if let Some(auth) = proxy_auth {
                out.extend_from_slice(format!("Proxy-Authorization: {auth}\r\n").as_bytes());
            }
        }
    }
    if force_close {
        out.extend_from_slice(b"Connection: close\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

fn content_length(head: &[u8]) -> Option<usize> {
    String::from_utf8_lossy(head)
        .lines()
        .find_map(|l| {
            let lower = l.to_ascii_lowercase();
            if lower.starts_with("content-length:") {
                l.split_once(':').and_then(|(_, v)| v.trim().parse().ok())
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_injects_auth_and_forces_close() {
        let head = b"GET http://example.com/x HTTP/1.1\r\nHost: example.com\r\nProxy-Connection: keep-alive\r\n\r\nbody";
        let out = rewrite_head(head, Some("Basic eHg6eXk="), true);
        let s = String::from_utf8_lossy(&out);
        assert!(s.starts_with("GET http://example.com/x HTTP/1.1\r\n"));
        assert!(s.contains("Proxy-Authorization: Basic eHg6eXk=\r\n"));
        assert!(s.contains("Connection: close\r\n"));
        assert!(!s.contains("Proxy-Connection"), "old connection header must go");
        assert!(s.ends_with("\r\n\r\nbody"), "body must be preserved");
    }

    #[test]
    fn rewrite_without_auth_keeps_request_intact() {
        let head = b"POST http://h:8080/p HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi";
        let out = rewrite_head(head, None, true);
        let s = String::from_utf8_lossy(&out);
        assert!(!s.contains("Proxy-Authorization"));
        assert!(s.contains("Content-Length: 2"));
        assert!(s.ends_with("\r\n\r\nhi"));
    }

    #[test]
    fn target_parsing() {
        assert_eq!(
            absolute_url_target("CONNECT api.example.com:443 HTTP/1.1").unwrap(),
            "api.example.com:443"
        );
        assert_eq!(
            absolute_url_target("GET http://example.com:8080/x HTTP/1.1").unwrap(),
            "example.com:8080"
        );
        assert_eq!(
            absolute_url_target("GET http://example.com/x HTTP/1.1").unwrap(),
            "example.com:80"
        );
        assert!(absolute_url_target("GET / HTTP/1.1").is_ok()); // origin-form → returned as-is
    }
}
