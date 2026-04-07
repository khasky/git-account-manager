use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAccount {
    pub username: String,
    pub git_name: String,
    pub git_email: String,
    pub ssh_private_key_path: String,
    pub ssh_public_key_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub is_active: bool,
}

impl Profile {
    pub fn active_identity(&self) -> Option<(&str, &str)> {
        let platform = self.default_platform.as_deref();
        match platform {
            Some("github") => self.github.as_ref(),
            Some("gitlab") => self.gitlab.as_ref(),
            _ => self.github.as_ref().or(self.gitlab.as_ref()),
        }
        .map(|a| (a.git_name.as_str(), a.git_email.as_str()))
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
}

impl Default for OAuthSettings {
    fn default() -> Self {
        Self {
            github_client_id: default_github_client_id(),
            gitlab_client_id: default_gitlab_client_id(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub oauth: OAuthSettings,
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
