//! Repository-scoped identity.
//!
//! A machine has no owner, a repository does. Everything here binds an identity
//! to a repository instead of to the machine, and then checks that the binding
//! still holds. The identity is written to the repository's own config because
//! that is the one place every Git client agrees on: the CLI, libgit2 (which is
//! what TortoiseGit commits through), and the IDEs.

use crate::git;
use crate::models::{Profile, RepoBinding, RepoRoot};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const PLATFORMS: [&str; 3] = ["github", "gitlab", "bitbucket"];
const HOOK_MARKER: &str = "# git-account-manager: pre-push identity guard";

/// Directories that never contain a repository worth binding and cost a lot to
/// walk.
const SKIP_DIRS: [&str; 12] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".output",
    "vendor",
    ".venv",
    "__pycache__",
    ".cache",
    ".pnpm-store",
];

// ---- Remote URLs ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteRef {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

fn strip_scheme(url: &str) -> Option<&str> {
    for scheme in ["ssh://", "git://", "https://", "http://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            return Some(rest);
        }
    }
    None
}

/// Understands the four forms a Git remote actually takes: `git@host:owner/repo`,
/// `ssh://git@host[:port]/owner/repo`, `https://host/owner/repo` and the same
/// with an SSH host alias in place of the real host. GitLab subgroups stay in
/// `repo` (`group/sub/name`), so `owner` is always the top-level namespace.
pub fn parse_remote_url(url: &str) -> Option<RemoteRef> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    let (host, path) = match strip_scheme(url) {
        Some(rest) => {
            let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
            let (host_port, path) = rest.split_once('/')?;
            (host_port.split(':').next()?.to_string(), path.to_string())
        }
        None => {
            let rest = url.rsplit_once('@').map(|(_, r)| r).unwrap_or(url);
            let (host, path) = rest.split_once(':')?;
            (host.to_string(), path.to_string())
        }
    };

    // A bare Windows path ("D:/repos/x") parses as host "D"; a real host always
    // carries a dot or an alias hyphen.
    if host.len() < 2 || !(host.contains('.') || host.contains('-')) {
        return None;
    }

    let path = path.trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some(RemoteRef {
        host,
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

pub fn canonical_host(platform: &str) -> &'static str {
    match platform {
        "gitlab" => "gitlab.com",
        "bitbucket" => "bitbucket.org",
        _ => "github.com",
    }
}

/// Resolves a remote host to a platform. An alias host (`github-work`) also
/// tells us which profile the repository was pinned to.
pub fn platform_for_host(host: &str, profiles: &[Profile]) -> Option<(String, Option<String>)> {
    for platform in PLATFORMS {
        if host.eq_ignore_ascii_case(canonical_host(platform)) {
            return Some((platform.to_string(), None));
        }
    }
    for profile in profiles {
        for platform in PLATFORMS {
            if profile.account(platform).is_some()
                && host.eq_ignore_ascii_case(&format!("{}-{}", platform, profile.slug()))
            {
                return Some((platform.to_string(), Some(profile.id.clone())));
            }
        }
    }
    None
}

pub fn alias_url(platform: &str, profile: &Profile, remote: &RemoteRef) -> String {
    format!(
        "git@{}-{}:{}/{}.git",
        platform,
        profile.slug(),
        remote.owner,
        remote.repo
    )
}

// ---- Discovery ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRepo {
    pub path: String,
    pub name: String,
    pub remote_url: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    /// Which profile the evidence points at, and how sure we are.
    pub suggested_profile_id: Option<String>,
    pub suggested_platform: Option<String>,
    /// `alias` | `owner` | `ambiguous` | `unknown` — never guessed silently.
    pub reason: String,
    pub candidate_profile_ids: Vec<String>,
    pub bound: bool,
}

fn walk(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    if git::is_repo(dir) {
        out.push(dir.to_path_buf());
        // Keep descending: a container repository can hold independent
        // repositories that its own .gitignore hides.
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP_DIRS.iter().any(|s| name.as_ref() == *s) {
            continue;
        }
        walk(&entry.path(), depth + 1, max_depth, out);
    }
}

