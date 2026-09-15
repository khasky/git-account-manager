use crate::models::{AppState, Platform, PLATFORMS};
use keyring_core::{Entry, Error};
use std::sync::OnceLock;

const SERVICE: &str = "com.khasky.git-account-manager";

pub trait SecretStore {
    fn set_token(&self, profile_id: &str, platform: Platform, token: &str) -> Result<(), String>;
    fn get_token(&self, profile_id: &str, platform: Platform) -> Result<String, String>;
    fn delete_token(&self, profile_id: &str, platform: Platform) -> Result<(), String>;

    fn delete_profile_tokens(&self, profile_id: &str) -> Result<(), String> {
        for platform in PLATFORMS {
            self.delete_token(profile_id, platform)?;
        }
        Ok(())
    }
}

pub struct OsSecretStore;

impl SecretStore for OsSecretStore {
    fn set_token(&self, profile_id: &str, platform: Platform, token: &str) -> Result<(), String> {
        ensure_store()?;
        entry(profile_id, platform)?
            .set_password(token)
            .map_err(|e| store_error("save", e))
    }

    fn get_token(&self, profile_id: &str, platform: Platform) -> Result<String, String> {
        ensure_store()?;
        entry(profile_id, platform)?
            .get_password()
            .map_err(|e| match e {
                Error::NoEntry => format!(
                    "No stored token for {}. Reconnect the account and try again.",
                    platform.label()
                ),
                other => store_error("read", other),
            })
    }

    fn delete_token(&self, profile_id: &str, platform: Platform) -> Result<(), String> {
        ensure_store()?;
        match entry(profile_id, platform)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(store_error("delete", e)),
        }
    }
}

pub fn set_token(profile_id: &str, platform: Platform, token: &str) -> Result<(), String> {
    OsSecretStore.set_token(profile_id, platform, token)
}

pub fn get_token(profile_id: &str, platform: Platform) -> Result<String, String> {
    OsSecretStore.get_token(profile_id, platform)
}

/// Drops everything this account authenticates with, the HTTPS credential
/// included: a platform disconnected from a profile must not leave a usable
/// password behind in the credential store.
pub fn delete_token(profile_id: &str, platform: Platform) -> Result<(), String> {
    OsSecretStore.delete_token(profile_id, platform)?;
    delete_https_token(profile_id, platform)
}

pub fn delete_profile_tokens(profile_id: &str) -> Result<(), String> {
    OsSecretStore.delete_profile_tokens(profile_id)?;
    for platform in PLATFORMS {
        delete_https_token(profile_id, platform)?;
    }
    Ok(())
}

/// The password git is handed for an HTTPS remote, kept in its own slot rather
/// than in the OAuth one.
///
/// The two are not interchangeable: the scopes this app asks for over OAuth do
/// not reach a private repository over HTTPS (`read:user user:email
/// admin:public_key write:ssh_signing_key` on GitHub), so the credential git
/// needs is one the user issues themselves and pastes in.
pub fn set_https_token(profile_id: &str, platform: Platform, token: &str) -> Result<(), String> {
    ensure_store()?;
    https_entry(profile_id, platform)?
        .set_password(token)
        .map_err(|e| store_error("save", e))
}

/// `None` where the account has no HTTPS credential, which is the normal state
/// of every account until someone adds one.
pub fn get_https_token(profile_id: &str, platform: Platform) -> Result<Option<String>, String> {
    ensure_store()?;
    match https_entry(profile_id, platform)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(store_error("read", e)),
    }
}

pub fn delete_https_token(profile_id: &str, platform: Platform) -> Result<(), String> {
    ensure_store()?;
    match https_entry(profile_id, platform)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(store_error("delete", e)),
    }
}

pub fn migrate_plaintext_tokens(state: &mut AppState) -> Result<bool, String> {
    migrate_plaintext_tokens_with_store(state, &OsSecretStore)
}

pub fn migrate_plaintext_tokens_with_store<S: SecretStore>(
    state: &mut AppState,
    store: &S,
) -> Result<bool, String> {
    let mut pending: Vec<(String, Platform, String)> = Vec::new();
    let mut found_legacy_tokens = false;

    for profile in &state.profiles {
        for platform in PLATFORMS {
            let Some(token) = profile.account(platform).and_then(|a| a.token.as_ref()) else {
                continue;
            };
            found_legacy_tokens = true;
            // An empty legacy field still has to be cleared from the JSON, but
            // storing it would shadow a real token in the OS store.
            if !token.trim().is_empty() {
                pending.push((profile.id.clone(), platform, token.clone()));
            }
        }
    }

    if !found_legacy_tokens {
        return Ok(false);
    }

    for (profile_id, platform, token) in &pending {
        store.set_token(profile_id, *platform, token)?;
    }

    for profile in &mut state.profiles {
        for platform in PLATFORMS {
            if let Some(account) = profile.account_mut(platform) {
                account.token = None;
            }
        }
    }

    Ok(true)
}

