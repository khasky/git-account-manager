use crate::http::client;
use crate::models::{Platform, PlatformUser};
use reqwest::{Client, Response};
use serde::Deserialize;

/// Statuses the three platforms answer with when a key is already registered.
/// None of them gives that case a code of its own, so the body has to decide.
const AMBIGUOUS_UPLOAD_STATUSES: [u16; 3] = [400, 409, 422];

/// Whether a rejected upload was rejected because the account already carries
/// the key.
///
/// Matched on wording because no platform distinguishes it by status: GitHub
/// answers 422 both to a duplicate and to a malformed key, GitLab answers 400 to
/// both. Treating the whole status as success — which is what this code used to
/// do — reported a key that was never registered as uploaded, leaving a profile
/// whose pushes fail and nothing on screen to explain why.
///
/// The match is deliberately narrow: "already" appears in all three duplicate
/// messages ("key is already in use", "has already been taken", "already have
/// this key") and in none of their validation messages. If a platform rewords
/// it, a duplicate starts being reported as a failure — visible and harmless —
/// rather than the reverse.
fn is_duplicate_key(body: &str) -> bool {
    body.to_ascii_lowercase().contains("already")
}

/// Turns an upload response into the answer the caller actually needs: is the
/// key on the account now?
async fn accept_key_upload(resp: Response, platform: Platform) -> Result<(), String> {
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if AMBIGUOUS_UPLOAD_STATUSES.contains(&status.as_u16()) && is_duplicate_key(&body) {
        return Ok(());
    }
    Err(format!(
        "{} API error ({}): {}",
        platform.label(),
        status,
        body.trim()
    ))
}

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

pub async fn verify_token(platform: Platform, token: &str) -> Result<PlatformUser, String> {
    let client = client();
    match platform {
        Platform::Github => verify_github(client, token).await,
        Platform::Gitlab => verify_gitlab(client, token).await,
        Platform::Bitbucket => verify_bitbucket(client, token).await,
    }
}

async fn verify_github(client: &Client, token: &str) -> Result<PlatformUser, String> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
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
        .or_else(|| {
            Some(format!(
                "{}-{}@users.noreply.gitlab.com",
                user.id, user.username
            ))
        });

    Ok(PlatformUser {
        username: user.username.clone(),
        name: user.name,
        email: user.email,
        noreply_email: noreply,
        avatar_url: user.avatar_url,
    })
}

#[derive(Deserialize)]
struct RemoteKey {
    id: u64,
    key: String,
}

/// Type plus base64 body, dropping the trailing comment — the platforms rewrite
/// or strip that, so it is the only part that compares reliably.
fn normalize_key(key: &str) -> String {
    let parts: Vec<&str> = key.split_whitespace().collect();
    if parts.len() >= 2 {
        format!("{} {}", parts[0], parts[1])
    } else {
        key.trim().to_string()
    }
}

/// Where a platform's user keys live, and the id shape they carry.
fn keys_endpoint(platform: Platform) -> &'static str {
    match platform {
        Platform::Github => "https://api.github.com/user/keys",
        Platform::Gitlab => "https://gitlab.com/api/v4/user/keys",
        // Bitbucket scopes keys under the account's uuid, which has to be
        // fetched first; `delete_bitbucket_key` builds the URL itself.
        Platform::Bitbucket => "",
    }
}

pub async fn delete_ssh_key_from_platform(
    platform: Platform,
    token: &str,
    pub_key_content: &str,
) -> Result<(), String> {
    let client = client();
    let local = normalize_key(pub_key_content);

    if platform == Platform::Bitbucket {
        return delete_bitbucket_key(client, token, &local).await;
    }

    let url = keys_endpoint(platform);
    let auth_header = format!("Bearer {}", token);

    let mut req = client.get(url).header("Authorization", &auth_header);
    if platform == Platform::Github {
        req = req.header("Accept", "application/vnd.github+json");
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Failed to list keys: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Failed to list keys: HTTP {}", resp.status()));
    }

    let keys: Vec<RemoteKey> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse keys: {}", e))?;

    for remote in &keys {
        if normalize_key(&remote.key) == local {
            let delete_url = format!("{}/{}", url, remote.id);
            let mut del = client
                .delete(&delete_url)
                .header("Authorization", &auth_header);
            if platform == Platform::Github {
                del = del.header("Accept", "application/vnd.github+json");
            }
            let del_resp = del
                .send()
                .await
                .map_err(|e| format!("Failed to delete key: {}", e))?;
            // A key that is already gone is the state the caller wanted.
            if !del_resp.status().is_success() && del_resp.status().as_u16() != 404 {
                return Err(format!(
                    "Failed to delete key from {}: HTTP {}",
                    platform.label(),
                    del_resp.status()
                ));
            }
            return Ok(());
        }
    }

    Ok(())
}