pub fn scan(roots: &[RepoRoot], profiles: &[Profile], bindings: &[RepoBinding]) -> Vec<DiscoveredRepo> {
    let mut found = Vec::new();
    for root in roots {
        let mut paths = Vec::new();
        walk(Path::new(&root.path), 0, 6, &mut paths);
        for path in paths {
            let dir = path.to_string_lossy().replace('\\', "/");
            let Some(url) = git::repo_remote_url(&dir, "origin") else {
                continue;
            };
            let Some(remote) = parse_remote_url(&url) else {
                continue;
            };
            if found.iter().any(|r: &DiscoveredRepo| r.path == dir) {
                continue;
            }
            let (profile_id, platform, reason, candidates) =
                suggest(&remote, profiles, Some(root));
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.clone());
            found.push(DiscoveredRepo {
                bound: bindings.iter().any(|b| b.path == dir),
                path: dir,
                name,
                remote_url: url,
                host: remote.host,
                owner: remote.owner,
                repo: remote.repo,
                suggested_profile_id: profile_id,
                suggested_platform: platform,
                reason,
                candidate_profile_ids: candidates,
            });
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// The evidence ladder: an alias host is an explicit pin, a namespace that
/// matches exactly one account is deterministic, anything else is handed back to
/// the user rather than guessed.
fn suggest(
    remote: &RemoteRef,
    profiles: &[Profile],
    root: Option<&RepoRoot>,
) -> (Option<String>, Option<String>, String, Vec<String>) {
    if let Some((platform, Some(profile_id))) = platform_for_host(&remote.host, profiles) {
        return (Some(profile_id), Some(platform), "alias".to_string(), vec![]);
    }

    let platform = platform_for_host(&remote.host, profiles).map(|(p, _)| p);

    if let Some(platform) = platform.clone() {
        let owners: Vec<&Profile> = profiles
            .iter()
            .filter(|p| {
                p.account(&platform)
                    .is_some_and(|a| a.username.eq_ignore_ascii_case(&remote.owner))
            })
            .collect();
        if owners.len() == 1 {
            return (
                Some(owners[0].id.clone()),
                Some(platform),
                "owner".to_string(),
                vec![],
            );
        }
        if owners.len() > 1 {
            return (
                None,
                Some(platform),
                "ambiguous".to_string(),
                owners.iter().map(|p| p.id.clone()).collect(),
            );
        }
    }

    // No account owns this namespace — an organisation or a fork. The folder the
    // repository sits in is a hint, not proof, so it is offered, not applied.
    let candidates: Vec<String> = root.map(|r| vec![r.profile_id.clone()]).unwrap_or_default();
    (None, platform, "unknown".to_string(), candidates)
}

// ---- Applying a binding ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindResult {
    pub identity: String,
    pub remote_url: Option<String>,
    /// `installed` | `kept-existing` | `unavailable` | `off`
    pub hook: String,
}

pub fn allowed_emails(binding: &RepoBinding, profile: &Profile) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    if let Some(account) = profile.account(&binding.platform) {
        if !account.git_email.is_empty() {
            list.push(account.git_email.clone());
        }
    }
    for extra in &binding.extra_allowed_emails {
        let extra = extra.trim();
        if !extra.is_empty() && !list.iter().any(|e| e == extra) {
            list.push(extra.to_string());
        }
    }
    list
}

pub fn apply_binding(binding: &RepoBinding, profile: &Profile) -> Result<BindResult, String> {
    let account = profile
        .account(&binding.platform)
        .ok_or_else(|| format!("Profile has no {} account", binding.platform))?;

    git::set_repo_identity(&binding.path, &account.git_name, &account.git_email)?;

    let allowed = allowed_emails(binding, profile);
    git::repo_config_replace_all(&binding.path, "gam.allowedEmail", &allowed)?;

    let mut rewritten = None;
    if binding.pin_remote_alias {
        if let Some(url) = git::repo_remote_url(&binding.path, "origin") {
            if let Some(remote) = parse_remote_url(&url) {
                let next = alias_url(&binding.platform, profile, &remote);
                if next != url {
                    git::set_repo_remote_url(&binding.path, "origin", &next)?;
                    rewritten = Some(next);
                }
            }
        }
    }

    let hook = if binding.install_hook {
        install_hook(&binding.path)?
    } else {
        "off".to_string()
    };

    Ok(BindResult {
        identity: account.git_email.clone(),
        remote_url: rewritten,
        hook,
    })
}