fn ensure_store() -> Result<(), String> {
    static READY: OnceLock<()> = OnceLock::new();

    if READY.get().is_some() {
        return Ok(());
    }

    init_platform_store()?;
    let _ = READY.set(());
    Ok(())
}

#[cfg(windows)]
fn init_platform_store() -> Result<(), String> {
    let store = windows_native_keyring_store::Store::new().map_err(|e| store_error("open", e))?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "macos")]
fn init_platform_store() -> Result<(), String> {
    let store =
        apple_native_keyring_store::keychain::Store::new().map_err(|e| store_error("open", e))?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "linux")]
fn init_platform_store() -> Result<(), String> {
    let store =
        zbus_secret_service_keyring_store::Store::new().map_err(|e| store_error("open", e))?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn init_platform_store() -> Result<(), String> {
    Err("OS credential store is not supported on this platform.".to_string())
}

fn entry(profile_id: &str, platform: Platform) -> Result<Entry, String> {
    Entry::new(SERVICE, &account_name(profile_id, platform)).map_err(|e| store_error("open", e))
}

/// The key a token is stored under. Built from `Platform::as_str`, so it is the
/// same string every existing install already has in its credential store.
fn account_name(profile_id: &str, platform: Platform) -> String {
    format!("token:{}:{}", profile_id, platform.as_str())
}

fn https_entry(profile_id: &str, platform: Platform) -> Result<Entry, String> {
    Entry::new(SERVICE, &https_account_name(profile_id, platform))
        .map_err(|e| store_error("open", e))
}

/// Its own prefix, so adding an HTTPS credential never overwrites the OAuth
/// token an install already holds under `token:`.
fn https_account_name(profile_id: &str, platform: Platform) -> String {
    format!("https:{}:{}", profile_id, platform.as_str())
}

fn store_error(action: &str, err: Error) -> String {
    format!(
        "Could not {} token in the OS credential store: {}",
        action, err
    )
}

/// Issuer prefixes that name a secret whatever its length.
const TOKEN_PREFIXES: [&str; 8] = [
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "ATATT",
];

/// Length at which an unbroken run of token characters is treated as a secret
/// on its own. A GitLab OAuth access token is 64 hex characters and carries no
/// prefix to recognise it by.
const UNPREFIXED_SECRET_LEN: usize = 32;

const REDACTED: &str = "[redacted]";

/// Cuts token-shaped runs out of a message before anything prints it.
///
/// The credential helper is the first place a stored token travels through a
/// process this app does not own, and its diagnostics land in a terminal, a CI
/// log or a pasted bug report. Known ceiling: a secret shorter than
/// `UNPREFIXED_SECRET_LEN` that carries no known prefix survives, and a long
/// identifier that is not a secret (a commit SHA) is masked along with them.
pub fn redact(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut run = String::new();

    for ch in message.chars() {
        if is_token_char(ch) {
            run.push(ch);
        } else {
            flush_run(&mut run, &mut out);
            out.push(ch);
        }
    }
    flush_run(&mut run, &mut out);

    out
}

/// Path separators, dots, colons and `=` are left out: a file path breaks into
/// short runs instead of reading as one long secret, and the `key=value` line
/// the credential protocol speaks in breaks at the `=`, so the value is weighed
/// on its own rather than hidden behind the length of its key.
fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '+')
}

