use crate::models::{AppState, PlatformAccount, Profile};
use keyring_core::{Entry, Error};
use std::sync::OnceLock;

const SERVICE: &str = "com.khasky.git-account-manager";
const PLATFORMS: [&str; 3] = ["github", "gitlab", "bitbucket"];

pub trait SecretStore {
    fn set_token(&self, profile_id: &str, platform: &str, token: &str) -> Result<(), String>;
    fn get_token(&self, profile_id: &str, platform: &str) -> Result<String, String>;
    fn delete_token(&self, profile_id: &str, platform: &str) -> Result<(), String>;

    fn delete_profile_tokens(&self, profile_id: &str) -> Result<(), String> {
        for platform in PLATFORMS {
            self.delete_token(profile_id, platform)?;
        }
        Ok(())
    }
}

pub struct OsSecretStore;

impl SecretStore for OsSecretStore {
    fn set_token(&self, profile_id: &str, platform: &str, token: &str) -> Result<(), String> {
        ensure_store()?;
        entry(profile_id, platform)?
            .set_password(token)
            .map_err(|e| store_error("save", e))
    }

    fn get_token(&self, profile_id: &str, platform: &str) -> Result<String, String> {
        ensure_store()?;
        entry(profile_id, platform)?
            .get_password()
            .map_err(|e| match e {
                Error::NoEntry => format!(
                    "No stored token for {}. Reconnect the account and try again.",
                    platform_label(platform)
                ),
                other => store_error("read", other),
            })
    }

    fn delete_token(&self, profile_id: &str, platform: &str) -> Result<(), String> {
        ensure_store()?;
        match entry(profile_id, platform)?.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(store_error("delete", e)),
        }
    }
}

pub fn set_token(profile_id: &str, platform: &str, token: &str) -> Result<(), String> {
    OsSecretStore.set_token(profile_id, platform, token)
}

pub fn get_token(profile_id: &str, platform: &str) -> Result<String, String> {
    OsSecretStore.get_token(profile_id, platform)
}

pub fn delete_token(profile_id: &str, platform: &str) -> Result<(), String> {
    OsSecretStore.delete_token(profile_id, platform)
}

pub fn delete_profile_tokens(profile_id: &str) -> Result<(), String> {
    OsSecretStore.delete_profile_tokens(profile_id)
}

pub fn migrate_plaintext_tokens(state: &mut AppState) -> Result<bool, String> {
    migrate_plaintext_tokens_with_store(state, &OsSecretStore)
}

pub fn migrate_plaintext_tokens_with_store<S: SecretStore>(
    state: &mut AppState,
    store: &S,
) -> Result<bool, String> {
    let mut pending: Vec<(String, &'static str, String)> = Vec::new();
    let mut found_legacy_tokens = false;

    for profile in &state.profiles {
        found_legacy_tokens |=
            collect_legacy_token(&mut pending, profile, "github", profile.github.as_ref());
        found_legacy_tokens |=
            collect_legacy_token(&mut pending, profile, "gitlab", profile.gitlab.as_ref());
        found_legacy_tokens |= collect_legacy_token(
            &mut pending,
            profile,
            "bitbucket",
            profile.bitbucket.as_ref(),
        );
    }

    if !found_legacy_tokens {
        return Ok(false);
    }

    for (profile_id, platform, token) in &pending {
        store.set_token(profile_id, platform, token)?;
    }

    for profile in &mut state.profiles {
        clear_legacy_token(profile.github.as_mut());
        clear_legacy_token(profile.gitlab.as_mut());
        clear_legacy_token(profile.bitbucket.as_mut());
    }

    Ok(true)
}

fn collect_legacy_token(
    pending: &mut Vec<(String, &'static str, String)>,
    profile: &Profile,
    platform: &'static str,
    account: Option<&PlatformAccount>,
) -> bool {
    if let Some(token) = account.and_then(|a| a.token.as_ref()) {
        if !token.trim().is_empty() {
            pending.push((profile.id.clone(), platform, token.clone()));
        }
        true
    } else {
        false
    }
}

fn clear_legacy_token(account: Option<&mut PlatformAccount>) {
    if let Some(account) = account {
        account.token = None;
    }
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

fn entry(profile_id: &str, platform: &str) -> Result<Entry, String> {
    validate_platform(platform)?;
    Entry::new(SERVICE, &account_name(profile_id, platform)).map_err(|e| store_error("open", e))
}

fn account_name(profile_id: &str, platform: &str) -> String {
    format!("token:{}:{}", profile_id, platform)
}

fn validate_platform(platform: &str) -> Result<(), String> {
    if PLATFORMS.contains(&platform) {
        Ok(())
    } else {
        Err(format!("Unknown platform: {}", platform))
    }
}

fn platform_label(platform: &str) -> &'static str {
    match platform {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "bitbucket" => "Bitbucket",
        _ => "platform",
    }
}

fn store_error(action: &str, err: Error) -> String {
    format!(
        "Could not {} token in the OS credential store: {}",
        action, err
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MockStore {
        writes: RefCell<Vec<(String, String, String)>>,
        fail_after_writes: Option<usize>,
    }

    impl SecretStore for MockStore {
        fn set_token(&self, profile_id: &str, platform: &str, token: &str) -> Result<(), String> {
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

        fn get_token(&self, _profile_id: &str, _platform: &str) -> Result<String, String> {
            unimplemented!()
        }

        fn delete_token(&self, _profile_id: &str, _platform: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn legacy_state() -> AppState {
        AppState {
            profiles: vec![Profile {
                id: "profile-1".to_string(),
                name: "Work".to_string(),
                default_platform: Some("github".to_string()),
                github: Some(PlatformAccount {
                    username: "octo".to_string(),
                    git_name: "Octo".to_string(),
                    git_email: "octo@example.com".to_string(),
                    ssh_private_key_path: "~/.ssh/id".to_string(),
                    ssh_public_key_path: "~/.ssh/id.pub".to_string(),
                    token: Some("gh-token".to_string()),
                }),
                gitlab: None,
                bitbucket: Some(PlatformAccount {
                    username: "bb".to_string(),
                    git_name: "BB".to_string(),
                    git_email: "bb@example.com".to_string(),
                    ssh_private_key_path: "~/.ssh/bb".to_string(),
                    ssh_public_key_path: "~/.ssh/bb.pub".to_string(),
                    token: Some("bb-token".to_string()),
                }),
                is_active: true,
            }],
            oauth: Default::default(),
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
