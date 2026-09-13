//! Taking back what older versions wrote inside repositories.
//!
//! Before the folder rules, an identity and an allow-list went into each
//! repository's own config and a `pre-push` guard into its hooks directory —
//! which, under husky, is a tracked folder. All of it is now supplied from
//! outside, and a leftover local copy is worse than clutter: `user.email` in a
//! repository's own config beats the rule, so it would go on naming an address
//! the profile has since changed.
//!
//! Runs once, and only removes a value this app would have written itself.

use crate::git;
use crate::models::{AppState, Profile, RepoRoot};
use crate::repos;

const HOOK_MARKER: &str = "# git-account-manager: pre-push identity guard";

/// The keys an older version wrote into a repository, paired with what it would
/// have written there. A different value is somebody else's decision.
fn expected_local(profile: &Profile, root: &RepoRoot) -> Vec<(&'static str, String)> {
    let Some(account) = profile.account(root.platform) else {
        return Vec::new();
    };
    let mut pairs = vec![
        ("user.name", account.git_name.clone()),
        ("user.email", account.git_email.clone()),
    ];
    if let Some(key) = account.signing_key() {
        pairs.push(("user.signingkey", key.replace('\\', "/")));
        pairs.push(("gpg.format", "ssh".to_string()));
        pairs.push(("commit.gpgsign", "true".to_string()));
        pairs.push(("tag.gpgsign", "true".to_string()));
    }
    pairs
}

#[derive(Debug, Default)]
pub struct MigrationReport {
    pub configs_cleaned: usize,
    pub hooks_removed: usize,
}

/// Cleans every repository under the known folders. Safe to call twice: a
/// repository with nothing of ours left in it is not touched.
pub fn run(state: &AppState) -> MigrationReport {
    let mut report = MigrationReport::default();

    for root in &state.repo_roots {
        let Some(profile) = state.profiles.iter().find(|p| p.id == root.profile_id) else {
            continue;
        };
        let expected = expected_local(profile, root);
        for repo in repos::scan_one(root, &state.profiles) {
            if clean_repo(&repo.path, &expected) {
                report.configs_cleaned += 1;
            }
            if remove_legacy_hook(&repo.path) {
                report.hooks_removed += 1;
            }
        }
    }

    report
}

fn clean_repo(dir: &str, expected: &[(&str, String)]) -> bool {
    let mut removed = false;

    // The allow-list came from this app and from nowhere else, so it goes
    // whatever it says; the folder rule supplies it now.
    if git::repo_config_get_local(dir, "gam.allowedEmail").is_some() {
        let _ = git::repo_config_unset_local(dir, &["gam.allowedEmail"]);
        removed = true;
    }

    for (key, value) in expected {
        if git::repo_config_get_local(dir, key).as_deref() == Some(value.as_str()) {
            let _ = git::repo_config_unset_local(dir, &[key]);
            removed = true;
        }
    }

    removed
}

/// Where older versions put the guard: the repository's hooks directory, or —
/// under husky, whose `core.hooksPath` owns `.husky/_` — the tracked folder
/// above it, which is the file that ended up in commits.
fn remove_legacy_hook(dir: &str) -> bool {
    let Some(hooks_dir) = git::repo_hooks_dir(dir) else {
        return false;
    };
    let husky_runner = hooks_dir.file_name().is_some_and(|name| name == "_")
        && (hooks_dir.join("h").exists() || hooks_dir.join("husky.sh").exists());
    let mut removed = false;
    let mut candidates = vec![hooks_dir.join("pre-push")];
    if husky_runner {
        if let Some(parent) = hooks_dir.parent() {
            candidates.push(parent.join("pre-push"));
        }
    }
    for path in candidates {
        let ours = std::fs::read_to_string(&path)
            .map(|body| body.contains(HOOK_MARKER))
            .unwrap_or(false);
        if ours && std::fs::remove_file(&path).is_ok() {
            removed = true;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Platform, PlatformAccount};
    use std::process::Command;

    fn profile() -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "Personal".to_string(),
            default_platform: None,
            github: Some(PlatformAccount {
                username: "octo".to_string(),
                git_name: "Octo".to_string(),
                git_email: "octo@example.com".to_string(),
                ssh_private_key_path: String::new(),
                ssh_public_key_path: String::new(),
                sign_commits: false,
                token: None,
            }),
            gitlab: None,
            bitbucket: None,
            is_active: true,
        }
    }

    /// What an install upgraded from the per-repository model actually holds:
    /// the identity this app wrote, an allow-list, and the hook that used to
    /// ride along with commits. All three go; a value somebody else chose stays.
    #[test]
    fn it_takes_back_what_this_app_wrote_and_leaves_the_rest() {
        let dir = std::env::temp_dir().join(format!("gam-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let ours = dir.join("ours");
        let theirs = dir.join("theirs");
        for repo in [&ours, &theirs] {
            std::fs::create_dir_all(repo).unwrap();
            let p = repo.to_string_lossy().replace('\\', "/");
            Command::new("git")
                .args(["init", "-q", &p])
                .output()
                .expect("git must be on PATH");
            Command::new("git")
                .args([
                    "-C",
                    &p,
                    "remote",
                    "add",
                    "origin",
                    "git@github.com:octo/x.git",
                ])
                .output()
                .unwrap();
        }

        let ours_path = ours.to_string_lossy().replace('\\', "/");
        git::repo_config_set_local(&ours_path, "user.name", "Octo").unwrap();
        git::repo_config_set_local(&ours_path, "user.email", "octo@example.com").unwrap();
        git::repo_config_set_local(&ours_path, "gam.allowedEmail", "octo@example.com").unwrap();
        let hook = ours.join(".git").join("hooks").join("pre-push");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, format!("#!/bin/sh\n{}\n", HOOK_MARKER)).unwrap();

        // Someone's own choice, and a hook that is not ours.
        let theirs_path = theirs.to_string_lossy().replace('\\', "/");
        git::repo_config_set_local(&theirs_path, "user.email", "me@personal.example").unwrap();
        let their_hook = theirs.join(".git").join("hooks").join("pre-push");
        std::fs::create_dir_all(their_hook.parent().unwrap()).unwrap();
        std::fs::write(&their_hook, "#!/bin/sh\nexit 0\n").unwrap();

        let state = AppState {
            profiles: vec![profile()],
            repo_roots: vec![RepoRoot {
                path: dir.to_string_lossy().replace('\\', "/"),
                profile_id: "p1".to_string(),
                platform: Platform::Github,
                fingerprint: vec![],
            }],
            ..AppState::default()
        };

        let report = run(&state);
        assert_eq!((report.configs_cleaned, report.hooks_removed), (1, 1));
        assert_eq!(git::repo_config_get_local(&ours_path, "user.email"), None);
        assert_eq!(
            git::repo_config_get_local(&ours_path, "gam.allowedEmail"),
            None
        );
        assert!(!hook.exists());

        assert_eq!(
            git::repo_config_get_local(&theirs_path, "user.email").as_deref(),
            Some("me@personal.example")
        );
        assert!(
            their_hook.exists(),
            "a hook this app did not write is not ours"
        );

        // Running again finds nothing left to do.
        let second = run(&state);
        assert_eq!((second.configs_cleaned, second.hooks_removed), (0, 0));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
