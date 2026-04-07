use crate::models::PlatformUser;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct GithubUser {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
}

#[derive(Deserialize)]
struct GitlabUser {
    id: u64,
    username: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
    commit_email: Option<String>,
}

pub async fn verify_token(platform: &str, token: &str) -> Result<PlatformUser, String> {
    let client = Client::new();
    match platform {
        "github" => verify_github(&client, token).await,
        "gitlab" => verify_gitlab(&client, token).await,
        _ => Err(format!("Unknown platform: {}", platform)),
    }
}

async fn verify_github(client: &Client, token: &str) -> Result<PlatformUser, String> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "git-account-manager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status()));
    }

    let user: GithubUser = resp.json().await.map_err(|e| e.to_string())?;

    let noreply = format!("{}+{}@users.noreply.github.com", user.id, user.login);

    let email = match user.email {
        Some(e) if !e.is_empty() => Some(e),
        _ => fetch_github_primary_email(client, token).await,
    };

    Ok(PlatformUser {
        username: user.login,
        name: user.name,
        email,
        noreply_email: Some(noreply),
        avatar_url: user.avatar_url,
    })
}

async fn fetch_github_primary_email(client: &Client, token: &str) -> Option<String> {
    let resp = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "git-account-manager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    let emails: Vec<GithubEmail> = resp.json().await.ok()?;
    emails
        .iter()
        .find(|e| e.primary)
        .or(emails.first())
        .map(|e| e.email.clone())
}

async fn verify_gitlab(client: &Client, token: &str) -> Result<PlatformUser, String> {
    let resp = client
        .get("https://gitlab.com/api/v4/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "git-account-manager")
        .send()
        .await
        .map_err(|e| format!("GitLab API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitLab API error: {}", resp.status()));
    }

    let user: GitlabUser = resp.json().await.map_err(|e| e.to_string())?;

    let noreply = user
        .commit_email
        .filter(|e| !e.is_empty() && e.contains("noreply"))
        .or_else(|| Some(format!("{}-{}@users.noreply.gitlab.com", user.id, user.username)));

    Ok(PlatformUser {
        username: user.username.clone(),
        name: user.name,
        email: user.email,
        noreply_email: noreply,
        avatar_url: user.avatar_url,
    })
}

// --- Delete SSH key from platform by matching public key content ---

#[derive(Deserialize)]
struct RemoteKey {
    id: u64,
    key: String,
}

fn normalize_key(key: &str) -> String {
    let parts: Vec<&str> = key.trim().split_whitespace().collect();
    if parts.len() >= 2 { format!("{} {}", parts[0], parts[1]) } else { key.trim().to_string() }
}

pub async fn delete_ssh_key_from_platform(
    platform: &str,
    token: &str,
    pub_key_content: &str,
) -> Result<(), String> {
    let client = Client::new();
    let local = normalize_key(pub_key_content);

    let (url, auth_header) = match platform {
        "github" => ("https://api.github.com/user/keys", format!("Bearer {}", token)),
        "gitlab" => ("https://gitlab.com/api/v4/user/keys", format!("Bearer {}", token)),
        _ => return Err(format!("Unknown platform: {}", platform)),
    };

    let mut req = client.get(url).header("Authorization", &auth_header).header("User-Agent", "git-account-manager");
    if platform == "github" {
        req = req.header("Accept", "application/vnd.github+json");
    }

    let resp = req.send().await.map_err(|e| format!("Failed to list keys: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Failed to list keys: HTTP {}", resp.status()));
    }

    let keys: Vec<RemoteKey> = resp.json().await.map_err(|e| format!("Failed to parse keys: {}", e))?;

    for remote in &keys {
        if normalize_key(&remote.key) == local {
            let delete_url = match platform {
                "github" => format!("https://api.github.com/user/keys/{}", remote.id),
                "gitlab" => format!("https://gitlab.com/api/v4/user/keys/{}", remote.id),
                _ => continue,
            };
            let mut del = client.delete(&delete_url).header("Authorization", &auth_header).header("User-Agent", "git-account-manager");
            if platform == "github" {
                del = del.header("Accept", "application/vnd.github+json");
            }
            let del_resp = del.send().await.map_err(|e| format!("Failed to delete key: {}", e))?;
            if !del_resp.status().is_success() && del_resp.status().as_u16() != 404 {
                return Err(format!("Failed to delete key from {}: HTTP {}", platform, del_resp.status()));
            }
            return Ok(());
        }
    }

    Ok(())
}

pub async fn upload_ssh_key(
    platform: &str,
    token: &str,
    title: &str,
    key_content: &str,
) -> Result<(), String> {
    let client = Client::new();
    match platform {
        "github" => upload_github_key(&client, token, title, key_content).await,
        "gitlab" => upload_gitlab_key(&client, token, title, key_content).await,
        _ => Err(format!("Unknown platform: {}", platform)),
    }
}

async fn upload_github_key(
    client: &Client,
    token: &str,
    title: &str,
    key: &str,
) -> Result<(), String> {
    let resp = client
        .post("https://api.github.com/user/keys")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "git-account-manager")
        .header("Accept", "application/vnd.github+json")
        .json(&serde_json::json!({ "title": title, "key": key }))
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if resp.status().as_u16() == 422 {
        return Ok(());
    }

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error: {}", body));
    }

    Ok(())
}

async fn upload_gitlab_key(
    client: &Client,
    token: &str,
    title: &str,
    key: &str,
) -> Result<(), String> {
    let resp = client
        .post("https://gitlab.com/api/v4/user/keys")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "git-account-manager")
        .json(&serde_json::json!({ "title": title, "key": key }))
        .send()
        .await
        .map_err(|e| format!("GitLab API request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 400 || status == 409 {
        return Ok(());
    }

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitLab API error: {}", body));
    }

    Ok(())
}
