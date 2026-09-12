//! The identity guard as a machine-wide hook, so no repository carries a file
//! of this app's.
//!
//! Git reads hooks from one directory when `core.hooksPath` is set, and the
//! global config can set it. This app owns a directory of dispatchers: each
//! one runs the guard where the hook has a say (`pre-commit` for the identity
//! about to be recorded, `pre-push` for commits already made) and then hands
//! over to the repository's own hook of the same name, so lefthook, a
//! hand-written hook or a project's `.git/hooks/*` keep running.
//!
//! What the dispatcher cannot reach is a repository whose local config sets
//! `core.hooksPath` itself (husky does, at `npm install`): local wins over
//! global and the guard never runs there. The doctor reports that repository
//! rather than writing into its tracked folders.

use crate::git;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Client-side hooks that a repository's own tooling commonly installs. The
/// high-frequency ones (`reference-transaction`, `post-index-change`,
/// `fsmonitor-watchman`) are left out: a dispatcher there would spawn a shell
/// on every ref update for the benefit of nobody.
pub const HOOK_NAMES: [&str; 14] = [
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "post-rewrite",
    "pre-auto-gc",
];

const GUARD_FILE: &str = "gam-guard.sh";

#[derive(Debug, Clone, Serialize)]
pub struct HooksStatus {
    /// What the global `core.hooksPath` says, if anything.
    pub hooks_path: Option<String>,
    /// That path is this app's dispatcher directory.
    pub ours: bool,
}

fn hooks_dir() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .ok_or("Cannot find data directory")?
        .join("git-account-manager")
        .join("hooks"))
}

