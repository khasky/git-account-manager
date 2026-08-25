use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAccount {
    pub username: String,
    pub git_name: String,
    pub git_email: String,
    pub ssh_private_key_path: String,
    pub ssh_public_key_path: String,
    #[serde(default, skip_serializing)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default_platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<PlatformAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gitlab: Option<PlatformAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitbucket: Option<PlatformAccount>,
    pub is_active: bool,
}

impl Profile {
    pub fn account(&self, platform: &str) -> Option<&PlatformAccount> {
        match platform {
            "github" => self.github.as_ref(),
            "gitlab" => self.gitlab.as_ref(),
            "bitbucket" => self.bitbucket.as_ref(),
            _ => None,
        }
    }

    /// The profile name as it appears in SSH host aliases (`github-<slug>`).
    pub fn slug(&self) -> String {
        self.name.to_lowercase().replace(' ', "-")
    }

    pub fn active_identity(&self) -> Option<(&str, &str)> {
        let platform = self.default_platform.as_deref();
        match platform {
            Some("github") => self.github.as_ref(),
            Some("gitlab") => self.gitlab.as_ref(),
            Some("bitbucket") => self.bitbucket.as_ref(),
            _ => self
                .github
                .as_ref()
                .or(self.gitlab.as_ref())
                .or(self.bitbucket.as_ref()),
        }
        .map(|a| (a.git_name.as_str(), a.git_email.as_str()))
    }
}

/// A folder that holds repositories belonging to one profile. Drives both the
/// scan suggestions and the generated `includeIf "gitdir:"` blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRoot {
    pub path: String,
    pub profile_id: String,
    pub platform: String,
}

/// One repository pinned to a profile. The identity is written to the
/// repository's own config, which is the only place every Git client — CLI,
/// libgit2/TortoiseGit, IDEs — agrees on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoBinding {
    pub path: String,
    pub profile_id: String,
    pub platform: String,
    /// Rewrite `origin` to the profile's `<platform>-<slug>` SSH alias so the
    /// key no longer depends on which profile is active.
    #[serde(default)]
    pub pin_remote_alias: bool,
    /// Install the pre-push identity guard into this repository's hooks path.
    #[serde(default)]
    pub install_hook: bool,
    /// Extra emails the pre-push guard accepts here (bots, co-authors).
    #[serde(default)]
    pub extra_allowed_emails: Vec<String>,
}

/// Machine-wide guard rails. All default to off so an existing install keeps
/// its current behaviour until the user opts in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardSettings {
    /// Remove global `user.name`/`user.email` and set `user.useConfigOnly`, so a
    /// repository without its own identity fails loudly instead of borrowing
    /// whichever profile happens to be active.
    #[serde(default)]
    pub unset_global_identity: bool,
    /// Maintain a generated `includeIf` region in `~/.gitconfig` so fresh clones
    /// under a known root start with the right identity.
    #[serde(default)]
    pub manage_gitconfig_includes: bool,
    /// Let the active profile own the bare `github.com` / `gitlab.com` /
    /// `bitbucket.org` SSH hosts. Turn off once repositories use aliases.
    #[serde(default = "default_true")]
    pub own_bare_ssh_hosts: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GuardSettings {
    fn default() -> Self {
        Self {
            unset_global_identity: false,
            manage_gitconfig_includes: false,
            own_bare_ssh_hosts: true,
        }
    }
}

fn default_github_client_id() -> String {
    "Ov23limWr3GZUp4WQ5If".to_string()
}
fn default_gitlab_client_id() -> String {
    "27a9b268a5c3c040969c2eb9b2bb9fdde051336f144601a1177e9a50be17dc5e".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSettings {
    #[serde(default = "default_github_client_id")]
    pub github_client_id: String,
    #[serde(default = "default_gitlab_client_id")]
    pub gitlab_client_id: String,
    /// When true (Windows only), write TortoiseGit SSH client registry value and set Git `core.sshCommand` to OpenSSH so `~/.ssh/config` applies everywhere.
    #[serde(default)]
    pub use_openssh_for_git_tools: bool,
}

impl Default for OAuthSettings {
    fn default() -> Self {
        Self {
            github_client_id: default_github_client_id(),
            gitlab_client_id: default_gitlab_client_id(),
            use_openssh_for_git_tools: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub oauth: OAuthSettings,
    #[serde(default)]
    pub repo_roots: Vec<RepoRoot>,
    #[serde(default)]
    pub bindings: Vec<RepoBinding>,
    #[serde(default)]
    pub guard: GuardSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub name: String,
    pub private_key_path: String,
    pub public_key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyPair {
    pub private_key_path: String,
    pub public_key_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUser {
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub noreply_email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}
