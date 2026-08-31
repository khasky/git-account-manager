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
        username_notice: None,
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
        username_notice: None,
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

/// GitHub verifies a signature against a list of its own, separate from the
/// authentication keys in `/user/keys`. A key present only in the second list
/// pushes fine and still leaves every commit Unverified.
const GITHUB_SIGNING_KEYS: &str = "https://api.github.com/user/ssh_signing_keys";

/// Removes a key from one `{id, key}` collection, matching on the key body.
///
/// Shared by the authentication and the signing lists: both are read, matched
/// and deleted the same way, and only the URL differs.
async fn delete_from_key_collection(
    client: &Client,
    collection: &str,
    auth_header: &str,
    github: bool,
    local: &str,
) -> Result<(), String> {
    let mut req = client.get(collection).header("Authorization", auth_header);
    if github {
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
            let delete_url = format!("{}/{}", collection, remote.id);
            let mut del = client
                .delete(&delete_url)
                .header("Authorization", auth_header);
            if github {
                del = del.header("Accept", "application/vnd.github+json");
            }
            let del_resp = del
                .send()
                .await
                .map_err(|e| format!("Failed to delete key: {}", e))?;
            // A key that is already gone is the state the caller wanted.
            if !del_resp.status().is_success() && del_resp.status().as_u16() != 404 {
                return Err(format!("Failed to delete key: HTTP {}", del_resp.status()));
            }
            return Ok(());
        }
    }

    Ok(())
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

    let auth_header = format!("Bearer {}", token);
    let github = platform == Platform::Github;

    delete_from_key_collection(
        client,
        keys_endpoint(platform),
        &auth_header,
        github,
        &local,
    )
    .await?;

    // The signing list is GitHub's alone, and a key can sit in it whether or not
    // this profile ever asked to sign — an account reconnected with the switch
    // off would otherwise leave the key behind after a disconnect that promised
    // to remove it. Absent from the list, this is a no-op.
    if github {
        delete_from_key_collection(client, GITHUB_SIGNING_KEYS, &auth_header, true, &local).await?;
    }

    Ok(())
}

/// Registers a key as one the platform will verify signatures against.
///
/// GitHub keeps that list apart and needs the extra call. GitLab registers new
/// keys as `auth_and_signing` unless told otherwise, and Bitbucket validates
/// against the single key list it has, so for both the key uploaded for
/// authentication already signs and there is nothing left to do.
pub async fn upload_signing_key(
    platform: Platform,
    token: &str,
    title: &str,
    key_content: &str,
) -> Result<(), String> {
    if platform != Platform::Github {
        return Ok(());
    }

    let resp = client()
        .post(GITHUB_SIGNING_KEYS)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .json(&serde_json::json!({ "title": title, "key": key_content }))
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    accept_key_upload(resp, Platform::Github).await
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

/// `/2.0/user/workspaces` answers with membership records, not with the
/// workspaces themselves — the workspace sits one level down, next to the
/// permission the account holds on it.
#[derive(Deserialize)]
struct BitbucketWorkspaceAccessList {
    values: Vec<BitbucketWorkspaceAccess>,
}

#[derive(Deserialize)]
struct BitbucketWorkspaceAccess {
    workspace: BitbucketWorkspace,
}

#[derive(Deserialize)]
struct BitbucketWorkspace {
    uuid: String,
    slug: String,
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

/// The personal workspace is the one carrying the account's own uuid; its slug
/// is what `bitbucket.org/<slug>` resolves to.
fn pick_workspace_slug(memberships: Vec<BitbucketWorkspaceAccess>, uuid: &str) -> Option<String> {
    memberships
        .into_iter()
        .map(|m| m.workspace)
        .find(|w| w.uuid == uuid)
        .map(|w| w.slug)
}

/// Bitbucket's URL-facing name for an account.
///
/// `nickname` is a display name: Atlassian's own schema says it "cannot be used
/// in place of username in URLs and queries, as nickname is not guaranteed to be
/// unique" — an account displaying "Ian Khasky" still lives at
/// `bitbucket.org/khasky`. Using it produced a profile link that 404s and an SSH
/// key filename naming someone else.
///
/// Needs `read:workspace:bitbucket`. An Atlassian token's scopes are fixed when
/// it is created, so a token issued before this was asked for answers 403 here
/// for good — which is why the failure is reported rather than swallowed.
async fn bitbucket_workspace_slug(
    client: &Client,
    token: &str,
    uuid: &str,
) -> Result<String, String> {
    let resp = client
        .get("https://api.bitbucket.org/2.0/user/workspaces?pagelen=100")
        .header("Authorization", basic_auth(token))
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }

    let list: BitbucketWorkspaceAccessList = resp
        .json()
        .await
        .map_err(|e| format!("unreadable response: {}", e))?;

    pick_workspace_slug(list.values, uuid).ok_or_else(|| "no workspace matches the account".into())
}

async fn verify_bitbucket(client: &Client, token: &str) -> Result<PlatformUser, String> {
    let user = bitbucket_get_user(client, token).await?;

    // Falling back to the nickname keeps the account connectable, but the name
    // it produces is wrong everywhere it is later used — the profile link, the
    // generated key filename, the `hasconfig` include pattern — so the reason
    // travels with it instead of being dropped here.
    let (username, notice) = match bitbucket_workspace_slug(client, token, &user.uuid).await {
        Ok(slug) => (slug, None),
        Err(reason) => (
            user.nickname.clone().unwrap_or_else(|| user.uuid.clone()),
            Some(reason),
        ),
    };

    let avatar_url = user.links.and_then(|l| l.avatar).and_then(|a| a.href);
    let email = fetch_bitbucket_primary_email(client, token).await;

    Ok(PlatformUser {
        username,
        name: user.display_name,
        email,
        // Bitbucket has no GitHub/GitLab-style noreply commit email.
        noreply_email: None,
        avatar_url,
        username_notice: notice,
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

    /// Two failures at once, and both are silent — a parse error falls back to
    /// the nickname, which is the wrong name this whole lookup exists to
    /// replace.
    ///
    /// The body is the shape `/2.0/user/workspaces` documents: membership
    /// records with the workspace nested inside, not workspaces at the top
    /// level. And an account belongs to every workspace it was invited to, in no
    /// useful order — only the uuid tells the personal one apart, so picking the
    /// first entry would name a colleague's organisation.
    #[test]
    fn the_personal_workspace_is_read_out_of_the_membership_list() {
        let body = r#"{
          "pagelen": 25, "page": 1, "size": 2,
          "values": [
            {
              "administrator": false,
              "type": "workspace_access",
              "workspace": {
                "type": "workspace_base",
                "uuid": "{team}",
                "slug": "acme-corp",
                "links": {"self": {"href": "https://api.bitbucket.org/2.0/workspaces/acme-corp"}}
              }
            },
            {
              "administrator": true,
              "type": "workspace_access",
              "workspace": {
                "type": "workspace_base",
                "uuid": "{me}",
                "slug": "khasky",
                "links": {"self": {"href": "https://api.bitbucket.org/2.0/workspaces/khasky"}}
              }
            }
          ]
        }"#;

        let list: BitbucketWorkspaceAccessList =
            serde_json::from_str(body).expect("the documented response must parse");
        assert_eq!(
            pick_workspace_slug(list.values, "{me}").as_deref(),
            Some("khasky")
        );
        assert_eq!(pick_workspace_slug(vec![], "{me}"), None);
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
