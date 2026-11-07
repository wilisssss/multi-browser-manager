//! Parser for proxy lists fetched from an endpoint URL.
//!
//! Providers serve their lists in a handful of shapes, so the parser accepts
//! all of them, per line:
//!
//! - `http://user:pass@host:port` (scheme, percent-encoded credentials)
//! - `user:pass@host:port`
//! - `host:port:user:pass`
//! - `host:port`
//! - `host:port user:pass`
//! - a JSON array of objects, or an object wrapping such an array under
//!   `proxies` / `data` / `items` / `rows`, with keys
//!   `host|ip|address|server`, `port`, `username|user|login`,
//!   `password|pass`, `protocol|scheme|type`
//!
//! Lines starting with `#` and blank lines are ignored. Credentials are
//! percent-decoded (`user:pa%40ss@host:8080` → `pa@ss`), which is what every
//! provider's own `curl`-friendly examples do.
//!
//! The parser is deliberately strict about ports and host shape; a rejected
//! line is reported, never silently imported.

use percent_encoding::percent_decode_str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProxy {
    /// Canonical protocol: `http`, `https`, `socks4` or `socks5`.
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
    pub line: usize,
    pub reason: String,
}

const MAX_LINE: usize = 512;
const MAX_HOST: usize = 253;

/// Canonical name for a scheme token, with the SOCKS aliases folded in.
fn canonical_scheme(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "http" => Some("http"),
        "https" => Some("https"),
        "socks" | "socks5" | "socks5h" => Some("socks5"),
        "socks4" | "socks4a" => Some("socks4"),
        _ => None,
    }
}

fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().to_string()
}

fn validate_host(host: &str) -> Result<String, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("empty host".into());
    }
    if host.len() > MAX_HOST {
        return Err("host is too long".into());
    }
    // Bracketed IPv6 and ordinary labels; no slashes, whitespace or userinfo.
    if host.starts_with('[') {
        let ok = host.ends_with(']')
            && host[1..host.len() - 1].chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.');
        return if ok { Ok(host.to_string()) } else { Err("invalid bracketed IPv6 address".into()) };
    }
    if host.chars().any(|c| c == '/' || c == '@' || c.is_whitespace()) {
        return Err("host contains illegal characters".into());
    }
    Ok(host.to_string())
}

/// `[::1]:8080` / `1.2.3.4:8080` / `example.com:8080` → (host, port).
fn parse_host_port(s: &str) -> Result<(String, i64), String> {
    let s = s.trim();
    let Some(colon) = s.rfind(':') else {
        return Err("missing port".into());
    };
    // `host:port` → host before the colon, port after it.
    let host = &s[..colon];
    let port: i64 = s[colon + 1..]
        .parse()
        .map_err(|_| "port is not a number".to_string())?;
    if !(1..=65535).contains(&port) {
        return Err("port must be between 1 and 65535".into());
    }
    Ok((validate_host(host)?, port))
}

fn split_credentials(s: &str) -> (Option<String>, Option<String>) {
    let decoded = decode(s.trim());
    match decoded.split_once(':') {
        Some((u, p)) => {
            let user = u.trim();
            let pass = p; // may be empty; leading ':' and spaces are kept verbatim
            (
                (!user.is_empty()).then(|| user.to_string()),
                (!pass.is_empty()).then(|| pass.to_string()),
            )
        }
        None => (
            (!decoded.trim().is_empty()).then(|| decoded.trim().to_string()),
            None,
        ),
    }
}

