//! The `git credential` protocol, the half of it a read-only helper needs.
//!
//! Git hands a helper one request on stdin as `key=value` lines ended by a
//! blank line, and reads the answer back in the same shape. This one answers
//! `get` and ignores `store` and `erase`, the model `gh auth git-credential`
//! uses: the credential belongs to this app and its credential store, so there
//! is nothing for git to teach us and nothing of ours for it to forget.
//!
//! Silence is a valid answer. Where no profile, no account or no stored
//! credential fits the host git asked about, the helper prints nothing and git
//! falls through to whatever it would have done without us.

use crate::models::{AppState, Platform, Profile, PLATFORMS};
use crate::{git, repos, secrets};
use std::path::PathBuf;

/// What git asked for. Only the keys that decide the answer are kept.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub protocol: String,
    pub host: String,
    /// The account git already knows it wants, from `user@host` in the remote
    /// URL or from `credential.<url>.username`. Empty when git left the choice
    /// open.
    pub username: String,
}

/// The two credential-store slots an account can answer from.
///
/// A trait rather than a direct call so the rules below are testable without a
/// credential store: on a CI runner there is none to write to.
pub trait TokenSource {
    fn https_token(&self, profile_id: &str, platform: Platform) -> Option<String>;
    fn oauth_token(&self, profile_id: &str, platform: Platform) -> Option<String>;
}

pub struct Keychain;

impl TokenSource for Keychain {
    fn https_token(&self, profile_id: &str, platform: Platform) -> Option<String> {
        secrets::get_https_token(profile_id, platform)
            .ok()
            .flatten()
    }

    fn oauth_token(&self, profile_id: &str, platform: Platform) -> Option<String> {
        secrets::get_token(profile_id, platform).ok()
    }
}

/// Reads one request off the text git wrote to stdin.
///
/// Unknown keys are skipped rather than refused: git adds attributes over time
/// (`wwwauth[]`, `capability[]`), and a helper that fails on one it does not
/// know would break on a git newer than itself.
pub fn parse_request(input: &str) -> Request {
    let mut request = Request::default();

    for line in input.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "protocol" => request.protocol = value.to_string(),
            "host" => request.host = value.to_string(),
            "username" => request.username = value.to_string(),
            // Git sends the whole URL instead of its parts when the caller used
            // `git credential fill` with a `url=` line.
            "url" => {
                if let Some((protocol, rest)) = value.split_once("://") {
                    request.protocol = protocol.to_string();
                    let authority = rest.split('/').next().unwrap_or(rest);
                    let (userinfo, host) = match authority.rsplit_once('@') {
                        Some((userinfo, host)) => (Some(userinfo), host),
                        None => (None, authority),
                    };
                    request.host = host.to_string();
                    if let Some(userinfo) = userinfo {
                        request.username = userinfo
                            .split_once(':')
                            .map_or(userinfo, |(u, _)| u)
                            .to_string();
                    }
                }
            }
            _ => {}
        }
    }

    request
}

/// What the helper prints for a request, or `None` when it has nothing to say.
pub fn answer(request: &Request, state: &AppState, tokens: &impl TokenSource) -> Option<String> {
    if !request.protocol.eq_ignore_ascii_case("https") {
        return None;
    }

    let platform = repos::platform_for_host(&request.host, &state.profiles)?;
    let profile = pick_profile(state, platform, &request.username)?;
    let (username, password) = credentials(profile, platform, tokens)?;

    // A value carrying a newline would read as the start of another attribute
    // on git's side, so a corrupted store cannot dictate the rest of the answer.
    if [&username, &password]
        .iter()
        .any(|v| v.contains(['\n', '\r']))
    {
        return None;
    }

    Some(format!("username={}\npassword={}\n\n", username, password))
}

/// The profile whose token answers for this host.
///
/// With no username in the request the active profile answers, which is what
/// makes an identity switch reach HTTPS remotes the way it already reaches SSH
/// ones. A username git did name has to match an account: handing it the active
/// profile's token instead would authenticate as somebody else.
fn pick_profile<'a>(
    state: &'a AppState,
    platform: Platform,
    username: &str,
) -> Option<&'a Profile> {
    if username.is_empty() {
        return state
            .profiles
            .iter()
            .find(|p| p.is_active && p.account(platform).is_some());
    }

    state.profiles.iter().find(|p| {
        p.account(platform)
            .is_some_and(|a| a.username.eq_ignore_ascii_case(username))
    })
}

