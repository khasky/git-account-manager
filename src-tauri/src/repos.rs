//! Repository-scoped identity.
//!
//! A machine has no owner, a repository does. Everything here binds an identity
//! to a repository instead of to the machine, and then checks that the binding
//! still holds. The identity is written to the repository's own config because
//! that is the one place every Git client agrees on: the CLI, libgit2 (which is
//! what TortoiseGit commits through), and the IDEs.

use crate::git;
use crate::models::{Platform, Profile, RepoBinding, RepoRoot, PLATFORMS};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const HOOK_MARKER: &str = "# git-account-manager: pre-push identity guard";

/// How deep a watched folder is walked. Deep enough for the usual
/// `<root>/<org>/<repo>` layouts without turning a scan into a full disk crawl.
const MAX_SCAN_DEPTH: usize = 6;

/// How far back the doctor reads author and committer addresses. Far enough to
/// catch a wrong identity that has been in use for a while, short enough that
/// the check stays instant on a large repository.
const HISTORY_COMMITS_CHECKED: usize = 200;

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

/// The SSH host alias that pins a remote to one profile's key.
pub fn host_alias(platform: Platform, profile: &Profile) -> String {
    format!("{}-{}", platform.as_str(), profile.slug())
}

/// Resolves a remote host to a platform. An alias host (`github-work`) also
/// tells us which profile the repository was pinned to.
pub fn platform_for_host(host: &str, profiles: &[Profile]) -> Option<(Platform, Option<String>)> {
    for platform in PLATFORMS {
        if host.eq_ignore_ascii_case(platform.canonical_host()) {
            return Some((platform, None));
        }
    }
    for profile in profiles {
        for platform in PLATFORMS {
            if profile.account(platform).is_some()
                && host.eq_ignore_ascii_case(&host_alias(platform, profile))
            {
                return Some((platform, Some(profile.id.clone())));
            }
        }
    }
    None
}

