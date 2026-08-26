use crate::models::{Profile, SshKeyInfo, SshKeyPair, MANAGED_FOOTER, MANAGED_HEADER, PLATFORMS};
use crate::proc::hidden_command;
use crate::repos::canonical_host;
use std::fs;
use std::path::PathBuf;

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

    let mut cmd = hidden_command("ssh-keygen");
    cmd.args(["-t", "ed25519", "-C", email, "-f"])
        .arg(&private_path)
        .args(["-N", ""]);
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

/// Drops a host's entries from `~/.ssh/known_hosts`.
///
/// `ssh-keygen -R` is the only thing that also matches hashed entries, and
/// `HashKnownHosts yes` is the default in most builds — a text filter reading
/// the file sees opaque hashes and silently removes nothing. It writes its own
/// `.old` backup. Failures are ignored: this runs while deleting a profile and
/// a leftover host key never blocks anything.
pub fn clean_known_hosts(hostnames: &[&str]) {
    for host in hostnames {
        let _ = hidden_command("ssh-keygen")
            .args(["-q", "-R", host])
            .output();
    }
}

pub fn delete_key_pair(private_key_path: &str) -> Result<(), String> {
    let priv_path = std::path::Path::new(private_key_path);
    let pub_path = priv_path.with_extension("pub");
    if priv_path.exists() {
        fs::remove_file(priv_path).map_err(|e| format!("Failed to delete private key: {}", e))?;
    }
    if pub_path.exists() {
        fs::remove_file(pub_path).map_err(|e| format!("Failed to delete public key: {}", e))?;
    }
    Ok(())
}

pub fn read_public_key(pub_key_path: &str) -> Result<String, String> {
    fs::read_to_string(pub_key_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}

/// Asks a host who it thinks we are. GitHub and GitLab answer `Hi <user>!` and
/// then close the connection with a non-zero exit, so the greeting — not the
/// exit code — is the result. This is what proves a host alias really reaches
/// the account it is named after.
pub fn probe_host(host: &str) -> Result<String, String> {
    let target = format!("git@{}", host);
    let mut cmd = hidden_command("ssh");
    cmd.args([
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "StrictHostKeyChecking=accept-new",
        &target,
    ]);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run ssh: {}", e))?;

    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr);
    }

    if text.is_empty() {
        return Err(format!("No response from {}", host));
    }
    Ok(text)
}

/// Writes the managed region of `~/.ssh/config`.
///
/// With `own_bare_hosts` the active profile also claims the bare `github.com` /
/// `gitlab.com` / `bitbucket.org` hosts, which is convenient but means every
/// repository without an alias follows whichever profile is active. Turn it off
/// once repositories are pinned to `<platform>-<slug>` aliases: the key then
/// depends on the repository, not on the app's current state.
pub fn update_ssh_config(profiles: &[Profile], own_bare_hosts: bool) -> Result<(), String> {
    let dir = ssh_dir()?;
    let config_path = dir.join("config");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();

    let unmanaged = strip_all_managed(&existing);

    let mut entries: Vec<String> = Vec::new();

    let active = profiles.iter().find(|p| p.is_active);

    if let (true, Some(profile)) = (own_bare_hosts, active) {
        for platform in PLATFORMS {
            if let Some(account) = profile.account(platform) {
                let host = canonical_host(platform);
                entries.push(host_entry(host, host, &account.ssh_private_key_path));
            }
        }
    }

    for profile in profiles {
        let slug = profile.slug();
        for platform in PLATFORMS {
            if let Some(account) = profile.account(platform) {
                entries.push(host_entry(
                    &format!("{}-{}", platform, slug),
                    canonical_host(platform),
                    &account.ssh_private_key_path,
                ));
            }
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

    if result == existing {
        return Ok(());
    }

    // One backup, taken the first time the app rewrites the file — the same
    // safety net `~/.gitconfig` gets, and this file can hold hand-written Host
    // blocks that only exist here.
    if !existing.is_empty() {
        let backup = config_path.with_file_name("config.gam-backup");
        if !backup.exists() {
            fs::write(&backup, &existing).map_err(|e| e.to_string())?;
        }
    }

    fs::write(&config_path, &result).map_err(|e| e.to_string())
}

fn strip_all_managed(config: &str) -> String {
    let headers: &[&str] = &[MANAGED_HEADER, "# === git-account-manager managed ==="];
    let mut result = config.to_string();

    loop {
        let header_pos = headers
            .iter()
            .filter_map(|h| result.find(h).map(|p| (p, *h)))
            .min_by_key(|(p, _)| *p);
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