pub async fn upload_ssh_key(
    platform: Platform,
    token: &str,
    title: &str,
    key_content: &str,
) -> Result<(), String> {
    let client = client();
    match platform {
        Platform::Github => upload_github_key(client, token, title, key_content).await,
        Platform::Gitlab => upload_gitlab_key(client, token, title, key_content).await,
        Platform::Bitbucket => upload_bitbucket_key(client, token, title, key_content).await,
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
        .header("Accept", "application/vnd.github+json")
        .json(&serde_json::json!({ "title": title, "key": key }))
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    accept_key_upload(resp, Platform::Github).await
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
        .json(&serde_json::json!({ "title": title, "key": key }))
        .send()
        .await
        .map_err(|e| format!("GitLab API request failed: {}", e))?;

    accept_key_upload(resp, Platform::Gitlab).await
}

/// Bitbucket takes an Atlassian API token over HTTP Basic, not a Bearer token
/// like the other two, and the token is already the `email:token` pair.
fn basic_auth(token: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    format!("Basic {}", STANDARD.encode(token))
}

#[derive(Deserialize)]
struct BitbucketUser {
    uuid: String,
    nickname: Option<String>,
    display_name: Option<String>,
    links: Option<BitbucketLinks>,
}

#[derive(Deserialize)]
struct BitbucketLinks {
    avatar: Option<BitbucketLink>,
}

#[derive(Deserialize)]
struct BitbucketLink {
    href: Option<String>,
}

#[derive(Deserialize)]
struct BitbucketEmails {
    values: Vec<BitbucketEmail>,
}

#[derive(Deserialize)]
struct BitbucketEmail {
    email: String,
    is_primary: bool,
}

#[derive(Deserialize)]
struct BitbucketKeyList {
    values: Vec<BitbucketKey>,
}

#[derive(Deserialize)]
struct BitbucketKey {
    uuid: String,
    key: String,
}

async fn bitbucket_get_user(client: &Client, token: &str) -> Result<BitbucketUser, String> {
    let resp = client
        .get("https://api.bitbucket.org/2.0/user")
        .header("Authorization", basic_auth(token))
        .send()
        .await
        .map_err(|e| format!("Bitbucket API request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Bitbucket API error: {}", resp.status()));
    }

    resp.json::<BitbucketUser>()
        .await
        .map_err(|e| e.to_string())
}

async fn verify_bitbucket(client: &Client, token: &str) -> Result<PlatformUser, String> {
    let user = bitbucket_get_user(client, token).await?;
    let username = user.nickname.clone().unwrap_or_else(|| user.uuid.clone());
    let avatar_url = user.links.and_then(|l| l.avatar).and_then(|a| a.href);
    let email = fetch_bitbucket_primary_email(client, token).await;

    Ok(PlatformUser {
        username,
        name: user.display_name,
        email,
        // Bitbucket has no GitHub/GitLab-style noreply commit email.
        noreply_email: None,
        avatar_url,
    })
}

async fn fetch_bitbucket_primary_email(client: &Client, token: &str) -> Option<String> {
    let resp = client
        .get("https://api.bitbucket.org/2.0/user/emails")
        .header("Authorization", basic_auth(token))
        .send()
        .await
        .ok()?;

    let emails: BitbucketEmails = resp.json().await.ok()?;
    emails
        .values
        .iter()
        .find(|e| e.is_primary)
        .or_else(|| emails.values.first())
        .map(|e| e.email.clone())
}

async fn upload_bitbucket_key(
    client: &Client,
    token: &str,
    title: &str,
    key: &str,
) -> Result<(), String> {
    let uuid = bitbucket_get_user(client, token).await?.uuid;
    let url = format!(
        "https://api.bitbucket.org/2.0/users/{}/ssh-keys",
        urlencoding::encode(&uuid)
    );

    let resp = client
        .post(&url)
        .header("Authorization", basic_auth(token))
        .json(&serde_json::json!({ "key": key, "label": title }))
        .send()
        .await
        .map_err(|e| format!("Bitbucket API request failed: {}", e))?;

    accept_key_upload(resp, Platform::Bitbucket).await
}

