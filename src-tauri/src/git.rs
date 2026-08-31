use crate::proc::hidden_command;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

pub fn set_global_identity(name: &str, email: &str) -> Result<(), String> {
    run_git(&["config", "--global", "user.name", name])?;
    run_git(&["config", "--global", "user.email", email])?;
    Ok(())
}

/// The keys that turn SSH commit signing on, and the values they need.
///
/// `gpg.format = ssh` is what makes `user.signingkey` a path to a public key
/// rather than a GPG key id; without it Git looks the value up in a keyring and
/// fails. Tags are signed alongside commits because a platform verifies both,
/// and a repository that signs only half of its objects is harder to reason
/// about than one that signs neither.
fn signing_values(pub_key_path: &str) -> [(&'static str, String); 4] {
    [
        ("gpg.format", "ssh".to_string()),
        ("user.signingkey", pub_key_path.replace('\\', "/")),
        ("commit.gpgsign", "true".to_string()),
        ("tag.gpgsign", "true".to_string()),
    ]
}

/// Keys removed when signing is switched off. Leaving `commit.gpgsign = true`
/// behind would keep signing with a key the profile no longer names.
const SIGNING_KEYS: [&str; 4] = [
    "gpg.format",
    "user.signingkey",
    "commit.gpgsign",
    "tag.gpgsign",
];

/// Turns signing on for one repository, or off when `pub_key_path` is `None`.
pub fn set_repo_signing(dir: &str, pub_key_path: Option<&str>) -> Result<(), String> {
    match pub_key_path {
        Some(path) => {
            for (key, value) in signing_values(path) {
                repo_config_set_local(dir, key, &value)?;
            }
            Ok(())
        }
        None => {
            for key in SIGNING_KEYS {
                run_git_optional(&["-C", dir, "config", "--local", "--unset-all", key])?;
            }
            Ok(())
        }
    }
}

/// The machine-wide half of the same switch, for the active profile.
pub fn set_global_signing(pub_key_path: Option<&str>) -> Result<(), String> {
    match pub_key_path {
        Some(path) => {
            for (key, value) in signing_values(path) {
                run_git(&["config", "--global", key, &value])?;
            }
            Ok(())
        }
        None => {
            for key in SIGNING_KEYS {
                run_git_optional(&["config", "--global", "--unset-all", key])?;
            }
            Ok(())
        }
    }
}

pub fn get_global_identity() -> Result<GitIdentity, String> {
    let name = run_git(&["config", "--global", "user.name"]).unwrap_or_default();
    let email = run_git(&["config", "--global", "user.email"]).unwrap_or_default();
    Ok(GitIdentity { name, email })
}

/// Drops the machine-wide identity. Without it Git cannot silently sign a commit
/// with whichever profile was activated last: it fails and asks for one.
pub fn unset_global_identity() -> Result<(), String> {
    run_git_optional(&["config", "--global", "--unset-all", "user.name"])?;
    run_git_optional(&["config", "--global", "--unset-all", "user.email"])?;
    Ok(())
}

/// `user.useConfigOnly` stops Git from inventing an identity from the machine's
/// hostname when none is configured.
pub fn set_use_config_only(enabled: bool) -> Result<(), String> {
    if enabled {
        run_git(&["config", "--global", "user.useConfigOnly", "true"]).map(|_| ())
    } else {
        run_git_optional(&["config", "--global", "--unset-all", "user.useConfigOnly"])
    }
}

pub fn get_global_config(key: &str) -> Option<String> {
    run_git(&["config", "--global", "--get", key])
        .ok()
        .filter(|v| !v.is_empty())
}

/// Sets Git's global `core.sshCommand` so CLI Git uses the same OpenSSH as
/// TortoiseGit when configured.
///
/// Windows-only, like the integration that calls it: everywhere else Git
/// already uses the system `ssh` and there is nothing to point it at.
#[cfg(windows)]
pub fn set_global_ssh_command(ssh_exe: &str) -> Result<(), String> {
    let normalized = ssh_exe.replace('\\', "/");
    run_git(&["config", "--global", "core.sshCommand", normalized.as_str()]).map(|_| ())
}

/// Removes `core.sshCommand` if present (ignores "not set").
#[cfg(windows)]
pub fn unset_global_ssh_command() -> Result<(), String> {
    run_git_optional(&["config", "--global", "--unset", "core.sshCommand"])
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn repo_config_get(dir: &str, key: &str) -> Option<String> {
    run_git(&["-C", dir, "config", "--get", key])
        .ok()
        .filter(|v| !v.is_empty())
}

pub fn repo_config_get_local(dir: &str, key: &str) -> Option<String> {
    run_git(&["-C", dir, "config", "--local", "--get", key])
        .ok()
        .filter(|v| !v.is_empty())
}

pub fn repo_config_set_local(dir: &str, key: &str, value: &str) -> Result<(), String> {
    run_git(&["-C", dir, "config", "--local", key, value]).map(|_| ())
}

/// Replaces every value of a multi-valued local key with `values`.
pub fn repo_config_replace_all(dir: &str, key: &str, values: &[String]) -> Result<(), String> {
    run_git_optional(&["-C", dir, "config", "--local", "--unset-all", key])?;
    for value in values {
        run_git(&["-C", dir, "config", "--local", "--add", key, value])?;
    }
    Ok(())
}

pub fn repo_identity(dir: &str) -> GitIdentity {
    GitIdentity {
        name: repo_config_get(dir, "user.name").unwrap_or_default(),
        email: repo_config_get(dir, "user.email").unwrap_or_default(),
    }
}

pub fn set_repo_identity(dir: &str, name: &str, email: &str) -> Result<(), String> {
    repo_config_set_local(dir, "user.name", name)?;
    repo_config_set_local(dir, "user.email", email)
}

pub fn repo_remote_url(dir: &str, remote: &str) -> Option<String> {
    run_git(&["-C", dir, "remote", "get-url", remote])
        .ok()
        .filter(|v| !v.is_empty())
}

pub fn set_repo_remote_url(dir: &str, remote: &str, url: &str) -> Result<(), String> {
    run_git(&["-C", dir, "remote", "set-url", remote, url]).map(|_| ())
}

/// `-F none` is what makes the answer about this one key. `IdentitiesOnly` drops
/// the agent and the default filenames but still offers every `IdentityFile`
/// `~/.ssh/config` declares — including the `Host github.com` block this app
/// points at the active profile, which authenticates and makes any key look like
/// it reaches anything. Skipping the config also skips a hand-written
/// `ProxyCommand` or `Port` for that host, which is the lesser problem: a probe
/// that quietly borrows the active profile's key answers a different question.
fn ssh_command_for_key(key_path: &str) -> String {
    // Git splits GIT_SSH_COMMAND with shell quoting rules, where a backslash
    // escapes rather than separates; ~/.ssh/config is written the same way.
    let key = key_path.replace('\\', "/");
    format!(
        "ssh -F none -i \"{}\" -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new",
        key
    )
}

/// Asks the host for a repository's refs over SSH using one key and no other.
/// On failure the error is Git's own first line — "Repository not found",
/// "Permission denied (publickey)" — which says more than a rephrasing would.
pub fn ls_remote_with_key(url: &str, key_path: &str) -> Result<(), String> {
    let mut cmd = git_command(&["ls-remote", "--heads", url]);
    cmd.env("GIT_SSH_COMMAND", ssh_command_for_key(key_path));
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("git ls-remote failed")
        .to_string())
}

/// Distinct author and committer emails across the most recent `limit` commits.
/// An empty repository has no log and yields an empty list.
pub fn repo_recent_identities(dir: &str, limit: usize) -> Vec<String> {
    let count = format!("-{}", limit);
    let Ok(out) = run_git(&["-C", dir, "log", &count, "--format=%ae%n%ce"]) else {
        return Vec::new();
    };
    let mut seen: Vec<String> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    seen.sort();
    seen.dedup();
    seen
}

/// Absolute path of the directory Git looks in for this repository's hooks.
/// Honours `core.hooksPath` (husky and friends point it elsewhere).
pub fn repo_hooks_dir(dir: &str) -> Option<std::path::PathBuf> {
    let toplevel = run_git(&["-C", dir, "rev-parse", "--show-toplevel"]).ok()?;
    let root = Path::new(toplevel.trim());
    match repo_config_get(dir, "core.hooksPath") {
        Some(custom) => {
            let p = Path::new(&custom);
            Some(if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            })
        }
        None => {
            let git_dir = run_git(&["-C", dir, "rev-parse", "--absolute-git-dir"]).ok()?;
            Some(Path::new(git_dir.trim()).join("hooks"))
        }
    }
}