/// The username and password each platform expects over HTTPS.
fn credentials(
    profile: &Profile,
    platform: Platform,
    tokens: &impl TokenSource,
) -> Option<(String, String)> {
    let account = profile.account(platform)?;
    let https_token = tokens
        .https_token(&profile.id, platform)
        .filter(|t| !t.trim().is_empty());

    match platform {
        // GitHub ignores the username and reads the scopes off the token. The
        // OAuth token this app holds carries none that reach a repository, so
        // nothing stands in for a token the user issued themselves.
        Platform::Github => Some((
            or_else_literal(&account.username, "x-access-token"),
            https_token?,
        )),
        // A GitLab personal access token authenticates under any username. The
        // OAuth access token would too, under `oauth2`, but it expires within
        // hours and no refresh token is stored: a password that quietly stops
        // working is worse than the prompt it replaced.
        Platform::Gitlab => Some((or_else_literal(&account.username, "oauth2"), https_token?)),
        // What the connect form stored is already an HTTPS Basic pair
        // (`email:api_token`, `commands::connect_bitbucket`), which is what
        // Bitbucket wants here, so it answers when no separate token was added.
        Platform::Bitbucket => {
            let stored = https_token
                .or_else(|| tokens.oauth_token(&profile.id, platform))
                .filter(|t| !t.trim().is_empty())?;
            Some(match stored.split_once(':') {
                Some((email, token)) => (email.to_string(), token.to_string()),
                None => ("x-bitbucket-api-token-auth".to_string(), stored),
            })
        }
    }
}

fn or_else_literal(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

/// The helper binary, which the installer places beside the app's own.
///
/// Resolved from the running executable rather than from `PATH`: an install
/// that never reached `PATH`, or a second copy of the app somewhere else, would
/// otherwise have git run a helper reading a different machine's idea of which
/// profile is active.
fn helper_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate the running application: {}", e))?;
    let path = exe
        .parent()
        .ok_or_else(|| "The running application has no directory.".to_string())?
        .join(if cfg!(windows) { "gam.exe" } else { "gam" });

    if !path.exists() {
        return Err(format!(
            "Could not find the credential helper at {}. Reinstall Git Account Manager.",
            path.display()
        ));
    }
    Ok(path)
}

/// Refuses the switch while the helper is missing, so it fails where the user
/// turned it on rather than silently at the next `git push`.
pub fn ensure_helper_available() -> Result<(), String> {
    helper_path().map(|_| ())
}

/// Brings git's credential config in line with the state.
///
/// The helper is configured for exactly the hosts the active profile can
/// answer for, and removed from the rest: a host this app has no credential for
/// must keep falling through to whatever answered for it before, since the
/// per-host reset in `git::set_credential_helper` would otherwise leave it with
/// no helper at all.
pub fn apply(state: &AppState) -> Result<(), String> {
    let helper = match state.oauth.use_https_credential_helper {
        true => Some(helper_path()?),
        false => None,
    };

    for platform in PLATFORMS {
        let host = platform.canonical_host();
        match helper.as_ref().filter(|_| can_answer(state, platform)) {
            Some(path) => git::set_credential_helper(host, &path.to_string_lossy())?,
            None => git::unset_credential_helper(host)?,
        }
    }
    Ok(())
}