async fn delete_bitbucket_key(
    client: &Client,
    token: &str,
    local_normalized: &str,
) -> Result<(), String> {
    let uuid = bitbucket_get_user(client, token).await?.uuid;
    let base = format!(
        "https://api.bitbucket.org/2.0/users/{}/ssh-keys",
        urlencoding::encode(&uuid)
    );

    let resp = client
        .get(&base)
        .header("Authorization", basic_auth(token))
        .send()
        .await
        .map_err(|e| format!("Failed to list keys: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Failed to list keys: HTTP {}", resp.status()));
    }

    let list: BitbucketKeyList = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse keys: {}", e))?;

    for remote in &list.values {
        if normalize_key(&remote.key) == local_normalized {
            let delete_url = format!("{}/{}", base, urlencoding::encode(&remote.uuid));
            let del_resp = client
                .delete(&delete_url)
                .header("Authorization", basic_auth(token))
                .header("User-Agent", "git-account-manager")
                .send()
                .await
                .map_err(|e| format!("Failed to delete key: {}", e))?;
            if !del_resp.status().is_success() && del_resp.status().as_u16() != 404 {
                return Err(format!(
                    "Failed to delete key from bitbucket: HTTP {}",
                    del_resp.status()
                ));
            }
            return Ok(());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real bodies the three platforms answer with when the account already
    /// carries the key. Reporting these as failures would make a second Save
    /// look broken.
    #[test]
    fn recognises_a_key_the_account_already_has() {
        for body in [
            r#"{"message":"Validation Failed","errors":[{"resource":"PublicKey","code":"custom","field":"key","message":"key is already in use"}]}"#,
            r#"{"message":{"fingerprint":["has already been taken"]}}"#,
            r#"{"error":{"message":"Someone has already added this SSH key to a Bitbucket account."}}"#,
        ] {
            assert!(is_duplicate_key(body), "must read as a duplicate: {}", body);
        }
    }

    /// The case this used to swallow. GitHub answers a malformed key with the
    /// same 422 it answers a duplicate with, so a status-only check reported
    /// "uploaded" for a key that was never registered — and the profile's pushes
    /// then failed with nothing on screen to connect the two.
    #[test]
    fn a_rejected_key_is_not_a_duplicate() {
        for body in [
            r#"{"message":"Validation Failed","errors":[{"field":"key","message":"key is invalid. It must begin with 'ssh-ed25519'"}]}"#,
            r#"{"message":{"key":["is invalid"]}}"#,
            r#"{"message":"Bad credentials"}"#,
            "",
        ] {
            assert!(
                !is_duplicate_key(body),
                "must not read as a duplicate: {}",
                body
            );
        }
    }

    /// Only the statuses that are genuinely ambiguous may be forgiven; a 500
    /// whose body happens to say "already" is still a failure.
    #[test]
    fn only_the_ambiguous_statuses_can_mean_duplicate() {
        assert_eq!(AMBIGUOUS_UPLOAD_STATUSES, [400, 409, 422]);
        assert!(!AMBIGUOUS_UPLOAD_STATUSES.contains(&500));
        assert!(!AMBIGUOUS_UPLOAD_STATUSES.contains(&401));
        assert!(!AMBIGUOUS_UPLOAD_STATUSES.contains(&403));
    }

    /// The comment body is rewritten by the platforms, so only the type and the
    /// base64 payload can be compared when looking for a key to delete.
    #[test]
    fn key_comparison_ignores_the_trailing_comment() {
        let local = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 me@laptop";
        let remote = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 someone@github";
        assert_eq!(normalize_key(local), normalize_key(remote));
        assert_ne!(
            normalize_key(local),
            normalize_key("ssh-ed25519 DIFFERENTBODY me@laptop")
        );
    }

    /// The delete path builds `<endpoint>/<id>`, so the endpoint has to be the
    /// collection URL for the two platforms that use it.
    #[test]
    fn the_key_endpoints_are_the_collections_the_delete_path_appends_to() {
        assert_eq!(
            keys_endpoint(Platform::Github),
            "https://api.github.com/user/keys"
        );
        assert_eq!(
            keys_endpoint(Platform::Gitlab),
            "https://gitlab.com/api/v4/user/keys"
        );
    }
}
