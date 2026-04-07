use crate::models::{Profile, SshKeyInfo, SshKeyPair};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn ssh_dir() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".ssh");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn generate_key(email: &str, key_name: &str) -> Result<SshKeyPair, String> {
    let dir = ssh_dir()?;
    let private_path = dir.join(key_name);
    let public_path = dir.join(format!("{}.pub", key_name));

    if private_path.exists() {
        return Err(format!("Key '{}' already exists", key_name));
    }

    let mut cmd = Command::new("ssh-keygen");
    cmd.args(["-t", "ed25519", "-C", email, "-f"])
        .arg(&private_path)
        .args(["-N", ""]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run ssh-keygen: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ssh-keygen failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(SshKeyPair {
        private_key_path: private_path.to_string_lossy().to_string(),
        public_key_path: public_path.to_string_lossy().to_string(),
    })
}


pub fn list_keys() -> Result<Vec<SshKeyInfo>, String> {
    let dir = ssh_dir()?;
    let mut keys = vec![];

    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.extension().map(|e| e == "pub").unwrap_or(false) {
            let priv_path = path.with_extension("");
            if priv_path.exists() {
                let name = priv_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                keys.push(SshKeyInfo {
                    name,
                    private_key_path: priv_path.to_string_lossy().to_string(),
                    public_key_path: path.to_string_lossy().to_string(),
                });
            }
        }
    }

    Ok(keys)
}

pub fn clean_known_hosts(hostnames: &[&str]) -> Result<(), String> {
    let dir = ssh_dir()?;
    let path = dir.join("known_hosts");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() { return true; }
            !hostnames.iter().any(|h| trimmed.starts_with(h))
        })
        .collect();

    let result = filtered.join("\n");
    fs::write(&path, if result.ends_with('\n') { result } else { result + "\n" })
        .map_err(|e| e.to_string())
}

pub fn delete_key_pair(private_key_path: &str) -> Result<(), String> {
    let priv_path = std::path::Path::new(private_key_path);
    let pub_path = priv_path.with_extension("pub");
    if priv_path.exists() {
        fs::remove_file(priv_path)
            .map_err(|e| format!("Failed to delete private key: {}", e))?;
    }
    if pub_path.exists() {
        fs::remove_file(pub_path)
            .map_err(|e| format!("Failed to delete public key: {}", e))?;
    }
    Ok(())
}

pub fn read_public_key(pub_key_path: &str) -> Result<String, String> {
    fs::read_to_string(pub_key_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}

const MANAGED_HEADER: &str = "# === begin git-account-manager ===";
const MANAGED_FOOTER: &str = "# === end git-account-manager ===";

pub fn update_ssh_config(profiles: &[Profile]) -> Result<(), String> {
    let dir = ssh_dir()?;
    let config_path = dir.join("config");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();

    let unmanaged = strip_all_managed(&existing);

    let mut entries: Vec<String> = Vec::new();

    let active = profiles.iter().find(|p| p.is_active);

    if let Some(profile) = active {
        if let Some(gh) = &profile.github {
            entries.push(host_entry("github.com", "github.com", &gh.ssh_private_key_path));
        }
        if let Some(gl) = &profile.gitlab {
            entries.push(host_entry("gitlab.com", "gitlab.com", &gl.ssh_private_key_path));
        }
    }

    for profile in profiles {
        let slug = profile.name.to_lowercase().replace(' ', "-");
        if let Some(gh) = &profile.github {
            entries.push(host_entry(&format!("github-{}", slug), "github.com", &gh.ssh_private_key_path));
        }
        if let Some(gl) = &profile.gitlab {
            entries.push(host_entry(&format!("gitlab-{}", slug), "gitlab.com", &gl.ssh_private_key_path));
        }
    }

    let mut result = String::new();
    let clean = unmanaged.trim();
    if !clean.is_empty() {
        result.push_str(clean);
        result.push('\n');
    }

    if !entries.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(MANAGED_HEADER);
        result.push_str("\n\n");
        result.push_str(&entries.join("\n"));
        result.push('\n');
        result.push_str(MANAGED_FOOTER);
        result.push('\n');
    }

    fs::write(&config_path, &result).map_err(|e| e.to_string())
}

fn strip_all_managed(config: &str) -> String {
    let headers: &[&str] = &[MANAGED_HEADER, "# === git-account-manager managed ==="];
    let mut result = config.to_string();

    loop {
        let header_pos = headers.iter().filter_map(|h| result.find(h).map(|p| (p, *h))).min_by_key(|(p, _)| *p);
        let footer_pos = result.find(MANAGED_FOOTER);

        match (header_pos, footer_pos) {
            (Some((h, _)), Some(f)) if h <= f => {
                let footer_end = f + MANAGED_FOOTER.len();
                let after_start = result[footer_end..]
                    .find('\n')
                    .map(|n| footer_end + n + 1)
                    .unwrap_or(result.len());
                result = format!("{}{}", &result[..h], &result[after_start..]);
            }
            _ => break,
        }
    }

    result
}

fn host_entry(host: &str, hostname: &str, identity_file: &str) -> String {
    let identity = identity_file.replace('\\', "/");
    format!(
        "Host {}\n  HostName {}\n  User git\n  IdentityFile {}\n  IdentitiesOnly yes\n",
        host, hostname, identity
    )
}
