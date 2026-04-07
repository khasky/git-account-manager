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