/// Whether the active profile has something to answer this platform with.
/// Asked of the same code that answers, so the config never claims a host the
/// helper then stays silent on.
fn can_answer(state: &AppState, platform: Platform) -> bool {
    state
        .profiles
        .iter()
        .find(|p| p.is_active)
        .and_then(|profile| credentials(profile, platform, &Keychain))
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlatformAccount;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeTokens {
        https: HashMap<String, String>,
        oauth: HashMap<String, String>,
    }

    impl FakeTokens {
        fn with_https(mut self, profile_id: &str, platform: Platform, token: &str) -> Self {
            self.https
                .insert(format!("{}:{}", profile_id, platform), token.to_string());
            self
        }

        fn with_oauth(mut self, profile_id: &str, platform: Platform, token: &str) -> Self {
            self.oauth
                .insert(format!("{}:{}", profile_id, platform), token.to_string());
            self
        }
    }

    impl TokenSource for FakeTokens {
        fn https_token(&self, profile_id: &str, platform: Platform) -> Option<String> {
            self.https
                .get(&format!("{}:{}", profile_id, platform))
                .cloned()
        }

        fn oauth_token(&self, profile_id: &str, platform: Platform) -> Option<String> {
            self.oauth
                .get(&format!("{}:{}", profile_id, platform))
                .cloned()
        }
    }

    fn account(username: &str) -> PlatformAccount {
        PlatformAccount {
            username: username.to_string(),
            git_name: username.to_string(),
            git_email: format!("{}@example.com", username),
            ssh_private_key_path: String::new(),
            ssh_public_key_path: String::new(),
            sign_commits: false,
            token: None,
        }
    }

    fn profile(id: &str, name: &str, is_active: bool) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            default_platform: None,
            github: None,
            gitlab: None,
            bitbucket: None,
            is_active,
        }
    }

    /// One active profile with a GitHub account, one inactive with another.
    fn state() -> AppState {
        let mut work = profile("p1", "Work", true);
        work.github = Some(account("work-user"));
        let mut personal = profile("p2", "Personal", false);
        personal.github = Some(account("personal-user"));

        AppState {
            profiles: vec![work, personal],
            ..Default::default()
        }
    }

    fn get(input: &str, state: &AppState, tokens: &FakeTokens) -> Option<String> {
        answer(&parse_request(input), state, tokens)
    }

    #[test]
    fn the_active_profile_answers_for_a_host_it_has_an_account_on() {
        let tokens = FakeTokens::default().with_https("p1", Platform::Github, "ghp_work");

        assert_eq!(
            get("protocol=https\nhost=github.com\n\n", &state(), &tokens).as_deref(),
            Some("username=work-user\npassword=ghp_work\n\n")
        );
    }

    #[test]
    fn a_named_username_picks_its_own_profile_over_the_active_one() {
        let tokens = FakeTokens::default()
            .with_https("p1", Platform::Github, "ghp_work")
            .with_https("p2", Platform::Github, "ghp_personal");

        assert_eq!(
            get(
                "protocol=https\nhost=github.com\nusername=personal-user\n\n",
                &state(),
                &tokens
            )
            .as_deref(),
            Some("username=personal-user\npassword=ghp_personal\n\n")
        );
    }

    #[test]
    fn a_username_no_account_carries_is_refused_rather_than_answered_with_another() {
        let tokens = FakeTokens::default().with_https("p1", Platform::Github, "ghp_work");

        assert!(get(
            "protocol=https\nhost=github.com\nusername=someone-else\n\n",
            &state(),
            &tokens
        )
        .is_none());
    }

    #[test]
    fn a_github_account_without_its_own_https_token_stays_silent() {
        let tokens = FakeTokens::default().with_oauth("p1", Platform::Github, "gho_oauth");

        assert!(get("protocol=https\nhost=github.com\n\n", &state(), &tokens).is_none());
    }

    #[test]
    fn an_unknown_host_and_a_plain_http_request_stay_silent() {
        let tokens = FakeTokens::default().with_https("p1", Platform::Github, "ghp_work");

        assert!(get("protocol=https\nhost=example.com\n\n", &state(), &tokens).is_none());
        assert!(get("protocol=http\nhost=github.com\n\n", &state(), &tokens).is_none());
    }

    #[test]
    fn bitbucket_splits_the_basic_pair_the_connect_form_stored() {
        let mut work = profile("p1", "Work", true);
        work.bitbucket = Some(account("work-user"));
        let state = AppState {
            profiles: vec![work],
            ..Default::default()
        };
        let tokens = FakeTokens::default().with_oauth(
            "p1",
            Platform::Bitbucket,
            "dev@example.com:ATATTsecret",
        );

        assert_eq!(
            get("protocol=https\nhost=bitbucket.org\n\n", &state, &tokens).as_deref(),
            Some("username=dev@example.com\npassword=ATATTsecret\n\n")
        );
    }

    #[test]
    fn a_bare_bitbucket_token_authenticates_under_the_api_token_username() {
        let mut work = profile("p1", "Work", true);
        work.bitbucket = Some(account("work-user"));
        let state = AppState {
            profiles: vec![work],
            ..Default::default()
        };
        let tokens = FakeTokens::default().with_https("p1", Platform::Bitbucket, "ATATTsecret");

        assert_eq!(
            get("protocol=https\nhost=bitbucket.org\n\n", &state, &tokens).as_deref(),
            Some("username=x-bitbucket-api-token-auth\npassword=ATATTsecret\n\n")
        );
    }

    #[test]
    fn a_url_line_carries_the_same_request_as_its_parts() {
        assert_eq!(
            parse_request("url=https://personal-user@github.com/acme/app.git\n\n"),
            Request {
                protocol: "https".to_string(),
                host: "github.com".to_string(),
                username: "personal-user".to_string(),
            }
        );
    }

    #[test]
    fn a_stored_value_carrying_a_newline_is_not_answered_with() {
        let tokens =
            FakeTokens::default().with_https("p1", Platform::Github, "ghp_work\nhost=evil.example");

        assert!(get("protocol=https\nhost=github.com\n\n", &state(), &tokens).is_none());
    }
}
