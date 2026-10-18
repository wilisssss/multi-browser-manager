//! Chrome DevTools Protocol control for a launched profile.
//!
//! Chromium's branded builds ignore `--load-extension`, so per-profile identity
//! (viewport, user agent + Client Hints, timezone, the fingerprint script) is
//! applied over CDP instead: the same mechanism Puppeteer's `setViewport` uses.
//! Each profile gets its own local debugging port.
//!
//! The browser-level WebSocket is used with `Target.setAutoAttach`, so every
//! page gets the overrides — including tabs opened after launch. Emulation
//! overrides are per-session and are dropped when a session ends, so each
//! session is configured as it attaches and the browser connection is held for
//! the lifetime of the process.

use crate::error::{AppError, AppResult};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::net::TcpListener;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// The browser WebSocket, which multiplexes per-target sessions.
type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// One message from the browser: a reply to one of our commands, or an event.
enum Frame {
    Reply(u64, Value),
    Event(Value),
}

/// Everything applied to a page right after the browser starts.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Full fingerprint script, run before any page script.
    pub init_script: Option<String>,
    pub user_agent: Option<String>,
    /// Client Hints reported through `navigator.userAgentData`.
    pub client_hints: Option<ClientHints>,
    /// Viewport `(width, height)`, applied as a mobile-sized device when `mobile`.
    pub viewport: Option<(u32, u32)>,
    pub mobile: bool,
    /// Device pixel ratio for the emulated screen. Phones report a density
    /// bucket (2.5–3), never the host's 1, so it is derived per screen width.
    pub device_scale_factor: Option<f64>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientHints {
    pub platform: String,
    pub platform_version: String,
    pub architecture: String,
    pub model: String,
    pub mobile: bool,
    pub full_version: String,
}

/// Picks a free loopback TCP port for `--remote-debugging-port`.
pub fn pick_debug_port() -> AppResult<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Applies `plan` to every page of the browser on `port`, now and for the rest
/// of the process's life. Returns only once the browser connection closes.
///
/// Overrides are per-session: they are dropped when the session that set them
/// ends, and a new tab starts a new session. So rather than configuring the
/// first page once, the browser endpoint is attached and every page session is
/// configured as it appears.
pub async fn apply(port: u16, plan: &Plan) -> AppResult<()> {
    let ws_url = wait_for_browser_ws(port).await?;
    let (ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| AppError::validation(format!("CDP connect failed: {e}")))?;
    let (sink, stream) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(pump(stream, tx));
    let mut cdp = Connection { sink, rx, next_id: 0, pending: VecDeque::new() };

    // Attach to the pages that already exist and to every page opened later.
    // `waitForDebuggerOnStart` holds a new page until its overrides are applied,
    // so a tab never paints with the host's default viewport first.
    let auto_attach = json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true });
    cdp.send(None, "Target.setAutoAttach", auto_attach).await?;

    let commands = page_identity_commands(plan);
    let reload_initial = plan.init_script.is_some();
    // Guards against configuring the same page twice if it re-attaches.
    let mut configured: HashSet<String> = HashSet::new();

    while let Some(event) = cdp.next_event().await {
        if event.get("method").and_then(Value::as_str) != Some("Target.attachedToTarget") {
            continue;
        }
        let params = &event["params"];
        let Some(session) = params.get("sessionId").and_then(Value::as_str) else {
            continue;
        };
        let target = &params["targetInfo"];
        // A page created while attached is held mid-load; a page from before we
        // attached (or an iframe/worker) is not.
        let waiting = params
            .get("waitingForDebugger")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        // Non-page targets (services, browser chrome) only need resuming.
        if target.get("type").and_then(Value::as_str) != Some("page") {
            if waiting {
                let _ = cdp
                    .send(Some(session), "Runtime.runIfWaitingForDebugger", json!({}))
                    .await;
            }
            continue;
        }

        let Some(target_id) = target.get("targetId").and_then(Value::as_str) else {
            continue;
        };
        // Fresh session for this page: (re)apply the overrides.
        if configured.insert(target_id.to_string()) {
            for (method, params) in &commands {
                if let Err(e) = cdp.send(Some(session), method, params.clone()).await {
                    eprintln!("mbm: CDP {method} failed: {e}");
                }
            }
            // A page that loaded before the init script was registered must be
            // reloaded for it to take effect. A held page already ran it, so it
            // is left alone (no visible reload on a fresh tab).
            if reload_initial && !waiting {
                let _ = cdp.send(Some(session), "Page.reload", json!({})).await;
            }
        }
        if waiting {
            let _ = cdp
                .send(Some(session), "Runtime.runIfWaitingForDebugger", json!({}))
                .await;
        }
    }
    Ok(())
}