/// Parse a single line; see module docs for the accepted formats.
pub fn parse_line(raw: &str, line_no: usize, default_protocol: &str) -> Result<ParsedProxy, ParseFailure> {
    let line = raw.trim();
    let fail = |reason: String| ParseFailure { line: line_no, reason };
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return Err(fail("empty line".into()));
    }
    if line.len() > MAX_LINE {
        return Err(fail("line too long".into()));
    }

    // Whitespace-separated `host:port user:pass`?
    let mut line = line.to_string();
    if line.split_whitespace().count() == 2 && !line.contains('@') {
        let mut parts = line.split_whitespace();
        let hp = parts.next().unwrap_or("");
        let creds = parts.next().unwrap_or("");
        if creds.contains(':') {
            line = format!("{creds}@{hp}");
        }
    }

    let mut protocol = canonical_scheme(default_protocol).map(str::to_string).ok_or_else(|| {
        fail(format!("unsupported default protocol `{default_protocol}`"))
    })?;

    let remainder = match line.find("://") {
        Some(at) => {
            let scheme = &line[..at];
            protocol = canonical_scheme(scheme)
                .map(str::to_string)
                .ok_or_else(|| fail(format!("unsupported scheme `{scheme}`")))?;
            line[at + 3..].trim().to_string()
        }
        None => line,
    };

    let (creds, host_port_part): (Option<&str>, &str) = match remainder.rfind('@') {
        Some(at) => (Some(remainder[..at].trim()), remainder[at + 1..].trim()),
        None => (None, remainder.as_str()),
    };

    let mut username = None;
    let mut password = None;
    if let Some(creds) = creds {
        if creds.is_empty() {
            return Err(fail("empty credentials before `@`".into()));
        }
        let (u, p) = split_credentials(creds);
        username = u;
        password = p;
        if username.is_none() {
            return Err(fail("missing username before `@`".into()));
        }
    }

    match parse_host_port(host_port_part) {
        Ok((host, port)) => Ok(ParsedProxy { protocol, host, port, username, password }),
        Err(e) => {
            // `host:port:user:pass` (no userinfo marker)
            let parts: Vec<&str> = host_port_part.split(':').collect();
            if creds.is_none() && parts.len() == 4 {
                let host = parts[0];
                let port: i64 = parts[1].parse().map_err(|_| fail("port is not a number".into()))?;
                if !(1..=65535).contains(&port) {
                    return Err(fail("port must be between 1 and 65535".into()));
                }
                let host = validate_host(host).map_err(fail)?;
                let username = decode(parts[2]);
                let password = decode(parts[3]);
                let username = if username.trim().is_empty() { None } else { Some(username) };
                let password = if password.trim().is_empty() { None } else { Some(password) };
                if username.is_none() {
                    return Err(fail("empty username".into()));
                }
                Ok(ParsedProxy { protocol, host, port, username, password })
            } else {
                Err(fail(e))
            }
        }
    }
}

fn json_item_to_parsed(v: &serde_json::Value, line_no: usize) -> Result<ParsedProxy, ParseFailure> {
    let fail = |reason: &str| ParseFailure { line: line_no, reason: reason.to_string() };
    let str_of = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| v.get(*k))
            .and_then(|x| match x {
                serde_json::Value::String(s) => Some(s.trim().to_string()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
    };
    let host = str_of(&["host", "ip", "address", "server"]).ok_or_else(|| fail("missing `host`"))?;
    let port_raw = str_of(&["port"]).ok_or_else(|| fail("missing `port`"))?;
    let port: i64 = port_raw.parse().map_err(|_| fail("`port` is not a number"))?;
    if !(1..=65535).contains(&port) {
        return Err(fail("port must be between 1 and 65535"));
    }
    let scheme = str_of(&["protocol", "scheme", "type"]);
    let protocol = match scheme.as_deref().map(canonical_scheme) {
        Some(Some(p)) => p.to_string(),
        Some(None) => return Err(fail("unsupported scheme in JSON object")),
        None => return Err(fail("missing `protocol` in JSON object")),
    };
    let username = str_of(&["username", "user", "login"]);
    let password = str_of(&["password", "pass", "passwd"]);
    Ok(ParsedProxy { protocol, host, port, username, password })
}

/// Parse an endpoint response body: line-delimited, falling back to a JSON
/// array of objects (optionally wrapped in `proxies`/`data`/`items`/`rows`).
/// Empty lines are ignored; `#` comments are ignored. Returns the parsed
/// proxies (duplicates removed, first occurrence wins) and the failures.
pub fn parse_body(body: &str, default_protocol: &str) -> (Vec<ParsedProxy>, Vec<ParseFailure>) {
    let trimmed = body.trim();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let array = if value.is_array() {
                value.as_array().cloned()
            } else {
                ["proxies", "data", "items", "rows", "result"]
                    .iter()
                    .find_map(|k| value.get(*k).and_then(|v| v.as_array()))
                    .cloned()
            };
            if let Some(items) = array {
                let mut parsed = Vec::new();
                let mut failures = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    match json_item_to_parsed(item, i + 1) {
                        Ok(p) => parsed.push(p),
                        Err(f) => failures.push(f),
                    }
                }
                return (dedupe(parsed), failures);
            }
            return (Vec::new(), vec![ParseFailure { line: 0, reason: "JSON object has no `proxies`/`data`/`items`/`rows` array".into() }]);
        }
        return (Vec::new(), vec![ParseFailure { line: 0, reason: "body is not valid JSON".into() }]);
    }

    let mut parsed = Vec::new();
    let mut failures = Vec::new();
    for (i, raw) in trimmed.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        match parse_line(line, line_no, default_protocol) {
            Ok(p) => parsed.push(p),
            Err(f) => failures.push(f),
        }
    }
    (dedupe(parsed), failures)
}

