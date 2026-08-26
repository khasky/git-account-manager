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

/// Sets Git's global `core.sshCommand` so CLI Git uses the same OpenSSH as TortoiseGit when configured.
pub fn set_global_ssh_command(ssh_exe: &str) -> Result<(), String> {
    let normalized = ssh_exe.replace('\\', "/");
    run_git(&["config", "--global", "core.sshCommand", normalized.as_str()]).map(|_| ())
}

/// Removes `core.sshCommand` if present (ignores "not set").
pub fn unset_global_ssh_command() -> Result<(), String> {
    run_git_optional(&["config", "--global", "--unset", "core.sshCommand"])
}

// ---- Repository-scoped helpers ----

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

// ---- Process plumbing ----

fn git_command(args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
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
