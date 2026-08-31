use serde::{Deserialize, Deserializer, Serialize};

/// A platform this app knows.
///
/// An enum rather than a string so the compiler, not a runtime check in one
/// module, is what rules out a fourth value: every `match` here is exhaustive,
/// and the `_ => None` / `_ => "github.com"` fallbacks that used to answer for
/// an unknown name — quietly, and sometimes wrongly — have nowhere left to
/// hide. The lowercase names are the wire format: what the webview sends, what
/// `profiles.json` holds, and what an SSH host alias is built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Github,
    Gitlab,
    Bitbucket,
}

/// The platforms this app knows, in the order every list and menu shows them.
pub const PLATFORMS: [Platform; 3] = [Platform::Github, Platform::Gitlab, Platform::Bitbucket];

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Github => "github",
            Platform::Gitlab => "gitlab",
            Platform::Bitbucket => "bitbucket",
        }
    }

    /// The platform's own spelling, for text a user reads.
    pub fn label(self) -> &'static str {
        match self {
            Platform::Github => "GitHub",
            Platform::Gitlab => "GitLab",
            Platform::Bitbucket => "Bitbucket",
        }
    }

    /// The host a remote for this platform really points at, as opposed to the
    /// `<platform>-<slug>` alias that may stand in for it.
    pub fn canonical_host(self) -> &'static str {
        match self {
            Platform::Github => "github.com",
            Platform::Gitlab => "gitlab.com",
            Platform::Bitbucket => "bitbucket.org",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stored `default_platform` naming something this build does not know reads
/// as "no preference" rather than failing the whole file. The old code did the
/// same thing with a `_ =>` arm; keeping it means a state file written by a
/// later version still opens here.
fn lenient_platform<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Platform>, D::Error> {
    Ok(Option::<String>::deserialize(d)?
        .and_then(|raw| serde_json::from_value(serde_json::Value::String(raw)).ok()))
}

/// Delimiters of the region this app owns inside `~/.gitconfig` and
/// `~/.ssh/config`. Two different modules rewrite those files and the markers
/// must never drift apart, so they live in one place.
pub const MANAGED_HEADER: &str = "# === begin git-account-manager ===";
pub const MANAGED_FOOTER: &str = "# === end git-account-manager ===";

/// Reduces a free-form name to a token usable as an SSH `Host` alias and as a
/// filename: anything that is not alphanumeric becomes a hyphen, runs of them
/// collapse, and the result never starts or ends with one. Letters outside
/// ASCII survive — dropping them would collapse two different non-Latin names
/// onto the same alias.
pub fn slugify(raw: &str) -> String {
    let collapsed = raw
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "unknown".to_string()
    } else {
        collapsed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAccount {
    pub username: String,
    pub git_name: String,
    pub git_email: String,
    pub ssh_private_key_path: String,
    pub ssh_public_key_path: String,
    /// Sign this account's commits with its own SSH key. Off by default so a
    /// profile written before this existed keeps committing exactly as it did;
    /// the form turns it on for accounts connected from here on.
    #[serde(default)]
    pub sign_commits: bool,
    #[serde(default, skip_serializing)]
    pub token: Option<String>,
}

impl PlatformAccount {
    /// The public key Git signs with, or `None` when this account does not sign.
    /// An account whose key was never generated cannot sign: pointing
    /// `user.signingkey` at an empty path makes every commit fail instead.
    pub fn signing_key(&self) -> Option<&str> {
        if self.sign_commits && !self.ssh_public_key_path.is_empty() {
            Some(self.ssh_public_key_path.as_str())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default, deserialize_with = "lenient_platform")]
    pub default_platform: Option<Platform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<PlatformAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gitlab: Option<PlatformAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitbucket: Option<PlatformAccount>,
    pub is_active: bool,
}

impl Profile {
    pub fn account(&self, platform: Platform) -> Option<&PlatformAccount> {
        match platform {
            Platform::Github => self.github.as_ref(),
            Platform::Gitlab => self.gitlab.as_ref(),
            Platform::Bitbucket => self.bitbucket.as_ref(),
        }
    }

    pub fn account_mut(&mut self, platform: Platform) -> Option<&mut PlatformAccount> {
        match platform {
            Platform::Github => self.github.as_mut(),
            Platform::Gitlab => self.gitlab.as_mut(),
            Platform::Bitbucket => self.bitbucket.as_mut(),
        }
    }

    /// The profile name as it appears in SSH host aliases (`github-<slug>`) and
    /// in generated identity filenames. Sanitized, because a name carrying `/`,
    /// `:` or `*` would otherwise write a broken `Host` line or escape the
    /// directory the identity file belongs in.
    pub fn slug(&self) -> String {
        slugify(&self.name)
    }

    /// The account the machine-wide config and the tray header speak for: the
    /// profile's chosen platform, or its first connected one when it has no
    /// choice recorded.
    pub fn active_account(&self) -> Option<&PlatformAccount> {
        self.default_platform
            .and_then(|p| self.account(p))
            .or_else(|| PLATFORMS.iter().find_map(|p| self.account(*p)))
    }

    pub fn active_identity(&self) -> Option<(&str, &str)> {
        self.active_account()
            .map(|a| (a.git_name.as_str(), a.git_email.as_str()))
    }
}

/// A folder that holds repositories belonging to one profile. Drives the scan
/// suggestions, the generated `includeIf "gitdir:"` blocks, and the switches
/// every repository inside it starts with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRoot {
    pub path: String,
    pub profile_id: String,
    pub platform: Platform,
    /// Default for the repositories in this folder. Installing the guard is the
    /// point of adding a folder, so it starts on.
    #[serde(default = "default_true")]
    pub install_hook: bool,
    /// Default for the repositories in this folder. Rewriting a remote is
    /// visible from outside the app, so it starts off.
    #[serde(default)]
    pub pin_remote_alias: bool,
}

/// One repository pinned to a profile. The identity is written to the
/// repository's own config, which is the only place every Git client — CLI,
/// libgit2/TortoiseGit, IDEs — agrees on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoBinding {
    pub path: String,
    pub profile_id: String,
    pub platform: Platform,
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
    /// Set once the user changes this repository's switches away from its
    /// folder's defaults. A later change to those defaults then leaves it alone,
    /// so a deliberate exception is not undone by an unrelated edit.
    #[serde(default)]
    pub overrides_root: bool,
    /// What `origin` was before the alias replaced it. Switching the alias back
    /// off restores this exactly; rebuilding a canonical URL instead would hand
    /// back an SSH address to a repository that was cloned over HTTPS, and would
    /// drop a non-default port along the way.
    #[serde(default)]
    pub original_remote_url: Option<String>,
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
    /// Why the key was not registered for signing, when that was asked for and
    /// failed. Carried instead of returned as an error because by then the key
    /// exists and authenticates: failing the whole call would leave it on disk
    /// and on the account with nothing in the profile pointing at it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformUser {
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub noreply_email: Option<String>,
    pub avatar_url: Option<String>,
    /// Why `username` is a fallback rather than the account's real name. Set
    /// only when the platform could not be asked, so the form can say so
    /// instead of presenting a wrong name as fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username_notice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slug is pasted into an SSH `Host` line and into a filename, so the
    /// characters that break either one must not survive: a path separator
    /// would escape the identities directory, `:` would split the scp-form
    /// remote URL in the wrong place, and `*`/`?` are SSH host wildcards.
    #[test]
    fn slug_drops_everything_that_breaks_a_host_alias_or_a_filename() {
        assert_eq!(slugify("Work/Team"), "work-team");
        assert_eq!(slugify(r"Work\Team"), "work-team");
        assert_eq!(slugify("host:22"), "host-22");
        assert_eq!(slugify("any*thing?"), "any-thing");
        assert_eq!(slugify("../escape"), "escape");
        assert_eq!(slugify("  spaced  out  "), "spaced-out");
        assert_eq!(slugify("!!!"), "unknown");
    }

    /// Plain names keep the shape aliases already written on disk have, so an
    /// existing install keeps resolving `github-<slug>`.
    #[test]
    fn slug_leaves_ordinary_names_untouched() {
        assert_eq!(slugify("Personal"), "personal");
        assert_eq!(slugify("Work Account"), "work-account");
        assert_eq!(slugify("my-pc"), "my-pc");
    }

    /// Stripping non-ASCII letters would map every Cyrillic name onto the same
    /// alias and silently point two profiles at one key.
    #[test]
    fn slug_keeps_non_ascii_letters_apart() {
        assert_ne!(slugify("Работа"), slugify("Личное"));
        assert_eq!(slugify("Работа"), "работа");
    }

    /// The lowercase names are a wire format shared with the webview and with
    /// every `profiles.json` already on disk. Renaming a variant would silently
    /// orphan both.
    #[test]
    fn platform_round_trips_through_its_stored_name() {
        for platform in PLATFORMS {
            let json = serde_json::to_string(&platform).unwrap();
            assert_eq!(json, format!("\"{}\"", platform.as_str()));
            assert_eq!(
                serde_json::from_str::<Platform>(&json).unwrap(),
                platform,
                "{} must survive a round trip",
                platform
            );
        }
        assert!(serde_json::from_str::<Platform>("\"gitea\"").is_err());
    }

    /// A file written by a later version that knows a fourth platform must still
    /// open here, with the unknown choice read as "no preference" — which is
    /// what the old string match's `_` arm did.
    #[test]
    fn an_unknown_default_platform_reads_as_no_preference() {
        let json = r#"{"id":"p1","name":"Work","default_platform":"gitea","is_active":true}"#;
        let profile: Profile = serde_json::from_str(json).expect("must still parse");
        assert_eq!(profile.default_platform, None);

        let known = r#"{"id":"p1","name":"Work","default_platform":"gitlab","is_active":true}"#;
        let profile: Profile = serde_json::from_str(known).unwrap();
        assert_eq!(profile.default_platform, Some(Platform::Gitlab));
    }

    /// The one regression that would cost every existing user their setup:
    /// `platform` moved from a `String` to an enum, and a state file written by
    /// the previous release has to keep opening. `load_state` treats a parse
    /// failure as corruption, so a mismatch here would not degrade — it would
    /// refuse to start with the profiles the user already has.
    #[test]
    fn a_state_file_from_the_previous_release_still_opens() {
        let on_disk = r#"{
          "profiles": [{
            "id": "p1",
            "name": "Work",
            "default_platform": "gitlab",
            "github": {
              "username": "octo",
              "git_name": "Octo",
              "git_email": "octo@example.com",
              "ssh_private_key_path": "C:/Users/a/.ssh/id_ed25519",
              "ssh_public_key_path": "C:/Users/a/.ssh/id_ed25519.pub"
            },
            "is_active": true
          }],
          "oauth": {
            "github_client_id": "abc",
            "gitlab_client_id": "def",
            "use_openssh_for_git_tools": true
          },
          "repo_roots": [{
            "path": "D:/repos",
            "profile_id": "p1",
            "platform": "github",
            "install_hook": true,
            "pin_remote_alias": false
          }],
          "bindings": [{
            "path": "D:/repos/demo",
            "profile_id": "p1",
            "platform": "bitbucket",
            "pin_remote_alias": true,
            "install_hook": true,
            "extra_allowed_emails": ["bot@example.com"],
            "overrides_root": true
          }],
          "guard": {
            "unset_global_identity": true,
            "manage_gitconfig_includes": false,
            "own_bare_ssh_hosts": true
          }
        }"#;

        let state: AppState = serde_json::from_str(on_disk).expect("must still open");
        assert_eq!(state.profiles[0].default_platform, Some(Platform::Gitlab));
        assert_eq!(state.repo_roots[0].platform, Platform::Github);
        assert_eq!(state.bindings[0].platform, Platform::Bitbucket);
        assert!(state.guard.unset_global_identity);
        assert!(state.oauth.use_openssh_for_git_tools);

        // And writes back the same names, so downgrading is not a trap either.
        let round_tripped = serde_json::to_string(&state).unwrap();
        assert!(round_tripped.contains(r#""platform":"github""#));
        assert!(round_tripped.contains(r#""platform":"bitbucket""#));
        assert!(round_tripped.contains(r#""default_platform":"gitlab""#));
    }

    fn account(email: &str) -> PlatformAccount {
        PlatformAccount {
            username: "octo".to_string(),
            git_name: "Octo".to_string(),
            git_email: email.to_string(),
            ssh_private_key_path: String::new(),
            ssh_public_key_path: String::new(),
            sign_commits: false,
            token: None,
        }
    }

    /// `user.signingkey` pointing at nothing makes every commit in the
    /// repository fail, so an account with the switch on but no key yet must
    /// still report that it does not sign.
    #[test]
    fn an_account_without_a_key_does_not_sign() {
        let mut acc = account("octo@example.com");
        acc.sign_commits = true;
        assert_eq!(acc.signing_key(), None);

        acc.ssh_public_key_path = "C:/Users/a/.ssh/id_ed25519.pub".to_string();
        assert_eq!(acc.signing_key(), Some("C:/Users/a/.ssh/id_ed25519.pub"));

        acc.sign_commits = false;
        assert_eq!(acc.signing_key(), None);
    }

    /// What the tray header and the machine-wide identity are read from.
    #[test]
    fn the_active_identity_prefers_the_chosen_platform_then_falls_back() {
        let mut profile = Profile {
            id: "p1".to_string(),
            name: "Work".to_string(),
            default_platform: Some(Platform::Gitlab),
            github: Some(account("gh@example.com")),
            gitlab: Some(account("gl@example.com")),
            bitbucket: None,
            is_active: true,
        };
        assert_eq!(profile.active_identity().unwrap().1, "gl@example.com");

        // No choice recorded: the first connected account in display order.
        profile.default_platform = None;
        assert_eq!(profile.active_identity().unwrap().1, "gh@example.com");

        // A choice pointing at an account that is no longer connected must not
        // leave the profile without an identity.
        profile.default_platform = Some(Platform::Bitbucket);
        assert_eq!(profile.active_identity().unwrap().1, "gh@example.com");

        profile.github = None;
        profile.gitlab = None;
        assert_eq!(profile.active_identity(), None);
    }
}