fn git_command(args: &[&str]) -> Command {
    let mut cmd = hidden_command("git");
    cmd.args(args);
    cmd
}

fn run_git(args: &[&str]) -> Result<String, String> {
    let output = git_command(args)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Like `run_git`, but treats "the key was not there" as success — the state the
/// caller wanted is already in place.
fn run_git_optional(args: &[&str]) -> Result<(), String> {
    let output = git_command(args)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not find key") || stderr.contains("not unset") {
        return Ok(());
    }

    // Git exits with code 5 when the key does not exist.
    if output.status.code() == Some(5) {
        return Ok(());
    }

    Err(stderr.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without `-F none` the `Host github.com` block this app writes supplies a
    /// second IdentityFile, it authenticates, and the probe reports success for a
    /// key that reaches nothing. Verified against github.com: an unregistered key
    /// passes with the flag missing and gets `Permission denied (publickey)` with
    /// it present.
    #[test]
    fn the_probe_offers_one_key_and_ignores_ssh_config() {
        let cmd = ssh_command_for_key(r"C:\Users\a\.ssh\id_ed25519");

        assert!(cmd.contains(" -F none "), "{}", cmd);
        assert!(
            cmd.contains(r#"-i "C:/Users/a/.ssh/id_ed25519""#),
            "{}",
            cmd
        );
        assert!(cmd.contains("IdentitiesOnly=yes"), "{}", cmd);
        // No prompt may block a probe running inside the app.
        assert!(cmd.contains("BatchMode=yes"), "{}", cmd);
    }
}