fn flush_run(run: &mut String, out: &mut String) {
    if run.len() >= UNPREFIXED_SECRET_LEN || TOKEN_PREFIXES.iter().any(|p| run.starts_with(p)) {
        out.push_str(REDACTED);
    } else {
        out.push_str(run);
    }
    run.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PlatformAccount, Profile};
    use std::cell::RefCell;

    #[derive(Default)]
    struct MockStore {
        writes: RefCell<Vec<(String, String, String)>>,
        fail_after_writes: Option<usize>,
    }

    impl SecretStore for MockStore {
        fn set_token(
            &self,
            profile_id: &str,
            platform: Platform,
            token: &str,
        ) -> Result<(), String> {
            if self
                .fail_after_writes
                .is_some_and(|limit| self.writes.borrow().len() >= limit)
            {
                return Err("mock keychain unavailable".to_string());
            }
            self.writes.borrow_mut().push((
                profile_id.to_string(),
                platform.to_string(),
                token.to_string(),
            ));
            Ok(())
        }

        fn get_token(&self, _profile_id: &str, _platform: Platform) -> Result<String, String> {
            unimplemented!()
        }

        fn delete_token(&self, _profile_id: &str, _platform: Platform) -> Result<(), String> {
            Ok(())
        }
    }

    fn legacy_state() -> AppState {
        AppState {
            profiles: vec![Profile {
                id: "profile-1".to_string(),
                name: "Work".to_string(),
                default_platform: Some(Platform::Github),
                github: Some(PlatformAccount {
                    username: "octo".to_string(),
                    git_name: "Octo".to_string(),
                    git_email: "octo@example.com".to_string(),
                    ssh_private_key_path: "~/.ssh/id".to_string(),
                    ssh_public_key_path: "~/.ssh/id.pub".to_string(),
                    sign_commits: false,
                    token: Some("gh-token".to_string()),
                }),
                gitlab: None,
                bitbucket: Some(PlatformAccount {
                    username: "bb".to_string(),
                    git_name: "BB".to_string(),
                    git_email: "bb@example.com".to_string(),
                    ssh_private_key_path: "~/.ssh/bb".to_string(),
                    ssh_public_key_path: "~/.ssh/bb.pub".to_string(),
                    sign_commits: false,
                    token: Some("bb-token".to_string()),
                }),
                is_active: true,
            }],
            oauth: Default::default(),
            repo_roots: Vec::new(),
            ..Default::default()
        }
    }

    #[test]
    fn migration_moves_plaintext_tokens_and_clears_json_state() {
        let mut state = legacy_state();
        let store = MockStore::default();

        assert!(migrate_plaintext_tokens_with_store(&mut state, &store).unwrap());

        assert_eq!(store.writes.borrow().len(), 2);
        assert!(state.profiles[0].github.as_ref().unwrap().token.is_none());
        assert!(state.profiles[0]
            .bitbucket
            .as_ref()
            .unwrap()
            .token
            .is_none());
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("\"token\""));
        assert!(!json.contains("gh-token"));
    }

    #[test]
    fn failed_migration_keeps_plaintext_tokens_in_memory() {
        let mut state = legacy_state();
        let store = MockStore {
            fail_after_writes: Some(1),
            ..Default::default()
        };

        assert!(migrate_plaintext_tokens_with_store(&mut state, &store).is_err());

        assert_eq!(store.writes.borrow().len(), 1);
        assert_eq!(
            state.profiles[0].github.as_ref().unwrap().token.as_deref(),
            Some("gh-token")
        );
        assert_eq!(
            state.profiles[0]
                .bitbucket
                .as_ref()
                .unwrap()
                .token
                .as_deref(),
            Some("bb-token")
        );
    }

    #[test]
    fn redact_masks_prefixed_and_long_unprefixed_tokens() {
        assert_eq!(
            redact("Authorization failed for ghp_16C7e42F292c6912E7710c838347Ae178B4a"),
            "Authorization failed for [redacted]"
        );
        assert_eq!(redact("token glpat-abc123DEF456"), "token [redacted]");
        assert_eq!(
            redact(&format!("Bearer {}", "a1b2c3d4".repeat(8))),
            "Bearer [redacted]"
        );
        assert_eq!(
            redact("password=ATATT3xFfGF0abcdefgh"),
            "password=[redacted]"
        );
    }

    #[test]
    fn redact_leaves_paths_and_ordinary_words_alone() {
        assert_eq!(
            redact("Could not read C:/Users/dev/.ssh/id_ed25519_gam_pc_github_work"),
            "Could not read C:/Users/dev/.ssh/id_ed25519_gam_pc_github_work"
        );
        assert_eq!(
            redact("git config --global --unset-all credential.https://github.com.helper"),
            "git config --global --unset-all credential.https://github.com.helper"
        );
    }

    #[test]
    fn https_credentials_live_under_their_own_key() {
        assert_eq!(
            https_account_name("p1", Platform::Github),
            "https:p1:github"
        );
        assert_ne!(
            https_account_name("p1", Platform::Github),
            account_name("p1", Platform::Github)
        );
    }

    #[test]
    fn migration_clears_empty_plaintext_tokens_without_storing_them() {
        let mut state = legacy_state();
        state.profiles[0].github.as_mut().unwrap().token = Some("".to_string());
        state.profiles[0].bitbucket.as_mut().unwrap().token = Some("   ".to_string());
        let store = MockStore::default();

        assert!(migrate_plaintext_tokens_with_store(&mut state, &store).unwrap());

        assert!(store.writes.borrow().is_empty());
        assert!(state.profiles[0].github.as_ref().unwrap().token.is_none());
        assert!(state.profiles[0]
            .bitbucket
            .as_ref()
            .unwrap()
            .token
            .is_none());
    }
}
