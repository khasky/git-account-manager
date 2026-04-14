use serde::{Deserialize, Serialize};
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

/// Sets Git's global `core.sshCommand` so CLI Git uses the same OpenSSH as TortoiseGit when configured.
pub fn set_global_ssh_command(ssh_exe: &str) -> Result<(), String> {
    let normalized = ssh_exe.replace('\\', "/");
    run_git(&["config", "--global", "core.sshCommand", normalized.as_str()]).map(|_| ())
}

/// Removes `core.sshCommand` if present (ignores "not set").
pub fn unset_global_ssh_command() -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.args(["config", "--global", "--unset", "core.sshCommand"]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("could not find key") || stderr.contains("not unset") {
        return Ok(());
    }

    // Git often exits with code 5 when the key does not exist
    if output.status.code() == Some(5) {
        return Ok(());
    }

    Err(stderr.trim().to_string())
}

fn run_git(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
