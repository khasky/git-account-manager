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

/// Drops the machine-wide identity, so Git cannot sign work with whichever
/// profile was activated last: outside a claimed folder it asks for one.
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

/// The global `core.hooksPath`, which routes every repository's hooks through
/// this app's dispatchers (`hooks.rs`).
pub fn set_global_hooks_path(dir: &str) -> Result<(), String> {
    run_git(&["config", "--global", "core.hooksPath", dir]).map(|_| ())
}

pub fn unset_global_hooks_path() -> Result<(), String> {
    run_git_optional(&["config", "--global", "--unset", "core.hooksPath"])
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

/// Where git looks for the helper that answers for one host. Scoped to the
/// host rather than set globally, so a credential manager the user already runs
/// keeps answering for every host this app has no account on.
fn credential_helper_key(host: &str) -> String {
    format!("credential.https://{}.helper", host)
}

/// Git runs a helper value beginning with `!` through a shell, and that is the
/// only form a path with spaces survives in, once quoted.
fn helper_command(helper_path: &str) -> String {
    format!("!\"{}\" credential", helper_path.replace('\\', "/"))
}

/// Points git at this app's helper for one host.
///
/// The empty value written first is git's own way to reset the helper list for
/// a context: helpers are consulted in config order, so without it a globally
/// configured manager answers before this one and the active profile never
/// reaches the wire.
pub fn set_credential_helper(host: &str, helper_path: &str) -> Result<(), String> {
    let key = credential_helper_key(host);
    unset_credential_helper(host)?;
    run_git(&["config", "--global", "--add", &key, ""])?;
    run_git(&[
        "config",
        "--global",
        "--add",
        &key,
        &helper_command(helper_path),
    ])?;
    Ok(())
}

/// Removes every helper entry this app wrote for one host, which hands the host
/// back to whatever was answering for it before.
pub fn unset_credential_helper(host: &str) -> Result<(), String> {
    run_git_optional(&[
        "config",
        "--global",
        "--unset-all",
        &credential_helper_key(host),
    ])
}

pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

pub fn repo_config_get_local(dir: &str, key: &str) -> Option<String> {
    run_git(&["-C", dir, "config", "--local", "--get", key])
        .ok()
        .filter(|v| !v.is_empty())
}

/// The value git resolves for a key in this repository, and the file it came
/// from. Which file is the whole question for a folder rule: the same address
/// read out of the repository's own config instead of the generated identity
/// file is an override waiting to outlive a change of profile.
pub fn repo_config_origin(dir: &str, key: &str) -> Option<(String, String)> {
    let line = run_git(&["-C", dir, "config", "--show-origin", "--get", key]).ok()?;
    let (origin, value) = line.split_once('\t')?;
    let origin = origin.strip_prefix("file:").unwrap_or(origin);
    Some((origin.replace('\\', "/"), value.to_string()))
}

/// Removes keys from one repository's own config, ignoring the ones that were
/// not there.
pub fn repo_config_unset_local(dir: &str, keys: &[&str]) -> Result<(), String> {
    for key in keys {
        run_git_optional(&["-C", dir, "config", "--local", "--unset-all", key])?;
    }
    Ok(())
}

/// Only a test writes into a repository's own config now: the folder rule is
/// what supplies these keys, and what a test needs is a repository that
/// disagrees with it.
#[cfg(test)]
pub fn repo_config_set_local(dir: &str, key: &str, value: &str) -> Result<(), String> {
    run_git(&["-C", dir, "config", "--local", key, value]).map(|_| ())
}

pub fn repo_remote_url(dir: &str, remote: &str) -> Option<String> {
    run_git(&["-C", dir, "remote", "get-url", remote])
        .ok()
        .filter(|v| !v.is_empty())
}

/// Where this repository keeps its own hooks.
///
/// Only a `core.hooksPath` the repository itself sets is followed: the global
/// one belongs to this app's dispatchers, and the question this answers is
/// where an older version put a file that now has to come back out.
pub fn repo_hooks_dir(dir: &str) -> Option<std::path::PathBuf> {
    let toplevel = run_git(&["-C", dir, "rev-parse", "--show-toplevel"]).ok()?;
    let root = Path::new(toplevel.trim());
    match repo_config_get_local(dir, "core.hooksPath") {
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

    #[test]
    fn a_helper_path_with_spaces_stays_one_argument_for_the_shell() {
        assert_eq!(
            helper_command(r"C:\Program Files\Git Account Manager\gam.exe"),
            "!\"C:/Program Files/Git Account Manager/gam.exe\" credential"
        );
    }

    #[test]
    fn the_helper_key_names_one_host_and_not_the_whole_config() {
        assert_eq!(
            credential_helper_key("github.com"),
            "credential.https://github.com.helper"
        );
    }
}