/// Writes the guard into whichever hooks directory this repository actually
/// uses. A `pre-push` that belongs to another tool is left alone — silently
/// replacing husky's hook would trade one broken guarantee for another.
fn install_hook(dir: &str) -> Result<String, String> {
    let Some(hooks_dir) = git::repo_hooks_dir(dir) else {
        return Ok("unavailable".to_string());
    };
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;
    let path = hooks_dir.join("pre-push");

    if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if !existing.contains(HOOK_MARKER) {
            return Ok("kept-existing".to_string());
        }
    }

    std::fs::write(&path, PRE_PUSH_HOOK).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok("installed".to_string())
}

pub fn remove_hook(dir: &str) -> Result<(), String> {
    let Some(hooks_dir) = git::repo_hooks_dir(dir) else {
        return Ok(());
    };
    let path = hooks_dir.join("pre-push");
    if path.exists() && std::fs::read_to_string(&path).unwrap_or_default().contains(HOOK_MARKER) {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn clear_binding(dir: &str) -> Result<(), String> {
    git::repo_config_replace_all(dir, "gam.allowedEmail", &[])?;
    remove_hook(dir)
}

const PRE_PUSH_HOOK: &str = r#"#!/bin/sh
# git-account-manager: pre-push identity guard
#
# Refuses a push carrying commits whose author or committer email this
# repository does not allow. The list lives in the repository's own config
# (`gam.allowedEmail`); with no list the hook does nothing.

allowed=$(git config --get-all gam.allowedEmail)
[ -z "$allowed" ] && exit 0

zero=0000000000000000000000000000000000000000
status=0

while read -r _local_ref local_sha _remote_ref remote_sha; do
	[ "$local_sha" = "$zero" ] && continue
	if [ "$remote_sha" = "$zero" ]; then
		range="$local_sha --not --remotes"
	else
		range="$remote_sha..$local_sha"
	fi
	for sha in $(git rev-list $range); do
		for email in $(git show -s --format='%ae %ce' "$sha"); do
			ok=0
			for candidate in $allowed; do
				if [ "$email" = "$candidate" ]; then
					ok=1
					break
				fi
			done
			if [ "$ok" -eq 0 ]; then
				echo "git-account-manager: refusing to push $(git rev-parse --short "$sha") - <$email> is not allowed in this repository" >&2
				status=1
			fi
		done
	done
done

if [ "$status" -ne 0 ]; then
	echo "git-account-manager: fix the commits or add the address in the app, then push again" >&2
fi
exit $status
"#;

// ---- Doctor ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCheck {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub path: String,
    pub name: String,
    pub profile_id: String,
    pub profile_name: String,
    pub platform: String,
    pub expected_email: String,
    pub effective_email: String,
    pub remote_url: String,
    pub offending_emails: Vec<String>,
    pub checks: Vec<RepoCheck>,
    pub ok: bool,
}

fn check(id: &str, ok: bool, detail: String) -> RepoCheck {
    RepoCheck {
        id: id.to_string(),
        ok,
        detail,
    }
}

pub fn inspect(binding: &RepoBinding, profile: &Profile) -> RepoStatus {
    let dir = binding.path.clone();
    let name = Path::new(&dir)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.clone());
    let account = profile.account(&binding.platform);
    let expected_email = account.map(|a| a.git_email.clone()).unwrap_or_default();
    let effective = git::repo_identity(&dir);
    let remote_url = git::repo_remote_url(&dir, "origin").unwrap_or_default();
    let allowed = allowed_emails(binding, profile);

    let mut checks = Vec::new();

    let exists = Path::new(&dir).exists();
    checks.push(check(
        "exists",
        exists,
        if exists { dir.clone() } else { String::new() },
    ));

    checks.push(check(
        "identity",
        !expected_email.is_empty() && effective.email == expected_email,
        effective.email.clone(),
    ));

    let local = git::repo_config_get_local(&dir, "user.email");
    checks.push(check(
        "local",
        local.as_deref() == Some(expected_email.as_str()),
        local.clone().unwrap_or_default(),
    ));

    let remote_ok = match parse_remote_url(&remote_url) {
        Some(parsed) => {
            let alias = format!("{}-{}", binding.platform, profile.slug());
            if binding.pin_remote_alias {
                parsed.host.eq_ignore_ascii_case(&alias)
            } else {
                parsed.host.eq_ignore_ascii_case(&alias)
                    || parsed
                        .host
                        .eq_ignore_ascii_case(canonical_host(&binding.platform))
            }
        }
        None => false,
    };
    checks.push(check("remote", remote_ok, remote_url.clone()));

    let seen = git::repo_recent_identities(&dir, 200);
    let offending: Vec<String> = seen
        .into_iter()
        .filter(|e| !allowed.iter().any(|a| a.eq_ignore_ascii_case(e)))
        .collect();
    checks.push(check("history", offending.is_empty(), offending.join(", ")));

    let hook_detail = hook_state(&dir);
    checks.push(check(
        "hooks",
        !binding.install_hook || hook_detail == "installed",
        hook_detail,
    ));

    let ok = checks.iter().all(|c| c.ok);
    RepoStatus {
        path: dir,
        name,
        profile_id: binding.profile_id.clone(),
        profile_name: profile.name.clone(),
        platform: binding.platform.clone(),
        expected_email,
        effective_email: effective.email,
        remote_url,
        offending_emails: offending,
        checks,
        ok,
    }
}

