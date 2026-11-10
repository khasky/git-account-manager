use crate::models::{
    Platform, Profile, SshKeyInfo, SshKeyPair, MANAGED_FOOTER, MANAGED_HEADER, PLATFORMS,
};
use crate::proc::hidden_command;
use crate::repos::host_alias;
use std::fs;
use std::path::PathBuf;

fn ssh_dir() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or("Cannot find home directory")?
        .join(".ssh");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `key_type` is what `ssh-keygen -t` takes. RSA is given 4096 bits rather than
/// the 3072 `ssh-keygen` defaults to; it is only made where a platform accepts
/// nothing else.
pub fn generate_key(email: &str, key_name: &str, key_type: &str) -> Result<SshKeyPair, String> {
    let dir = ssh_dir()?;
    let private_path = dir.join(key_name);
    let public_path = dir.join(format!("{}.pub", key_name));

    if private_path.exists() {
        return Err(format!("Key '{}' already exists", key_name));
    }

    let mut cmd = hidden_command("ssh-keygen");
    cmd.args(["-t", key_type]);
    if key_type == "rsa" {
        cmd.args(["-b", "4096"]);
    }
    cmd.args(["-C", email, "-f"])
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
        signing_error: None,
        manual_upload_url: None,
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

/// The private half of a key in `~/.ssh` whose public half is `public_key`,
/// compared on type and body since the comment is free to differ.
pub fn private_key_for(public_key: &str) -> Option<String> {
    let body = |k: &str| k.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let wanted = body(public_key);
    list_keys().ok()?.into_iter().find_map(|k| {
        let pub_key = read_public_key(&k.public_key_path).ok()?;
        (body(&pub_key) == wanted).then_some(k.private_key_path)
    })
}

/// What a host says to `ssh -T` when it is reached with exactly this key.
///
/// `-F none` keeps the user's own config, this app's region included, from
/// offering a second key the host might answer to instead. A host key seen
/// for the first time is accepted, as the first `git clone` would.
pub fn greeting(user_host: &str, private_key: &str) -> Result<String, String> {
    let output = hidden_command(&crate::openssh_integration::ssh_program())
        .args(["-T", "-F", "none", "-i"])
        .arg(private_key)
        .args([
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=15",
            user_host,
        ])
        .output()
        .map_err(|e| format!("Failed to run ssh: {}", e))?;
    Ok(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

pub fn read_public_key(pub_key_path: &str) -> Result<String, String> {
    fs::read_to_string(pub_key_path)
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}

/// Which profile's key answers on each platform's bare host. `None` is a host
/// the active profile has no account on: a remote addressed as `git@<host>:`
/// is refused there, since no other block matches.
pub fn bare_host_owners(profiles: &[Profile]) -> Vec<(Platform, Option<&Profile>)> {
    let active = profiles.iter().find(|p| p.is_active);
    PLATFORMS
        .iter()
        .map(|&platform| (platform, active.filter(|p| p.account(platform).is_some())))
        .collect()
}

/// Writes the managed region of `~/.ssh/config`.
///
/// The active profile owns the bare `github.com` / `gitlab.com` /
/// `bitbucket.org` / `ssh.dev.azure.com` hosts, so a repository without an alias still has a key;
/// a repository pinned to a `<platform>-<slug>` alias keeps its own key no
/// matter which profile is active, because its block names the key directly
/// with `IdentitiesOnly`.
pub fn update_ssh_config(profiles: &[Profile]) -> Result<(), String> {
    let dir = ssh_dir()?;
    let config_path = dir.join("config");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();

    let unmanaged = strip_all_managed(&existing);

    let mut entries: Vec<String> = Vec::new();

    for (platform, owner) in bare_host_owners(profiles) {
        if let Some(account) = owner.and_then(|p| p.account(platform)) {
            let host = platform.canonical_host();
            entries.push(host_entry(host, host, &account.ssh_private_key_path));
        }
    }

    for profile in profiles {
        for platform in PLATFORMS {
            if let Some(account) = profile.account(platform) {
                entries.push(host_entry(
                    &host_alias(platform, profile),
                    platform.canonical_host(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlatformAccount;

    fn account() -> PlatformAccount {
        PlatformAccount {
            username: "octo".to_string(),
            git_name: "Octo".to_string(),
            git_email: "octo@example.com".to_string(),
            ssh_private_key_path: String::new(),
            ssh_public_key_path: String::new(),
            sign_commits: false,
            token: None,
        }
    }

    fn profile(name: &str, is_active: bool, github: bool) -> Profile {
        Profile {
            id: name.to_string(),
            name: name.to_string(),
            default_platform: None,
            github: github.then(account),
            gitlab: Some(account()),
            bitbucket: None,
            azure: None,
            codeberg: None,
            huggingface: None,
            is_active,
        }
    }

    /// The failure this guards against: a repository whose origin is the bare
    /// `git@github.com:` address with no `Host github.com` block to answer it,
    /// which ssh reports as "Permission denied (publickey)".
    #[test]
    fn the_active_profile_owns_every_bare_host_it_has_an_account_on() {
        let profiles = vec![profile("work", false, true), profile("home", true, true)];
        let owners = bare_host_owners(&profiles);
        let owner_of = |wanted: Platform| {
            owners
                .iter()
                .find(|(p, _)| *p == wanted)
                .and_then(|(_, o)| o.map(|p| p.name.as_str()))
        };
        assert_eq!(owner_of(Platform::Github), Some("home"));
        assert_eq!(owner_of(Platform::Gitlab), Some("home"));
        assert_eq!(owner_of(Platform::Bitbucket), None);
    }

    #[test]
    fn a_host_the_active_profile_lacks_has_no_owner() {
        let profiles = vec![profile("work", false, true), profile("home", true, false)];
        let github = bare_host_owners(&profiles)
            .into_iter()
            .find(|(p, _)| *p == Platform::Github)
            .and_then(|(_, o)| o);
        assert!(github.is_none());
    }

    #[test]
    fn no_active_profile_leaves_every_host_unowned() {
        let profiles = vec![profile("work", false, true)];
        assert!(bare_host_owners(&profiles).iter().all(|(_, o)| o.is_none()));
    }
}
