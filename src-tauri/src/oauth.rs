use crate::models::DeviceCodeResponse;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub async fn github_device_start(client_id: &str) -> Result<DeviceCodeResponse, String> {
    let client = Client::new();
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("scope", "read:user user:email admin:public_key"),
        ])
        .send()
        .await
        .map_err(|e| format!("GitHub request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub device code error: {}", body));
    }

    resp.json::<DeviceCodeResponse>()
        .await
        .map_err(|e| format!("Parse error: {}", e))
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

pub async fn github_device_poll(
    client_id: &str,
    device_code: &str,
) -> Result<Option<String>, String> {
    let client = Client::new();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: GithubTokenResponse = resp.json().await.map_err(|e| e.to_string())?;

    if let Some(token) = body.access_token {
        return Ok(Some(token));
    }

    match body.error.as_deref() {
        Some("authorization_pending") | Some("slow_down") => Ok(None),
        Some(err) => Err(format!("GitHub OAuth error: {}", err)),
        None => Err("Unexpected response from GitHub".to_string()),
    }
}

/// 256 bits from the OS CSPRNG, hex. Used for both the PKCE verifier and the
/// `state` value, which need the same property: unguessable by anything that
/// did not start this flow.
fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    )
}

pub fn generate_pkce() -> (String, String) {
    let verifier = random_token();

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

/// The `state` the callback must echo back.
///
/// PKCE proves that whoever redeems the code also started the flow; it says
/// nothing about *which* request the code answers. The callback listener
/// accepts a plain HTTP request on a known localhost port, so any page the user
/// has open can send one carrying an attacker's `code` and get this app to
/// store an attacker's token under the user's profile. Binding the response to
/// a value only this process knows is what rules that out.
pub fn generate_state() -> String {
    random_token()
}

pub fn build_gitlab_auth_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> String {
    format!(
        "https://gitlab.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=api&state={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(state),
        urlencoding::encode(challenge),
    )
}

pub const GITLAB_CALLBACK_PORT: u16 = 19847;
const GITLAB_CALLBACK_TIMEOUT_SECS: u64 = 120;
const CALLBACK_POLL_INTERVAL_MS: u64 = 250;

/// Binds the callback port on both loopback addresses.
///
/// The redirect URI names `localhost`, and which address a browser picks for it
/// is not knowable in advance — Windows and macOS commonly try `::1` first. A
/// single IPv4 listener silently never receives the callback there. One
/// successful bind is enough; a machine with IPv6 disabled just fails the
/// second, and the port being taken fails both.
pub fn bind_callback_listeners(port: u16) -> Result<Vec<TcpListener>, String> {
    let mut listeners = Vec::new();
    let mut first_error = None;

    for addr in [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ] {
        match TcpListener::bind(addr) {
            Ok(listener) => listeners.push(listener),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }

    if listeners.is_empty() {
        return Err(format!(
            "Cannot bind to port {} (is the app already running?): {}",
            port,
            first_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "no loopback address available".to_string())
        ));
    }
    Ok(listeners)
}

/// The in-flight GitLab flow's cancel flag.
///
/// Only one flow can be in flight — the callback port binds once — so a single
/// slot is enough. It exists so `abort` can reach the blocking accept loop from
/// a different command while that loop is parked on its own thread.
fn cancel_slot() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub fn register_cancel(flag: Arc<AtomicBool>) {
    if let Ok(mut slot) = cancel_slot().lock() {
        *slot = Some(flag);
    }
}

pub fn clear_cancel_slot() {
    if let Ok(mut slot) = cancel_slot().lock() {
        *slot = None;
    }
}

/// Asks the in-flight flow to stop waiting. Does nothing when none is running.
pub fn abort() {
    if let Ok(slot) = cancel_slot().lock() {
        if let Some(flag) = slot.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

pub fn wait_for_callback(
    listeners: Vec<TcpListener>,
    cancel: Arc<AtomicBool>,
    expected_state: &str,
) -> Result<String, String> {
    for listener in &listeners {
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    }
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(GITLAB_CALLBACK_TIMEOUT_SECS);

    let mut stream = loop {
        let mut accepted = None;
        for listener in &listeners {
            match listener.accept() {
                Ok((s, _)) => {
                    accepted = Some(s);
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("Waiting for callback: {}", e)),
            }
        }
        if let Some(s) = accepted {
            break s;
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("Authorization cancelled.".to_string());
        }
        if std::time::Instant::now() > deadline {
            return Err("Authorization timed out. Please try again.".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(CALLBACK_POLL_INTERVAL_MS));
    };

    stream.set_nonblocking(false).ok();
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    // Parsed before the page is written: a request that fails the state check
    // must not be told the authorization succeeded.
    let outcome = parse_callback(&request, expected_state);
    let body = match &outcome {
        Ok(_) => concat!(
            "<div style='text-align:center'><h2>Authorization Successful</h2>",
            "<p>You can close this tab and return to the app.</p></div>"
        ),
        Err(_) => concat!(
            "<div style='text-align:center'><h2>Authorization Rejected</h2>",
            "<p>This response did not come from the request the app started.</p></div>"
        ),
    };
    let html = format!(
        "<html><body style='font-family:system-ui;display:flex;justify-content:center;\
         align-items:center;height:100vh;margin:0;background:#0f172a;color:#e2e8f0'>{}</body></html>",
        body
    );
    let status = if outcome.is_ok() {
        "200 OK"
    } else {
        "400 Bad Request"
    };
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        html.len(),
        html
    );
    stream.write_all(response.as_bytes()).ok();

    outcome
}

fn query_param(path: &str, wanted: &str) -> Option<String> {
    let query_start = path.find('?')?;
    for param in path[query_start + 1..].split('&') {
        let mut parts = param.splitn(2, '=');
        if parts.next() == Some(wanted) {
            let raw = parts.next()?;
            return urlencoding::decode(raw).ok().map(|s| s.to_string());
        }
    }
    None
}

/// Reads the authorization code out of the callback request, but only if the
/// request echoes the `state` this flow sent. A missing `state` fails the same
/// way a wrong one does: the listener is a plain HTTP server on a fixed
/// localhost port, so "no state at all" is exactly what a forged request looks
/// like.
fn parse_callback(request: &str, expected_state: &str) -> Result<String, String> {
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    let state = query_param(path, "state").unwrap_or_default();
    if state.is_empty() || state != expected_state {
        return Err("Authorization response did not match this request.".to_string());
    }

    query_param(path, "code").ok_or_else(|| "No authorization code found in callback".to_string())
}

pub async fn gitlab_exchange_code(
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let client = Client::new();
    let resp = client
        .post("https://gitlab.com/oauth/token")
        .form(&[
            ("client_id", client_id),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitLab token exchange failed: {}", body));
    }

    let tr: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(tr.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(query: &str) -> String {
        format!(
            "GET /callback?{} HTTP/1.1\r\nHost: localhost\r\n\r\n",
            query
        )
    }

    /// The happy path, and the only one that may return a code.
    #[test]
    fn accepts_a_callback_that_echoes_the_state() {
        let code = parse_callback(&get("code=abc123&state=s3cret"), "s3cret").unwrap();
        assert_eq!(code, "abc123");
    }

    /// Without this check any local page could POST an attacker's code to the
    /// callback port and have the app store an attacker's token.
    #[test]
    fn rejects_a_callback_carrying_someone_elses_state() {
        assert!(parse_callback(&get("code=abc123&state=attacker"), "s3cret").is_err());
    }

    /// A forged request is likelier to omit `state` than to guess it, so the
    /// absent case must fail too rather than fall through to the code.
    #[test]
    fn rejects_a_callback_with_no_state_at_all() {
        assert!(parse_callback(&get("code=abc123"), "s3cret").is_err());
        assert!(parse_callback(&get("code=abc123&state="), "s3cret").is_err());
    }

    /// The state check must not accidentally pass for a request that carries no
    /// code either, and the error must not be the "matched but empty" kind.
    #[test]
    fn rejects_a_matching_state_with_no_code() {
        assert!(parse_callback(&get("state=s3cret"), "s3cret").is_err());
    }

    /// GitLab percent-encodes both values; comparing the raw forms would fail a
    /// legitimate callback.
    #[test]
    fn decodes_both_values_before_comparing() {
        let code = parse_callback(&get("code=a%2Fb%2Bc&state=x%20y"), "x y").unwrap();
        assert_eq!(code, "a/b+c");
    }

    #[test]
    fn the_auth_url_carries_the_state_the_callback_is_checked_against() {
        let url = build_gitlab_auth_url("cid", "http://localhost:19847/callback", "chal", "s3cret");
        assert!(url.contains("&state=s3cret"), "{}", url);
        assert!(url.contains("code_challenge=chal"), "{}", url);
        assert!(url.contains("code_challenge_method=S256"), "{}", url);
    }

    /// Two flows must never be able to accept each other's callback.
    #[test]
    fn every_flow_gets_a_distinct_unguessable_state() {
        let a = generate_state();
        let b = generate_state();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }
}
