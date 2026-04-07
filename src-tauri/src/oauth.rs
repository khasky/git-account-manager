use crate::models::DeviceCodeResponse;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// --------------- GitHub Device Flow ---------------

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
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code",
            ),
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

// --------------- GitLab PKCE Flow ---------------

pub fn generate_pkce() -> (String, String) {
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    );

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

pub fn build_gitlab_auth_url(client_id: &str, redirect_uri: &str, challenge: &str) -> String {
    format!(
        "https://gitlab.com/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=api&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(challenge),
    )
}

pub const GITLAB_CALLBACK_PORT: u16 = 19847;
const GITLAB_CALLBACK_TIMEOUT_SECS: u64 = 120;

pub fn wait_for_callback(listener: TcpListener, cancel: Arc<AtomicBool>) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(GITLAB_CALLBACK_TIMEOUT_SECS);

    let mut stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if cancel.load(Ordering::SeqCst) {
                    return Err("Authorization cancelled.".to_string());
                }
                if std::time::Instant::now() > deadline {
                    return Err("Authorization timed out. Please try again.".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            Err(e) => return Err(format!("Waiting for callback: {}", e)),
        }
    };

    stream.set_nonblocking(false).ok();
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    let html = concat!(
        "<html><body style='font-family:system-ui;display:flex;justify-content:center;",
        "align-items:center;height:100vh;margin:0;background:#0f172a;color:#e2e8f0'>",
        "<div style='text-align:center'><h2>Authorization Successful</h2>",
        "<p>You can close this tab and return to the app.</p></div></body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    stream.write_all(response.as_bytes()).ok();

    parse_code(&request)
}

fn parse_code(request: &str) -> Result<String, String> {
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    if let Some(query_start) = path.find('?') {
        for param in path[query_start + 1..].split('&') {
            let mut parts = param.splitn(2, '=');
            if parts.next() == Some("code") {
                if let Some(code) = parts.next() {
                    return urlencoding::decode(code)
                        .map(|s| s.to_string())
                        .map_err(|e| e.to_string());
                }
            }
        }
    }

    Err("No authorization code found in callback".to_string())
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