pub fn alias_url(platform: Platform, profile: &Profile, remote: &RemoteRef) -> String {
    format!(
        "git@{}:{}/{}.git",
        host_alias(platform, profile),
        remote.owner,
        remote.repo
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRepo {
    pub path: String,
    pub name: String,
    /// The watched folder this was found under, so the caller can read that
    /// folder's default switches without re-deriving which root contains it.
    pub root_path: String,
    pub remote_url: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
    /// Which profile the evidence points at, and how sure we are.
    pub suggested_profile_id: Option<String>,
    pub suggested_platform: Option<Platform>,
    /// `alias` | `owner` | `ambiguous` | `unknown` — never guessed silently.
    pub reason: String,
    pub candidate_profile_ids: Vec<String>,
    pub bound: bool,
    /// Switches resolved here rather than in the caller, so the folder-default
    /// rule has one implementation. See `effective_switches`.
    pub install_hook: bool,
    pub pin_remote_alias: bool,
    pub overrides_root: bool,
}

/// The switches a repository starts with. A folder's defaults reach everything
/// inside it, except a repository the user deliberately set apart — that
/// exception survives a later edit of the folder's defaults, which is the whole
/// reason the flag is stored rather than inferred from a value comparison.
pub fn effective_switches(root: &RepoRoot, existing: Option<&RepoBinding>) -> (bool, bool) {
    match existing {
        Some(binding) if binding.overrides_root => (binding.install_hook, binding.pin_remote_alias),
        _ => (root.install_hook, root.pin_remote_alias),
    }
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

pub fn scan(
    roots: &[RepoRoot],
    profiles: &[Profile],
    bindings: &[RepoBinding],
) -> Vec<DiscoveredRepo> {
    let mut found = Vec::new();
    for root in roots {
        let mut paths = Vec::new();
        walk(Path::new(&root.path), 0, MAX_SCAN_DEPTH, &mut paths);
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
            let (profile_id, platform, reason, candidates) = suggest(&remote, profiles, Some(root));
            let existing = bindings.iter().find(|b| b.path == dir);
            let (install_hook, pin_remote_alias) = effective_switches(root, existing);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.clone());
            found.push(DiscoveredRepo {
                bound: existing.is_some(),
                overrides_root: existing.is_some_and(|b| b.overrides_root),
                install_hook,
                pin_remote_alias,
                path: dir,
                name,
                root_path: root.path.clone(),
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
) -> (Option<String>, Option<Platform>, String, Vec<String>) {
    if let Some((platform, Some(profile_id))) = platform_for_host(&remote.host, profiles) {
        return (
            Some(profile_id),
            Some(platform),
            "alias".to_string(),
            vec![],
        );
    }

    let platform = platform_for_host(&remote.host, profiles).map(|(p, _)| p);

    if let Some(platform) = platform {
        let owners: Vec<&Profile> = profiles
            .iter()
            .filter(|p| {
                p.account(platform)
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

#[derive(Debug, Serialize)]
pub struct RepoReach {
    pub reachable: bool,
    pub full_name: String,
    /// Empty when reachable; otherwise Git's own explanation.
    pub detail: String,
}

/// Answers what a push actually depends on: does *this profile's key* reach this
/// repository? Asking the platform API instead would answer a different question
/// and answer it wrongly — GitHub returns 404 for a private repository whenever
/// the token lacks the `repo` scope, so every private repository reads as
/// missing, and the token is not what carries a push anyway.
///
/// The canonical host is used with `IdentitiesOnly`, so the answer describes the
/// profile's key rather than whichever key an SSH alias currently resolves to.
pub fn reach(profile: &Profile, platform: Platform, owner: &str, repo: &str) -> RepoReach {
    let full_name = format!("{}/{}", owner, repo);
    let deny = |detail: String| RepoReach {
        reachable: false,
        full_name: full_name.clone(),
        detail,
    };

    let Some(account) = profile.account(platform) else {
        return deny(format!("Profile has no {} account", platform.label()));
    };
    if account.ssh_private_key_path.trim().is_empty() {
        return deny(format!("Profile has no SSH key for {}", platform.label()));
    }

    let url = format!("git@{}:{}/{}.git", platform.canonical_host(), owner, repo);
    match git::ls_remote_with_key(&url, &account.ssh_private_key_path) {
        Ok(()) => RepoReach {
            reachable: true,
            full_name,
            detail: String::new(),
        },
        Err(detail) => deny(detail),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindResult {
    pub identity: String,
    pub remote_url: Option<String>,
    /// `installed` | `kept-existing` | `unavailable` | `off`
    pub hook: String,
}

pub fn allowed_emails(binding: &RepoBinding, profile: &Profile) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    if let Some(account) = profile.account(binding.platform) {
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

/// Everything the profile form collected about one profile's folders.
#[derive(Debug, Clone, Deserialize)]
pub struct RepoPlan {
    pub profile_id: String,
    pub roots: Vec<RepoRoot>,
    pub bindings: Vec<RepoBinding>,
    /// Repositories the form dropped — a removed folder, or an explicit unbind.
    pub released: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ApplyReport {
    pub bound: usize,
    pub released: usize,
    pub failed: Vec<ApplyFailure>,
}

#[derive(Debug, Serialize)]
pub struct ApplyFailure {
    pub path: String,
    pub error: String,
}

/// True when `path` is the folder itself or lives under it.
fn is_inside(root: &str, path: &str) -> bool {
    path == root || path.starts_with(&format!("{}/", root))
}

/// A binding written before folders carried defaults has no `overrides_root`, so
/// a switch the user turned off on one repository would quietly come back the
/// first time its folder's defaults reached it. Values that already diverge are
/// the record of that decision — mark them, and the folder leaves them alone.
pub fn mark_pre_existing_exceptions(roots: &[RepoRoot], bindings: &mut [RepoBinding]) {
    for binding in bindings.iter_mut() {
        if binding.overrides_root {
            continue;
        }
        let Some(root) = roots.iter().find(|r| is_inside(&r.path, &binding.path)) else {
            continue;
        };
        if binding.install_hook != root.install_hook
            || binding.pin_remote_alias != root.pin_remote_alias
        {
            binding.overrides_root = true;
        }
    }
}

/// Writes a profile's folders and bindings into `state` and onto disk.
///
/// One repository that refuses does not cancel the rest: a missing directory or
/// a read-only config is a fact about that repository, and stopping there would
/// leave the batch half-applied with nothing said about which half.
pub fn apply_plan(
    profiles: &[Profile],
    roots: &mut Vec<RepoRoot>,
    bindings: &mut Vec<RepoBinding>,
    plan: RepoPlan,
) -> Result<ApplyReport, String> {
    let profile = profiles
        .iter()
        .find(|p| p.id == plan.profile_id)
        .ok_or_else(|| format!("Unknown profile: {}", plan.profile_id))?;

    // Only this profile's folders are replaced; another profile's stay put.
    roots.retain(|r| r.profile_id != plan.profile_id);
    roots.extend(plan.roots);

    // Release before binding: a repository handed to another profile has to lose
    // the previous allow-list first, or a stale one would outlive the change.
    let released = plan.released.len();
    for path in &plan.released {
        clear_binding(path).ok();
        bindings.retain(|b| &b.path != path);
    }

    let mut bound = 0;
    let mut failed = Vec::new();
    for mut binding in plan.bindings {
        // The form cannot know what a repository's remote was before a previous
        // run pinned it, so the stored binding keeps that memory across saves.
        binding.original_remote_url = bindings
            .iter()
            .find(|b| b.path == binding.path)
            .and_then(|b| b.original_remote_url.clone());

        match apply_binding(&mut binding, profile) {
            Ok(_) => {
                match bindings.iter_mut().find(|b| b.path == binding.path) {
                    Some(existing) => *existing = binding,
                    None => bindings.push(binding),
                }
                bound += 1;
            }
            Err(error) => failed.push(ApplyFailure {
                path: binding.path,
                error,
            }),
        }
    }

    Ok(ApplyReport {
        bound,
        released,
        failed,
    })
}

/// Writes a binding to its repository.
///
/// Both switches undo themselves: clearing one puts back what was there rather
/// than merely stopping short of writing it again, or a repository would keep a
/// guard and an alias the user had just turned off.
pub fn apply_binding(binding: &mut RepoBinding, profile: &Profile) -> Result<BindResult, String> {
    let account = profile
        .account(binding.platform)
        .ok_or_else(|| format!("Profile has no {} account", binding.platform.label()))?;

    git::set_repo_identity(&binding.path, &account.git_name, &account.git_email)?;
    git::set_repo_signing(&binding.path, account.signing_key())?;

    let allowed = allowed_emails(binding, profile);
    git::repo_config_replace_all(&binding.path, "gam.allowedEmail", &allowed)?;

    let alias = host_alias(binding.platform, profile);
    let current = git::repo_remote_url(&binding.path, "origin");
    let mut rewritten = None;

    if binding.pin_remote_alias {
        if let Some(url) = current {
            if let Some(remote) = parse_remote_url(&url) {
                let next = alias_url(binding.platform, profile, &remote);
                if next != url {
                    // Remembered before the overwrite, and only the first time:
                    // re-applying an already pinned remote must not record the
                    // alias as the thing to restore.
                    binding.original_remote_url = Some(url);
                    git::set_repo_remote_url(&binding.path, "origin", &next)?;
                    rewritten = Some(next);
                }
            }
        }
    } else if let Some(url) = current {
        // Only a remote still carrying this profile's alias is ours to undo; one
        // the user aliased by hand is left alone.
        let is_ours = parse_remote_url(&url).is_some_and(|r| r.host.eq_ignore_ascii_case(&alias));
        if is_ours {
            if let Some(original) = binding.original_remote_url.take() {
                git::set_repo_remote_url(&binding.path, "origin", &original)?;
                rewritten = Some(original);
            }
        }
    }

    let hook = if binding.install_hook {
        install_hook(&binding.path)?
    } else {
        remove_hook(&binding.path)?;
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
/// Where this repository's `pre-push` guard belongs.
///
/// husky points `core.hooksPath` at `.husky/_`, and every file in there is one of
/// husky's own runners: each looks for a same-named file in the parent directory
/// and executes it. Writing into `_` would fight husky over that file, while the
/// parent is exactly the slot husky exists to call — so the guard installs there
/// and both hooks run, ours and whatever the project set husky up for.
fn pre_push_path(dir: &str) -> Option<PathBuf> {
    let hooks_dir = git::repo_hooks_dir(dir)?;
    let is_husky_runner_dir = hooks_dir.file_name().is_some_and(|name| name == "_")
        && (hooks_dir.join("h").exists() || hooks_dir.join("husky.sh").exists());
    let target = if is_husky_runner_dir {
        hooks_dir.parent()?.to_path_buf()
    } else {
        hooks_dir
    };
    Some(target.join("pre-push"))
}

fn install_hook(dir: &str) -> Result<String, String> {
    let Some(path) = pre_push_path(dir) else {
        return Ok("unavailable".to_string());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

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

/// Removes only a guard this app wrote. It has to resolve the path the same way
/// `install_hook` does — under husky the guard lives one directory above the
/// hooks path, and looking in the hooks path itself would leave it behind,
/// refusing pushes for a binding that no longer exists.
pub fn remove_hook(dir: &str) -> Result<(), String> {
    let Some(path) = pre_push_path(dir) else {
        return Ok(());
    };
    if path.exists()
        && std::fs::read_to_string(&path)
            .unwrap_or_default()
            .contains(HOOK_MARKER)
    {
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

# One `git log` for a whole range, not one `git show` per commit: pushing a
# branch of a few hundred commits used to spawn a few hundred processes, and on
# Windows each one is a visible cost. The loop reads from a here-document
# rather than a pipe so that `status` set inside it survives — a piped `while`
# runs in a subshell and its assignments are lost.
check_range() {
	commits=$(git log --format='%h %ae %ce' "$@")
	while read -r short author committer; do
		[ -z "$short" ] && continue
		for email in "$author" "$committer"; do
			[ -z "$email" ] && continue
			ok=0
			for candidate in $allowed; do
				if [ "$email" = "$candidate" ]; then
					ok=1
					break
				fi
			done
			if [ "$ok" -eq 0 ]; then
				echo "git-account-manager: refusing to push $short - <$email> is not allowed in this repository" >&2
				status=1
			fi
			# An ordinary commit authors and commits under one address; naming
			# it twice would print the same refusal twice.
			[ "$author" = "$committer" ] && break
		done
	done <<EOF
$commits
EOF
}

while read -r _local_ref local_sha _remote_ref remote_sha; do
	[ "$local_sha" = "$zero" ] && continue
	if [ "$remote_sha" = "$zero" ]; then
		check_range "$local_sha" --not --remotes
	else
		check_range "$remote_sha..$local_sha"
	fi
done

if [ "$status" -ne 0 ]; then
	echo "git-account-manager: fix the commits or add the address in the app, then push again" >&2
fi
exit $status
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCheck {
    pub id: String,
    pub ok: bool,
    pub detail: String,
    /// Where to look, when knowing that is the difference between a report and
    /// an instruction. Empty when the detail already says everything.
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub path: String,
    pub name: String,
    pub profile_id: String,
    pub profile_name: String,
    pub platform: Platform,
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
        hint: String::new(),
    }
}

/// How many repositories are inspected at once. Each one is mostly spent waiting
/// on Git processes, so the cap is about not spawning a thread per repository on
/// a tree with hundreds of them, not about saturating the CPU.
const INSPECT_CONCURRENCY: usize = 8;

/// Inspects every binding, several at a time.
///
/// A single repository costs roughly eight Git processes and they are slow to
/// spawn on Windows; the repositories do not depend on each other, so running
/// them one after another made the report take the sum rather than the slowest.
pub fn inspect_all(bindings: &[RepoBinding], profiles: &[Profile]) -> Vec<RepoStatus> {
    let pairs: Vec<(&RepoBinding, &Profile)> = bindings
        .iter()
        .filter_map(|b| {
            profiles
                .iter()
                .find(|p| p.id == b.profile_id)
                .map(|p| (b, p))
        })
        .collect();

    let mut out = Vec::with_capacity(pairs.len());
    for chunk in pairs.chunks(INSPECT_CONCURRENCY) {
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(binding, profile)| scope.spawn(move || inspect(binding, profile)))
                .collect();
            for handle in handles {
                if let Ok(status) = handle.join() {
                    out.push(status);
                }
            }
        });
    }
    out
}

pub fn inspect(binding: &RepoBinding, profile: &Profile) -> RepoStatus {
    let dir = binding.path.clone();
    let name = Path::new(&dir)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.clone());
    let account = profile.account(binding.platform);
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
            let alias = host_alias(binding.platform, profile);
            if binding.pin_remote_alias {
                parsed.host.eq_ignore_ascii_case(&alias)
            } else {
                parsed.host.eq_ignore_ascii_case(&alias)
                    || parsed
                        .host
                        .eq_ignore_ascii_case(binding.platform.canonical_host())
            }
        }
        None => false,
    };
    checks.push(check("remote", remote_ok, remote_url.clone()));

    let seen = git::repo_recent_identities(&dir, HISTORY_COMMITS_CHECKED);
    let offending: Vec<String> = seen
        .into_iter()
        .filter(|e| !allowed.iter().any(|a| a.eq_ignore_ascii_case(e)))
        .collect();
    checks.push(check("history", offending.is_empty(), offending.join(", ")));

    let (hook_detail, hook_path) = hook_state(&dir);
    let mut hooks_check = check(
        "hooks",
        !binding.install_hook || hook_detail == "installed",
        hook_detail,
    );
    hooks_check.hint = hook_path;
    checks.push(hooks_check);

    let ok = checks.iter().all(|c| c.ok);
    RepoStatus {
        path: dir,
        name,
        profile_id: binding.profile_id.clone(),
        profile_name: profile.name.clone(),
        platform: binding.platform,
        expected_email,
        effective_email: effective.email,
        remote_url,
        offending_emails: offending,
        checks,
        ok,
    }
}

/// The state of the guard plus the file it was read from, so a report can name
/// what to open instead of leaving the user to work out where hooks live here.
fn hook_state(dir: &str) -> (String, String) {
    let Some(path) = pre_push_path(dir) else {
        return ("unavailable".to_string(), String::new());
    };
    let shown = path.to_string_lossy().replace('\\', "/");
    if !path.exists() {
        return ("missing".to_string(), shown);
    }
    let state = if std::fs::read_to_string(&path)
        .unwrap_or_default()
        .contains(HOOK_MARKER)
    {
        "installed"
    } else {
        "kept-existing"
    };
    (state.to_string(), shown)
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
            default_platform: Some(Platform::Github),
            github: Some(PlatformAccount {
                username: "octo".to_string(),
                git_name: "Octo".to_string(),
                git_email: "1+octo@users.noreply.github.com".to_string(),
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

    fn root_at(path: &str) -> RepoRoot {
        RepoRoot {
            path: path.to_string(),
            profile_id: "p1".to_string(),
            platform: Platform::Github,
            install_hook: true,
            pin_remote_alias: false,
        }
    }

    fn binding_at(path: &str, install_hook: bool, pin_alias: bool, overrides: bool) -> RepoBinding {
        RepoBinding {
            path: path.to_string(),
            profile_id: "p1".to_string(),
            platform: Platform::Github,
            pin_remote_alias: pin_alias,
            install_hook,
            extra_allowed_emails: vec![],
            overrides_root: overrides,
            original_remote_url: None,
        }
    }

    /// Editing a folder's defaults must reach the repositories that follow it and
    /// must not touch the one the user deliberately set apart.
    #[test]
    fn folder_defaults_reach_everything_except_a_deliberate_exception() {
        let mut root = root_at("/tmp/roots");

        assert_eq!(effective_switches(&root, None), (true, false));

        let inherits = binding_at("/tmp/roots/a", false, true, false);
        assert_eq!(effective_switches(&root, Some(&inherits)), (true, false));

        let exception = binding_at("/tmp/roots/b", false, true, true);
        assert_eq!(effective_switches(&root, Some(&exception)), (false, true));

        root.install_hook = false;
        root.pin_remote_alias = true;
        assert_eq!(effective_switches(&root, Some(&inherits)), (false, true));
        assert_eq!(effective_switches(&root, Some(&exception)), (false, true));
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
        let root = root_at("/tmp/roots");

        let alias = parse_remote_url("git@github-personal:octo/demo.git").unwrap();
        let (id, platform, reason, _) = suggest(&alias, &profiles, Some(&root));
        assert_eq!(
            (id.as_deref(), platform, reason.as_str()),
            (Some("p1"), Some(Platform::Github), "alias")
        );

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
            Command::new("git")
                .args(["init", "-q", &p])
                .output()
                .unwrap();
            git(&p, &["remote", "add", "origin", url]);
        }

        let roots = vec![root_at(&dir.to_string_lossy().replace('\\', "/"))];
        let found = scan(&roots, &[profile()], &[]);
        let names: Vec<&str> = found.iter().map(|r| r.repo.as_str()).collect();
        assert!(names.contains(&"outer"), "found: {:?}", names);
        assert!(names.contains(&"inner"), "found: {:?}", names);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The write path Save takes. A repository that cannot be written must not
    /// cancel the ones that can, and what it did has to be reported honestly.
    #[test]
    fn saving_applies_the_batch_releases_the_rest_and_reports_failures() {
        let dir = std::env::temp_dir().join(format!("gam-plan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut paths = Vec::new();
        for name in ["keep", "drop"] {
            let repo = dir.join(name);
            std::fs::create_dir_all(&repo).unwrap();
            let p = repo.to_string_lossy().replace('\\', "/");
            Command::new("git")
                .args(["init", "-q", &p])
                .output()
                .unwrap();
            git(
                &p,
                &[
                    "remote",
                    "add",
                    "origin",
                    &format!("git@github.com:octo/{}.git", name),
                ],
            );
            paths.push(p);
        }
        let (keep, drop) = (paths[0].clone(), paths[1].clone());
        let missing = dir.to_string_lossy().replace('\\', "/") + "/not-a-repository";

        // `drop` is already bound, so releasing it has something to clear.
        let profiles = vec![profile()];
        let mut roots = vec![root_at(&dir.to_string_lossy().replace('\\', "/"))];
        let mut bindings = vec![binding_at(&drop, true, false, false)];
        apply_binding(&mut bindings[0], &profiles[0]).unwrap();
        assert!(git::repo_config_get(&drop, "gam.allowedEmail").is_some());

        let planned_roots = roots.clone();
        let report = apply_plan(
            &profiles,
            &mut roots,
            &mut bindings,
            RepoPlan {
                profile_id: "p1".to_string(),
                roots: planned_roots,
                bindings: vec![
                    binding_at(&keep, true, false, false),
                    binding_at(&missing, true, false, false),
                ],
                released: vec![drop.clone()],
            },
        )
        .unwrap();

        assert_eq!((report.bound, report.released), (1, 1));
        assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
        assert_eq!(report.failed[0].path, missing);

        // The good one is written and stored, the released one is cleared, and
        // the failure left nothing behind.
        assert_eq!(
            git::repo_config_get(&keep, "user.email").as_deref(),
            Some("1+octo@users.noreply.github.com")
        );
        assert!(git::repo_config_get(&drop, "gam.allowedEmail").is_none());
        let stored: Vec<&str> = bindings.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(stored, vec![keep.as_str()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The form builds this object in TypeScript, so nothing but a round trip
    /// proves the field names still line up. A rename on either side turns Save
    /// into an error dialog with no other warning.
    #[test]
    fn the_plan_the_form_sends_still_deserializes() {
        let sent = r#"{
          "profile_id": "p1",
          "roots": [{
            "path": "D:/repos/github/khasky",
            "profile_id": "p1",
            "platform": "github",
            "install_hook": true,
            "pin_remote_alias": false
          }],
          "bindings": [{
            "path": "D:/repos/github/khasky/demo",
            "profile_id": "p1",
            "platform": "github",
            "install_hook": true,
            "pin_remote_alias": false,
            "extra_allowed_emails": [],
            "overrides_root": false
          }],
          "released": ["D:/repos/github/khasky/gone"]
        }"#;

        let plan: RepoPlan = serde_json::from_str(sent).expect("form payload must parse");
        assert_eq!(plan.profile_id, "p1");
        assert!(plan.roots[0].install_hook);
        assert!(!plan.bindings[0].overrides_root);
        assert_eq!(
            plan.released,
            vec!["D:/repos/github/khasky/gone".to_string()]
        );
    }

    /// A switch turned off before folders had defaults is a decision, not a
    /// value waiting to be overwritten.
    #[test]
    fn an_old_binding_that_diverges_becomes_an_exception() {
        let root = root_at("/tmp/roots"); // install_hook: true, pin: false
        let mut bindings = vec![
            binding_at("/tmp/roots/off", false, false, false),
            binding_at("/tmp/roots/same", true, false, false),
            binding_at("/tmp/elsewhere/other", false, true, false),
        ];

        mark_pre_existing_exceptions(std::slice::from_ref(&root), &mut bindings);

        assert!(bindings[0].overrides_root, "diverging binding must be kept");
        assert!(
            !bindings[1].overrides_root,
            "matching binding must follow the folder"
        );
        assert!(
            !bindings[2].overrides_root,
            "binding outside the folder is not ours"
        );

        // The folder's own defaults still reach the one that matched.
        assert_eq!(
            effective_switches(&root, Some(&bindings[0])),
            (false, false)
        );
        assert_eq!(effective_switches(&root, Some(&bindings[1])), (true, false));
    }

    /// `sh` is what git runs a hook with. Git for Windows ships one but does not
    /// always put it on PATH, so the hook's behaviour is proven wherever a shell
    /// exists and the test says so out loud where one does not — CI's Linux job
    /// always runs it.
    fn shell() -> Option<&'static str> {
        ["sh", "bash", "C:/Program Files/Git/usr/bin/sh.exe"]
            .into_iter()
            .find(|candidate| {
                Command::new(candidate)
                    .arg("-c")
                    .arg("exit 0")
                    .output()
                    .is_ok()
            })
    }

    /// The guard is the last thing between a wrong identity and a public
    /// repository, so it is checked by running it, not by reading it.
    #[test]
    fn the_pre_push_hook_refuses_a_disallowed_address_and_passes_an_allowed_one() {
        let Some(sh) = shell() else {
            eprintln!("no POSIX shell on PATH: pre-push hook behaviour not verified here");
            return;
        };

        let dir = std::env::temp_dir().join(format!("gam-hook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.to_string_lossy().replace('\\', "/");
        Command::new("git")
            .args(["init", "-q", &path])
            .output()
            .unwrap();
        git(&path, &["config", "user.name", "Octo"]);
        git(&path, &["config", "user.email", "stranger@example.com"]);
        git(&path, &["config", "commit.gpgsign", "false"]);
        git(
            &path,
            &["commit", "-q", "--allow-empty", "-m", "wrong identity"],
        );
        git(
            &path,
            &["config", "--add", "gam.allowedEmail", "octo@example.com"],
        );

        let head = String::from_utf8(
            Command::new("git")
                .args(["-C", &path, "rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let head = head.trim();
        let zero = "0".repeat(40);
        let refs = format!("refs/heads/main {} refs/heads/main {}\n", head, zero);

        let hook = format!("{}/.git/hooks/pre-push", path);
        std::fs::write(&hook, PRE_PUSH_HOOK).unwrap();

        let run = |stdin_text: &str| {
            use std::io::Write as _;
            let mut child = Command::new(sh)
                .arg(&hook)
                .current_dir(&path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("hook must start");
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(stdin_text.as_bytes())
                .unwrap();
            child.wait_with_output().unwrap()
        };

        let refused = run(&refs);
        let stderr = String::from_utf8_lossy(&refused.stderr).to_string();
        assert!(
            !refused.status.success(),
            "a commit by an unlisted address must not push: {}",
            stderr
        );
        assert!(
            stderr.contains("stranger@example.com"),
            "the refusal must name the address: {}",
            stderr
        );
        // Author and committer are the same person here; saying so twice would
        // be noise, so the message appears once.
        assert_eq!(
            stderr.matches("stranger@example.com").count(),
            1,
            "one commit, one refusal: {}",
            stderr
        );

        // The same push, once the address is allowed.
        git(
            &path,
            &[
                "config",
                "--add",
                "gam.allowedEmail",
                "stranger@example.com",
            ],
        );
        let accepted = run(&refs);
        assert!(
            accepted.status.success(),
            "an allowed address must push: {}",
            String::from_utf8_lossy(&accepted.stderr)
        );

        // A repository with no allow-list is not this app's business.
        git(&path, &["config", "--unset-all", "gam.allowedEmail"]);
        assert!(
            run(&refs).status.success(),
            "no allow-list means no opinion"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Clearing a switch has to undo what setting it did. Leaving the guard on
    /// disk or the remote rewritten would keep enforcing a choice the user has
    /// just reversed, and the repository would disagree with its own settings.
    #[test]
    fn clearing_a_switch_puts_back_what_setting_it_changed() {
        for original in [
            "git@github.com:octo/demo.git",
            // Stored rather than rebuilt precisely for this one: a canonical SSH
            // address is not what this repository was cloned with.
            "https://github.com/octo/demo.git",
        ] {
            let dir = std::env::temp_dir().join(format!(
                "gam-undo-{}-{}",
                std::process::id(),
                original.len()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.to_string_lossy().replace('\\', "/");
            Command::new("git")
                .args(["init", "-q", &path])
                .output()
                .unwrap();
            git(&path, &["remote", "add", "origin", original]);

            let profile = profile();
            let mut binding = binding_at(&path, true, true, false);

            apply_binding(&mut binding, &profile).unwrap();
            assert_eq!(
                git::repo_remote_url(&path, "origin").as_deref(),
                Some("git@github-personal:octo/demo.git")
            );
            assert_eq!(binding.original_remote_url.as_deref(), Some(original));
            assert_eq!(hook_state(&path).0, "installed");

            binding.install_hook = false;
            binding.pin_remote_alias = false;
            apply_binding(&mut binding, &profile).unwrap();

            assert_eq!(
                git::repo_remote_url(&path, "origin").as_deref(),
                Some(original),
                "the remote must come back as it was, not as a rebuilt guess"
            );
            assert_eq!(hook_state(&path).0, "missing");

            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// husky owns every file in `.husky/_` and each one runs its namesake from
    /// the parent. Installing into `_` would fight husky for the file and report
    /// a conflict that is not one; the parent slot lets both hooks run.
    #[test]
    fn the_guard_installs_into_huskys_hook_slot_rather_than_its_runners() {
        let dir = std::env::temp_dir().join(format!("gam-husky-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.to_string_lossy().replace('\\', "/");

        Command::new("git")
            .args(["init", "-q", &path])
            .output()
            .unwrap();
        let runners = dir.join(".husky").join("_");
        std::fs::create_dir_all(&runners).unwrap();
        std::fs::write(runners.join("h"), "#!/usr/bin/env sh\n").unwrap();
        // husky's own runner, which must survive.
        std::fs::write(
            runners.join("pre-push"),
            "#!/usr/bin/env sh\n. \"$(dirname \"$0\")/h\"\n",
        )
        .unwrap();
        git(&path, &["config", "core.hooksPath", ".husky/_"]);

        assert_eq!(install_hook(&path).unwrap(), "installed");

        let ours = dir.join(".husky").join("pre-push");
        assert!(ours.exists(), "guard must land in the slot husky calls");
        assert!(std::fs::read_to_string(&ours)
            .unwrap()
            .contains(HOOK_MARKER));
        assert!(
            !std::fs::read_to_string(runners.join("pre-push"))
                .unwrap()
                .contains(HOOK_MARKER),
            "husky's runner must be left alone"
        );
        assert_eq!(hook_state(&path).0, "installed");

        // Releasing the repository has to find the guard where it was put, or it
        // outlives the binding and keeps refusing pushes.
        remove_hook(&path).unwrap();
        assert!(!ours.exists(), "guard must be removed from husky's slot");
        assert!(
            runners.join("pre-push").exists(),
            "husky's own runner is not ours to delete"
        );

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
        git(
            &path,
            &["remote", "add", "origin", "git@github.com:octo/demo.git"],
        );

        let profile = profile();
        let mut binding = RepoBinding {
            path: path.clone(),
            profile_id: profile.id.clone(),
            platform: Platform::Github,
            pin_remote_alias: true,
            install_hook: true,
            extra_allowed_emails: vec!["bot@example.com".to_string()],
            overrides_root: false,
            original_remote_url: None,
        };

        let result = apply_binding(&mut binding, &profile).unwrap();
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
