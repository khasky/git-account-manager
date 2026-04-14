//! Windows: align TortoiseGit and Git CLI with OpenSSH so `~/.ssh/config` (managed by this app) is honored.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSshIntegrationProbe {
    /// Integration is meaningful (currently: Windows only).
    pub available: bool,
    /// First `ssh.exe` found on this system, if any.
    pub ssh_exe: Option<String>,
}

pub fn probe() -> OpenSshIntegrationProbe {
    #[cfg(windows)]
    {
        OpenSshIntegrationProbe {
            available: true,
            ssh_exe: detect_ssh_exe().map(|p| p.to_string_lossy().to_string()),
        }
    }
    #[cfg(not(windows))]
    {
        OpenSshIntegrationProbe {
            available: false,
            ssh_exe: None,
        }
    }
}

pub fn ensure_ssh_available() -> Result<(), String> {
    #[cfg(windows)]
    {
        detect_ssh_exe().ok_or_else(|| {
            "Could not find ssh.exe. Install Git for Windows or enable the OpenSSH optional feature in Windows Settings.".to_string()
        })?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

pub fn apply(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if enabled {
            let path = detect_ssh_exe().ok_or_else(|| "Could not find ssh.exe.".to_string())?;
            let path_str = path.to_string_lossy().to_string();
            write_tortoise_git_ssh(&path_str)?;
            crate::git::set_global_ssh_command(&path_str)?;
        } else {
            clear_tortoise_git_ssh();
            let _ = crate::git::unset_global_ssh_command();
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[cfg(windows)]
fn detect_ssh_exe() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(root) = std::env::var("SystemRoot") {
        candidates.push(
            PathBuf::from(root)
                .join("System32")
                .join("OpenSSH")
                .join("ssh.exe"),
        );
    }
    candidates.push(PathBuf::from(r"C:\Windows\System32\OpenSSH\ssh.exe"));

    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(
            PathBuf::from(pf)
                .join("Git")
                .join("usr")
                .join("bin")
                .join("ssh.exe"),
        );
    }
    candidates.push(PathBuf::from(
        r"C:\Program Files\Git\usr\bin\ssh.exe",
    ));

    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        candidates.push(
            PathBuf::from(pf86)
                .join("Git")
                .join("usr")
                .join("bin")
                .join("ssh.exe"),
        );
    }

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("Git")
                .join("usr")
                .join("bin")
                .join("ssh.exe"),
        );
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    find_ssh_via_where()
}

#[cfg(windows)]
fn find_ssh_via_where() -> Option<std::path::PathBuf> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "where", "ssh"]);
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|l| !l.trim().is_empty())?
        .trim()
        .to_string();

    let p = std::path::PathBuf::from(line);
    if p.is_file() { Some(p) } else { None }
}

#[cfg(windows)]
fn write_tortoise_git_ssh(path_str: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(r"Software\TortoiseGit").map_err(|e| {
        format!(
            "Could not open or create HKCU\\Software\\TortoiseGit: {}",
            e
        )
    })?;
    key.set_value("SSH", &path_str)
        .map_err(|e| format!("Could not set TortoiseGit SSH client path: {}", e))
}

#[cfg(windows)]
fn clear_tortoise_git_ssh() {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(r"Software\TortoiseGit", KEY_SET_VALUE) {
        let _ = key.delete_value("SSH");
    }
}