/// First occurrence wins; order is preserved.
fn dedupe(items: Vec<ParsedProxy>) -> Vec<ParsedProxy> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| seen.insert((p.protocol.clone(), p.host.clone(), p.port, p.username.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scheme_userinfo_and_host_port() {
        let p = parse_line("socks5://alice:s3cret@proxy.example.com:1080", 1, "http").unwrap();
        assert_eq!(p.protocol, "socks5");
        assert_eq!(p.host, "proxy.example.com");
        assert_eq!(p.port, 1080);
        assert_eq!(p.username.as_deref(), Some("alice"));
        assert_eq!(p.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn percent_decodes_credentials() {
        let p = parse_line("http://user:pa%40ss%3Aword@example.com:8080", 1, "http").unwrap();
        assert_eq!(p.username.as_deref(), Some("user"));
        assert_eq!(p.password.as_deref(), Some("pa@ss:word"));
    }

    #[test]
    fn accepts_host_port_user_pass_four_field_line() {
        let p = parse_line("1.2.3.4:3128:alice:s3cret", 3, "http").unwrap();
        assert_eq!(p.protocol, "http");
        assert_eq!(p.host, "1.2.3.4");
        assert_eq!(p.port, 3128);
        assert_eq!(p.username.as_deref(), Some("alice"));
        assert_eq!(p.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn accepts_plain_host_port() {
        let p = parse_line("1.2.3.4:3128", 1, "https").unwrap();
        assert_eq!(p.protocol, "https");
        assert_eq!(p.username, None);
    }

    #[test]
    fn accepts_whitespace_user_pass_after_host_port() {
        let p = parse_line("1.2.3.4:3128 alice:s3cret", 1, "http").unwrap();
        assert_eq!(p.host, "1.2.3.4");
        assert_eq!(p.port, 3128);
        assert_eq!(p.username.as_deref(), Some("alice"));
        assert_eq!(p.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn socks_scheme_aliases_fold_into_canonical_names() {
        assert_eq!(parse_line("socks4a://h:1080", 1, "http").unwrap().protocol, "socks4");
        assert_eq!(parse_line("socks5h://h:1080", 1, "http").unwrap().protocol, "socks5");
        assert_eq!(parse_line("socks://h:1080", 1, "http").unwrap().protocol, "socks5");
    }

    #[test]
    fn rejects_bad_lines_with_reasons() {
        assert!(parse_line("no-port", 1, "http").is_err());
        assert!(parse_line("http://host:0", 1, "http").is_err());
        assert!(parse_line("http://host:70000", 1, "http").is_err());
        assert!(parse_line("http://host:abc", 1, "http").is_err());
        assert!(parse_line("ftp://host:8080", 1, "http").is_err());
        assert!(parse_line("1.2.3.4:3128:alice", 1, "http").is_err());
    }

    #[test]
    fn parses_bracketed_ipv6() {
        let p = parse_line("socks5://u:p@[2001:db8::1]:1080", 1, "http").unwrap();
        assert_eq!(p.host, "[2001:db8::1]");
        assert_eq!(p.port, 1080);
    }

    #[test]
    fn body_parse_ignores_blanks_and_dedupes() {
        let body = "# comment\n\nhttp://u:p@1.2.3.4:8080\nhttp://u:p@1.2.3.4:8080\nhttps://u:p@2.3.4.5:443\n";
        let (parsed, failures) = parse_body(body, "http");
        assert_eq!(parsed.len(), 2);
        assert!(failures.is_empty());
    }

    #[test]
    fn body_parse_reports_failing_lines_with_numbers() {
        let body = "http://u:p@1.2.3.4:8080\ngarbage\n# ok line\nftp://nope:21\n";
        let (parsed, failures) = parse_body(body, "http");
        assert_eq!(parsed.len(), 1);
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].line, 2);
        assert_eq!(failures[1].line, 4);
    }

    #[test]
    fn parses_json_object_list() {
        let body = r#"[{"ip":"1.2.3.4","port":3128,"username":"u","password":"p","protocol":"socks5"},
                        {"host":"2.3.4.5","port":8080,"protocol":"https"}]"#;
        let (parsed, failures) = parse_body(body, "http");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].host, "1.2.3.4");
        assert_eq!(parsed[0].port, 3128);
        assert_eq!(parsed[0].protocol, "socks5");
        assert_eq!(parsed[1].protocol, "https");
        assert!(failures.is_empty());
    }

    #[test]
    fn parses_json_wrapper_object() {
        let body = r#"{"proxies":[{"ip":"1.2.3.4","port":3128,"protocol":"http"}]}"#;
        let (parsed, failures) = parse_body(body, "http");
        assert_eq!(parsed.len(), 1);
        assert!(failures.is_empty());
    }

    #[test]
    fn invalid_json_reports_a_failure() {
        let (parsed, failures) = parse_body("{not json", "http");
        assert!(parsed.is_empty());
        assert_eq!(failures.len(), 1);
    }
}