fn hook_state(dir: &str) -> String {
    let Some(hooks_dir) = git::repo_hooks_dir(dir) else {
        return "unavailable".to_string();
    };
    let path = hooks_dir.join("pre-push");
    if !path.exists() {
        return "missing".to_string();
    }
    if std::fs::read_to_string(&path)
        .unwrap_or_default()
        .contains(HOOK_MARKER)
    {
        "installed".to_string()
    } else {
        "kept-existing".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlatformAccount;
    use std::process::Command;

    fn r(host: &str, owner: &str, repo: &str) -> Option<RemoteRef> {
        Some(RemoteRef {
            host: host.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    #[test]
    fn parses_every_remote_shape() {
        assert_eq!(
            parse_remote_url("git@github.com:owner/repo.git"),
            r("github.com", "owner", "repo")
        );
        assert_eq!(
            parse_remote_url("git@github-work:owner/repo.git"),
            r("github-work", "owner", "repo")
        );
        assert_eq!(
            parse_remote_url("ssh://git@github.com:22/owner/repo.git"),
            r("github.com", "owner", "repo")
        );
        assert_eq!(
            parse_remote_url("https://github.com/owner/repo"),
            r("github.com", "owner", "repo")
        );
        assert_eq!(
            parse_remote_url("https://user@bitbucket.org/owner/repo.git"),
            r("bitbucket.org", "owner", "repo")
        );
        // GitLab subgroups keep the namespace tail in `repo`.
        assert_eq!(
            parse_remote_url("git@gitlab.com:group/sub/repo.git"),
            r("gitlab.com", "group", "sub/repo")
        );
    }

    #[test]
    fn rejects_non_remotes() {
        assert_eq!(parse_remote_url(""), None);
        assert_eq!(parse_remote_url("D:/repos/local"), None);
        assert_eq!(parse_remote_url("/srv/git/bare.git"), None);
        assert_eq!(parse_remote_url("git@github.com:owner"), None);
    }

    fn profile() -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "Personal".to_string(),
            default_platform: Some("github".to_string()),
            github: Some(PlatformAccount {
                username: "octo".to_string(),
                git_name: "Octo".to_string(),
                git_email: "1+octo@users.noreply.github.com".to_string(),
                ssh_private_key_path: String::new(),
                ssh_public_key_path: String::new(),
                token: None,
            }),
            gitlab: None,
            bitbucket: None,
            is_active: true,
        }
    }

    fn git(dir: &str, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git must be on PATH")
            .status
            .success();
        assert!(ok, "git {:?} failed in {}", args, dir);
    }

    fn second_profile() -> Profile {
        let mut p = profile();
        p.id = "p2".to_string();
        p.name = "Work".to_string();
        p.is_active = false;
        if let Some(account) = p.github.as_mut() {
            account.username = "octo".to_string();
            account.git_email = "2+octo@users.noreply.github.com".to_string();
        }
        p
    }

    #[test]
    fn evidence_ladder_never_guesses() {
        let profiles = vec![profile()];
        let root = RepoRoot {
            path: "/tmp/roots".to_string(),
            profile_id: "p1".to_string(),
            platform: "github".to_string(),
        };

        let alias = parse_remote_url("git@github-personal:octo/demo.git").unwrap();
        let (id, platform, reason, _) = suggest(&alias, &profiles, Some(&root));
        assert_eq!((id.as_deref(), platform.as_deref(), reason.as_str()), (Some("p1"), Some("github"), "alias"));

        let owned = parse_remote_url("git@github.com:octo/demo.git").unwrap();
        let (id, _, reason, _) = suggest(&owned, &profiles, Some(&root));
        assert_eq!((id.as_deref(), reason.as_str()), (Some("p1"), "owner"));

        // An organisation nobody's account owns: offered, never applied.
        let org = parse_remote_url("git@github.com:some-org/demo.git").unwrap();
        let (id, _, reason, candidates) = suggest(&org, &profiles, Some(&root));
        assert_eq!((id, reason.as_str()), (None, "unknown"));
        assert_eq!(candidates, vec!["p1".to_string()]);

        // Two accounts share the namespace: the user decides.
        let both = vec![profile(), second_profile()];
        let (id, _, reason, candidates) = suggest(&owned, &both, Some(&root));
        assert_eq!((id, reason.as_str()), (None, "ambiguous"));
        assert_eq!(candidates.len(), 2);
    }

    /// A container repository can hold independent repositories its own
    /// .gitignore hides, so discovery must keep descending past the first hit.
    #[test]
    fn scan_finds_repositories_nested_inside_a_repository() {
        let dir = std::env::temp_dir().join(format!("gam-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = dir.join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();

        for (path, url) in [
            (&outer, "git@github.com:octo/outer.git"),
            (&inner, "git@github.com:octo/inner.git"),
        ] {
            let p = path.to_string_lossy().replace('\\', "/");
            Command::new("git").args(["init", "-q", &p]).output().unwrap();
            git(&p, &["remote", "add", "origin", url]);
        }

        let roots = vec![RepoRoot {
            path: dir.to_string_lossy().replace('\\', "/"),
            profile_id: "p1".to_string(),
            platform: "github".to_string(),
        }];
        let found = scan(&roots, &[profile()], &[]);
        let names: Vec<&str> = found.iter().map(|r| r.repo.as_str()).collect();
        assert!(names.contains(&"outer"), "found: {:?}", names);
        assert!(names.contains(&"inner"), "found: {:?}", names);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole path a user takes: a repository with a remote gets bound, and
    /// the doctor then agrees that it is bound.
    #[test]
    fn binding_writes_identity_allow_list_and_hook() {
        let dir = std::env::temp_dir().join(format!("gam-bind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.to_string_lossy().replace('\\', "/");

        Command::new("git")
            .args(["init", "-q", &path])
            .output()
            .expect("git must be on PATH");
        git(&path, &["remote", "add", "origin", "git@github.com:octo/demo.git"]);

        let profile = profile();
        let binding = RepoBinding {
            path: path.clone(),
            profile_id: profile.id.clone(),
            platform: "github".to_string(),
            pin_remote_alias: true,
            install_hook: true,
            extra_allowed_emails: vec!["bot@example.com".to_string()],
        };

        let result = apply_binding(&binding, &profile).unwrap();
        assert_eq!(result.hook, "installed");
        assert_eq!(
            result.remote_url.as_deref(),
            Some("git@github-personal:octo/demo.git")
        );

        assert_eq!(
            crate::git::repo_config_get_local(&path, "user.email").as_deref(),
            Some("1+octo@users.noreply.github.com")
        );
        assert_eq!(
            allowed_emails(&binding, &profile),
            vec![
                "1+octo@users.noreply.github.com".to_string(),
                "bot@example.com".to_string()
            ]
        );

        // An empty repository has no history, so every other check must pass.
        let status = inspect(&binding, &profile);
        assert!(status.ok, "unexpected failures: {:?}", status.checks);

        clear_binding(&path).unwrap();
        assert!(!dir.join(".git/hooks/pre-push").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