/// Forwards every message the browser sends into `tx`, keeping replies and
/// events on separate tracks. Chromium delivers `attachedToTarget` for the
/// pages that already exist *before* the reply to `setAutoAttach`, so anything
/// waiting for a reply must queue those events, never drop them.
async fn pump(mut stream: SplitStream<Ws>, tx: mpsc::UnboundedSender<Frame>) {
    while let Some(frame) = stream.next().await {
        let parsed = match frame {
            Ok(Message::Text(text)) => match serde_json::from_str::<Value>(&text) {
                Ok(value) => match value.get("id").and_then(Value::as_u64) {
                    Some(id) => Frame::Reply(id, value),
                    None if value.get("method").is_some() => Frame::Event(value),
                    None => continue,
                },
                Err(_) => continue,
            },
            Ok(_) => continue,
            Err(_) => break,
        };
        if tx.send(parsed).is_err() {
            break;
        }
    }
}

/// The write half plus the event stream. Events that arrive while a command
/// reply is awaited are queued in `pending`, not dropped.
struct Connection {
    sink: SplitSink<Ws, Message>,
    rx: mpsc::UnboundedReceiver<Frame>,
    next_id: u64,
    pending: VecDeque<Value>,
}

impl Connection {
    /// Sends one command, optionally into a flat session, and waits for its
    /// reply, queueing any events seen on the way.
    async fn send(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
    ) -> AppResult<Value> {
        self.next_id += 1;
        let cmd_id = self.next_id;
        let mut message = json!({ "id": cmd_id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        self.sink
            .send(Message::Text(message.to_string().into()))
            .await
            .map_err(|e| AppError::validation(format!("CDP send failed: {e}")))?;

        loop {
            let frame = tokio::time::timeout(Duration::from_secs(10), self.rx.recv())
                .await
                .map_err(|_| AppError::validation("CDP command timed out"))?;
            match frame {
                Some(Frame::Reply(id, reply)) if id == cmd_id => {
                    if let Some(err) = reply.get("error") {
                        return Err(AppError::validation(format!("CDP error: {err}")));
                    }
                    return Ok(reply);
                }
                Some(Frame::Reply(..)) => {}
                // This is what lost the startup tab: its attach event lands
                // here while `setAutoAttach` is still awaiting its reply.
                Some(Frame::Event(event)) => self.pending.push_back(event),
                None => return Err(AppError::validation("CDP connection closed")),
            }
        }
    }

    /// The next event to handle: queued ones first, then fresh ones.
    async fn next_event(&mut self) -> Option<Value> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        loop {
            match self.rx.recv().await {
                Some(Frame::Event(event)) => return Some(event),
                Some(Frame::Reply(..)) => {}
                None => return None,
            }
        }
    }
}

/// The per-page overrides, in the order they must be applied: register the init
/// script first, then the emulation overrides it complements.
fn page_identity_commands(plan: &Plan) -> Vec<(String, Value)> {
    // `Page.enable` is required before `addScriptToEvaluateOnNewDocument`:
    // without it the registration is accepted but the script never runs.
    let mut commands: Vec<(String, Value)> =
        vec![("Page.enable".to_string(), json!({}))];

    if let Some(script) = &plan.init_script {
        commands.push((
            "Page.addScriptToEvaluateOnNewDocument".to_string(),
            json!({ "source": script, "runImmediately": true }),
        ));
    }
    if let Some((w, h)) = plan.viewport {
        let mut metrics = json!({ "width": w, "height": h, "deviceScaleFactor": plan.device_scale_factor.unwrap_or(1.0), "mobile": plan.mobile,
                "screenWidth": w, "screenHeight": h });
        if plan.mobile {
            // Phones are portrait and touch-first; without these a page sees a
            // landscape, mouse-driven desktop.
            metrics["screenOrientation"] = json!({ "type": "portraitPrimary", "angle": 0 });
        }
        commands.push(("Emulation.setDeviceMetricsOverride".to_string(), metrics));
        if plan.mobile {
            commands.push((
                "Emulation.setTouchEmulationEnabled".to_string(),
                json!({ "enabled": true, "maxTouchPoints": 5 }),
            ));
        }
    }
    if let Some(ua) = &plan.user_agent {
        let mut params = json!({ "userAgent": ua });
        if let Some(hints) = &plan.client_hints {
            let major = hints.full_version.split('.').next().unwrap_or("");
            params["userAgentMetadata"] = json!({
                "brands": [
                    { "brand": "Chromium", "version": major },
                    { "brand": "Google Chrome", "version": major },
                    { "brand": "Not.A/Brand", "version": "8" }
                ],
                "fullVersion": hints.full_version,
                "platform": hints.platform,
                "platformVersion": hints.platform_version,
                "architecture": hints.architecture,
                "model": hints.model,
                "mobile": hints.mobile,
                "bitness": "64",
                "wow64": false,
                "formFactors": if hints.mobile { vec!["Mobile"] } else { vec!["Desktop"] },
            });
        }
        commands.push(("Emulation.setUserAgentOverride".to_string(), params));
    }
    if let Some(tz) = &plan.timezone {
        commands.push((
            "Emulation.setTimezoneOverride".to_string(),
            json!({ "timezoneId": tz }),
        ));
    }
    if let Some(locale) = &plan.locale {
        commands.push((
            "Emulation.setLocaleOverride".to_string(),
            json!({ "locale": locale }),
        ));
    }

    commands
}

/// Polls `/json/version` until the browser exposes its debugger WebSocket.
async fn wait_for_browser_ws(port: u16) -> AppResult<String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build()?;
    let url = format!("http://127.0.0.1:{port}/json/version");
    for _ in 0..60 {
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(info) = resp.json::<Value>().await {
                if let Some(ws) = info.get("webSocketDebuggerUrl").and_then(Value::as_str) {
                    return Ok(ws.to_string());
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(AppError::validation("Browser did not expose a debuggable endpoint"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_metrics_carry_the_screen_size() {
        let plan = Plan { viewport: Some((412, 915)), mobile: true, ..Default::default() };
        let commands = page_identity_commands(&plan);
        let (_, params) = commands
            .iter()
            .find(|(m, _)| m == "Emulation.setDeviceMetricsOverride")
            .expect("viewport override");
        // screenWidth/screenHeight must match, or new tabs report the host's
        // display size instead of the emulated one.
        assert_eq!(params["width"], 412);
        assert_eq!(params["height"], 915);
        assert_eq!(params["screenWidth"], 412);
        assert_eq!(params["screenHeight"], 915);
        assert_eq!(params["mobile"], true);
        // Portrait orientation and touch input, like a real phone.
        assert_eq!(params["screenOrientation"]["type"], "portraitPrimary");
        let (_, touch) = commands
            .iter()
            .find(|(m, _)| m == "Emulation.setTouchEmulationEnabled")
            .expect("touch emulation for phones");
        assert_eq!(touch["enabled"], true);
        assert_eq!(touch["maxTouchPoints"], 5);
    }

    #[test]
    fn phone_device_scale_factor_comes_from_the_plan() {
        let plan = Plan {
            viewport: Some((412, 915)),
            mobile: true,
            device_scale_factor: Some(2.625),
            ..Default::default()
        };
        let commands = page_identity_commands(&plan);
        let (_, params) = commands
            .iter()
            .find(|(m, _)| m == "Emulation.setDeviceMetricsOverride")
            .unwrap();
        assert_eq!(params["deviceScaleFactor"], 2.625);
    }

    #[test]
    fn client_hints_are_metadatad_from_the_plan() {
        let plan = Plan {
            user_agent: Some("UA".into()),
            client_hints: Some(ClientHints {
                platform: "Android".into(),
                platform_version: "16".into(),
                architecture: "arm".into(),
                model: "Pixel 10".into(),
                mobile: true,
                full_version: "155.0.8059.79".into(),
            }),
            ..Default::default()
        };
        let commands = page_identity_commands(&plan);
        let (_, params) = commands
            .iter()
            .find(|(m, _)| m == "Emulation.setUserAgentOverride")
            .expect("ua override");
        assert_eq!(params["userAgentMetadata"]["platform"], "Android");
        assert_eq!(params["userAgentMetadata"]["mobile"], true);
        assert_eq!(params["userAgentMetadata"]["model"], "Pixel 10");
        assert_eq!(params["userAgentMetadata"]["formFactors"], json!(["Mobile"]));
    }

    #[test]
    fn identity_commands_are_ordered_enable_script_metrics() {
        let plan = Plan {
            init_script: Some("window.__x=1".into()),
            viewport: Some((360, 780)),
            timezone: Some("Europe/Berlin".into()),
            ..Default::default()
        };
        let commands = page_identity_commands(&plan);
        let methods: Vec<&str> = commands.iter().map(|(m, _)| m.as_str()).collect();
        // Page.enable must come first: the script registration is ignored
        // without it.
        assert_eq!(methods.first().copied(), Some("Page.enable"));
        let script = methods.iter().position(|m| *m == "Page.addScriptToEvaluateOnNewDocument");
        let metrics = methods.iter().position(|m| *m == "Emulation.setDeviceMetricsOverride");
        assert!(script.is_some() && metrics.is_some());
        assert!(script < metrics, "the init script is registered before the metrics");
        assert!(methods.contains(&"Emulation.setTimezoneOverride"));
    }
}