fn posix(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Two spellings of one directory: git stores what it was given, and on
/// Windows that may carry backslashes or a trailing slash.
fn same_dir(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(b)
}

pub fn status() -> HooksStatus {
    let hooks_path = git::get_global_config("core.hooksPath");
    let ours = match (&hooks_path, hooks_dir()) {
        (Some(current), Ok(dir)) => same_dir(current, &posix(&dir)),
        _ => false,
    };
    HooksStatus { hooks_path, ours }
}

/// Points the global `core.hooksPath` at the dispatchers, or releases it.
///
/// A `core.hooksPath` that already points somewhere else is the user's, and
/// silently replacing it would switch off whatever they routed through it, so
/// it is left as it is. This runs on every profile switch, so that case is not
/// an error here; `ensure_in_force` is where switching the guard on says so.
pub fn apply(enabled: bool) -> Result<(), String> {
    let dir = hooks_dir()?;
    let ours = posix(&dir);
    let current = git::get_global_config("core.hooksPath");

    if !enabled {
        if current.as_deref().is_some_and(|c| same_dir(c, &ours)) {
            git::unset_global_hooks_path()?;
        }
        return Ok(());
    }

    write_scripts(&dir)?;
    match current {
        Some(_) => Ok(()),
        None => git::set_global_hooks_path(&ours),
    }
}

/// The answer to switching the guard on: either the dispatchers are in force
/// or the reason they are not is named.
pub fn ensure_in_force() -> Result<(), String> {
    match status() {
        HooksStatus { ours: true, .. } => Ok(()),
        HooksStatus {
            hooks_path: Some(existing),
            ..
        } => Err(format!(
            "Global core.hooksPath already points at {}. Remove it or leave the commit guard off.",
            existing
        )),
        HooksStatus {
            hooks_path: None, ..
        } => Err("core.hooksPath was not set".to_string()),
    }
}

/// How the guard reaches one repository: `global` when the dispatcher is in
/// force there, `local-override` when the repository's own `core.hooksPath`
/// bypasses it, `off` when the global setting is not this app's.
pub fn repo_guard_state(dir: &str) -> String {
    if git::repo_config_get_local(dir, "core.hooksPath").is_some() {
        return "local-override".to_string();
    }
    if status().ours {
        "global".to_string()
    } else {
        "off".to_string()
    }
}

/// Writes the guard library and one dispatcher per hook name. Rewritten on
/// every enable so an upgrade carries its script changes without a migration.
pub fn write_scripts(dir: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let guard = dir.join(GUARD_FILE);
    write_executable(&guard, GUARD_SH)?;
    let guard_posix = posix(&guard);
    for name in HOOK_NAMES {
        let body = DISPATCHER
            .replace("{guard}", &guard_posix)
            .replace("{name}", name);
        write_executable(&dir.join(name), &body)?;
    }
    Ok(())
}

fn write_executable(path: &std::path::Path, body: &str) -> Result<(), String> {
    fs::write(path, body).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

const DISPATCHER: &str = r#"#!/bin/sh
# git-account-manager: global hook dispatcher
. "{guard}"
gam_run "{name}" "$@"
"#;

/// The guard itself. Sourced by every dispatcher; `gam_run` decides what the
/// hook name calls for and then chains to the repository's own hook.
const GUARD_SH: &str = r#"#!/bin/sh
# git-account-manager: identity guard
#
# Refuses a commit, or a push carrying commits, whose author or committer
# email this repository does not allow. The list comes from `git config
# --get-all gam.allowedEmail`, which the app supplies through includeIf rules;
# a repository with no list is not this app's business.

gam_allowed() {
	git config --get-all gam.allowedEmail 2>/dev/null
}

# 0 when $1 is in $allowed.
gam_permits() {
	for candidate in $allowed; do
		[ "$1" = "$candidate" ] && return 0
	done
	return 1
}

# The identity git is about to record, before the commit exists.
gam_pre_commit() {
	allowed=$(gam_allowed)
	[ -z "$allowed" ] && return 0
	status=0
	for var in GIT_AUTHOR_IDENT GIT_COMMITTER_IDENT; do
		ident=$(git var "$var" 2>/dev/null)
		[ -z "$ident" ] && continue
		email=${ident#*<}
		email=${email%%>*}
		[ -z "$email" ] && continue
		if ! gam_permits "$email"; then
			echo "git-account-manager: refusing to commit as <$email> - not allowed in this repository" >&2
			status=1
		fi
	done
	if [ "$status" -ne 0 ]; then
		echo "git-account-manager: set this repository's identity in the app, or add the address there, then commit again" >&2
	fi
	return $status
}

# One `git log` for a whole range, not one `git show` per commit: pushing a
# branch of a few hundred commits used to spawn a few hundred processes, and on
# Windows each one is a visible cost. The loop reads from a here-document
# rather than a pipe so that `status` set inside it survives - a piped `while`
# runs in a subshell and its assignments are lost.
gam_check_range() {
	commits=$(git log --format='%h %ae %ce' "$@")
	while read -r short author committer; do
		[ -z "$short" ] && continue
		for email in "$author" "$committer"; do
			[ -z "$email" ] && continue
			if ! gam_permits "$email"; then
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

# $1 is what git wrote to the hook's stdin: one line per ref being pushed.
gam_pre_push() {
	allowed=$(gam_allowed)
	[ -z "$allowed" ] && return 0
	zero=0000000000000000000000000000000000000000
	status=0
	while read -r _local_ref local_sha _remote_ref remote_sha; do
		[ -z "$local_sha" ] && continue
		[ "$local_sha" = "$zero" ] && continue
		if [ "$remote_sha" = "$zero" ]; then
			gam_check_range "$local_sha" --not --remotes
		else
			gam_check_range "$remote_sha..$local_sha"
		fi
	done <<EOF
$1
EOF
	if [ "$status" -ne 0 ]; then
		echo "git-account-manager: fix the commits or add the address in the app, then push again" >&2
	fi
	return $status
}

# Runs the guard for this hook name, then the repository's own hook of the
# same name with the same arguments and the same stdin.
gam_run() {
	name=$1
	shift
	input=""
	case "$name" in
		pre-push|post-rewrite) input=$(cat) ;;
	esac
	case "$name" in
		pre-commit) gam_pre_commit || exit 1 ;;
		pre-push) gam_pre_push "$input" || exit 1 ;;
	esac
	common=$(git rev-parse --git-common-dir 2>/dev/null)
	[ -z "$common" ] && exit 0
	repo_hook="$common/hooks/$name"
	[ -f "$repo_hook" ] || exit 0
	# `$(cat)` dropped the final newline; a hook reading its lines with `read`
	# would skip the last ref without it.
	if [ -n "$input" ]; then
		printf '%s
' "$input" | gam_exec "$repo_hook" "$@"
	else
		gam_exec "$repo_hook" "$@" </dev/null
	fi
	exit $?
}

# Git for Windows marks nothing executable, so a hook without the bit still
# has to run.
gam_exec() {
	hook=$1
	shift
	if [ -x "$hook" ]; then
		"$hook" "$@"
	else
		sh "$hook" "$@"
	fi
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// `sh` is what git runs a hook with. Git for Windows ships one but does not
    /// always put it on PATH, so the guard's behaviour is proven wherever a
    /// shell exists and the test says so out loud where one does not.
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

    fn git(dir: &str, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git must be on PATH");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A fresh repository plus a dispatcher directory beside it, isolated from
    /// the machine's global config so the allow-list is exactly what the test
    /// puts in the repository.
    fn sandbox(tag: &str) -> (PathBuf, String, PathBuf) {
        let dir = std::env::temp_dir().join(format!("gam-hooks-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let repo = dir.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let path = posix(&repo);
        Command::new("git")
            .args(["init", "-q", &path])
            .output()
            .unwrap();
        git(&path, &["config", "commit.gpgsign", "false"]);
        let hooks = dir.join("hooks");
        write_scripts(&hooks).unwrap();
        (dir, path, hooks)
    }

    fn run_hook(
        sh: &str,
        hooks: &std::path::Path,
        name: &str,
        repo: &str,
        stdin_text: &str,
    ) -> std::process::Output {
        use std::io::Write as _;
        let mut child = Command::new(sh)
            .arg(hooks.join(name))
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
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
    }

    /// The commit never exists when the identity is wrong: that is the whole
    /// point of moving the check off pre-push.
    #[test]
    fn pre_commit_refuses_a_disallowed_identity_and_passes_an_allowed_one() {
        let Some(sh) = shell() else {
            eprintln!("no POSIX shell on PATH: hook behaviour not verified here");
            return;
        };
        let (dir, repo, hooks) = sandbox("precommit");
        git(&repo, &["config", "user.name", "Octo"]);
        git(&repo, &["config", "user.email", "stranger@example.com"]);
        git(
            &repo,
            &["config", "--add", "gam.allowedEmail", "octo@example.com"],
        );

        let refused = run_hook(sh, &hooks, "pre-commit", &repo, "");
        let stderr = String::from_utf8_lossy(&refused.stderr).to_string();
        assert!(
            !refused.status.success(),
            "wrong identity must not commit: {}",
            stderr
        );
        assert!(stderr.contains("stranger@example.com"), "{}", stderr);

        git(&repo, &["config", "user.email", "octo@example.com"]);
        let accepted = run_hook(sh, &hooks, "pre-commit", &repo, "");
        assert!(
            accepted.status.success(),
            "allowed identity must commit: {}",
            String::from_utf8_lossy(&accepted.stderr)
        );

        // A repository with no allow-list is not this app's business.
        git(&repo, &["config", "--unset-all", "gam.allowedEmail"]);
        git(&repo, &["config", "user.email", "anyone@example.com"]);
        assert!(run_hook(sh, &hooks, "pre-commit", &repo, "")
            .status
            .success());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Commits made before the repository was bound are caught on the way out.
    #[test]
    fn pre_push_refuses_a_disallowed_address_in_the_range() {
        let Some(sh) = shell() else {
            eprintln!("no POSIX shell on PATH: hook behaviour not verified here");
            return;
        };
        let (dir, repo, hooks) = sandbox("prepush");
        git(&repo, &["config", "user.name", "Octo"]);
        git(&repo, &["config", "user.email", "stranger@example.com"]);
        git(
            &repo,
            &["commit", "-q", "--allow-empty", "-m", "wrong identity"],
        );
        git(
            &repo,
            &["config", "--add", "gam.allowedEmail", "octo@example.com"],
        );

        let head = git(&repo, &["rev-parse", "HEAD"]);
        let zero = "0".repeat(40);
        let refs = format!("refs/heads/main {} refs/heads/main {}\n", head, zero);

        let refused = run_hook(sh, &hooks, "pre-push", &repo, &refs);
        let stderr = String::from_utf8_lossy(&refused.stderr).to_string();
        assert!(!refused.status.success(), "{}", stderr);
        // Author and committer are one person here; one commit, one refusal.
        assert_eq!(
            stderr.matches("stranger@example.com").count(),
            1,
            "{}",
            stderr
        );

        git(
            &repo,
            &[
                "config",
                "--add",
                "gam.allowedEmail",
                "stranger@example.com",
            ],
        );
        assert!(run_hook(sh, &hooks, "pre-push", &repo, &refs)
            .status
            .success());

        let _ = fs::remove_dir_all(&dir);
    }

    /// The dispatcher replaces the repository's hooks directory in git's eyes,
    /// so a hook the project put there has to be called by hand, with the same
    /// stdin git gave us.
    #[test]
    fn the_repositorys_own_hook_still_runs_after_the_guard() {
        let Some(sh) = shell() else {
            eprintln!("no POSIX shell on PATH: hook behaviour not verified here");
            return;
        };
        let (dir, repo, hooks) = sandbox("chain");
        let own = dir.join("repo").join(".git").join("hooks");
        fs::create_dir_all(&own).unwrap();
        fs::write(
            own.join("pre-push"),
            "#!/bin/sh\ncat > ran-with-stdin\necho \"$1\" > ran-with-arg\nexit 3\n",
        )
        .unwrap();

        let out = run_hook(sh, &hooks, "pre-push", &repo, "refs/x 0 refs/x 0\n");
        assert_eq!(
            out.status.code(),
            Some(3),
            "the project's exit code is the answer"
        );
        assert_eq!(
            fs::read_to_string(dir.join("repo").join("ran-with-stdin")).unwrap(),
            "refs/x 0 refs/x 0\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_hook_name_gets_a_dispatcher_that_sources_the_guard() {
        let dir = std::env::temp_dir().join(format!("gam-hooks-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_scripts(&dir).unwrap();
        for name in HOOK_NAMES {
            let body = fs::read_to_string(dir.join(name)).unwrap();
            assert!(body.contains(GUARD_FILE), "{} must source the guard", name);
            assert!(body.contains(&format!("gam_run \"{}\"", name)));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hooks_path_spelled_differently_is_still_ours() {
        assert!(same_dir("C:/Users/a/hooks", "c:\\Users\\a\\hooks\\"));
        assert!(!same_dir("C:/Users/a/hooks", "C:/Users/a/other"));
    }
}
