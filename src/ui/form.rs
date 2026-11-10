//! The profile form: a name, one section per platform, the default identity
//! and the folders the profile owns.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::helpers::{attached_key, save_percent};
use super::settings::panel;
use super::theme::{hex, Palette, BLUE_600};
use super::widgets::*;
use super::{blocking, f, palette, t, tokio};
use crate::commands::{self, SaveProgress};
use crate::doctor::FolderStatus;
use crate::git::GitIdentity;
use crate::models::{
    DeviceCodeResponse, OAuthSettings, Platform, PlatformAccount, PlatformUser, Profile, RepoRoot,
    SshKeyInfo, PLATFORMS,
};
use crate::repos::FolderRepo;

const COPY_HINT: Duration = Duration::from_secs(2);

/// How long the GitLab browser flow waits before giving up, matching the
/// backend's own callback timeout.
const GITLAB_TIMEOUT_S: u64 = 120;

/// GitHub's documented device-flow lifetime, used until the real one arrives.
const GITHUB_FALLBACK_EXPIRY_S: u64 = 900;

#[derive(Clone, Debug, PartialEq)]
pub enum SectionError {
    None,
    /// The OAuth app is not configured: the message links to Settings.
    Settings,
    Message(String),
}

/// One platform's part of the form.
pub struct Section {
    pub connected: bool,
    pub connecting: bool,
    pub username: String,
    pub public_email: String,
    pub noreply_email: String,
    pub private_key: String,
    pub public_key: String,
    /// `true` for "generate and upload", `false` for "use an existing key".
    pub generate: bool,
    pub selected_key: String,
    pub error: SectionError,
    pub key_uploaded: bool,
    /// A key is being generated, uploaded or checked; its buttons wait.
    pub key_busy: bool,
    pub sign_commits: bool,
    pub signing_error: String,
    pub username_notice: String,
    pub https_stored: bool,
    pub https_notice: String,
    pub device: Option<DeviceCodeResponse>,
    /// Whether the HTTPS field's placeholder says a token is stored, as last
    /// set; the input keeps no getter for it.
    pub https_placeholder: std::cell::Cell<Option<bool>>,
    /// Seconds left in the running flow; 0 hides the counter.
    pub countdown: u64,
    pub git_name: Entity<InputState>,
    pub git_email: Entity<InputState>,
    pub https: Entity<InputState>,
    /// Bitbucket's email or Azure DevOps' organization.
    pub first: Entity<InputState>,
    pub token: Entity<InputState>,
    flow: Option<Task<()>>,
}

impl Section {
    fn clear(&mut self) {
        self.connected = false;
        self.connecting = false;
        self.username.clear();
        self.public_email.clear();
        self.noreply_email.clear();
        self.private_key.clear();
        self.public_key.clear();
        self.generate = true;
        self.selected_key.clear();
        self.error = SectionError::None;
        self.key_uploaded = false;
        self.key_busy = false;
        // On for anything connected from here on: a signed commit is what
        // earns the "Verified" badge.
        self.sign_commits = true;
        self.signing_error.clear();
        self.username_notice.clear();
        self.https_stored = false;
        self.https_notice.clear();
        self.device = None;
        self.countdown = 0;
        self.flow = None;
    }
}

#[derive(Clone, Debug)]
pub enum FormEvent {
    Saved,
    Cancel,
    Settings,
    Delete(String, bool),
}

impl EventEmitter<FormEvent> for ProfileForm {}

/// A result addressed to the control that produced it.
#[derive(Clone)]
pub struct RepoNote {
    pub key: String,
    pub ok: bool,
    pub text: String,
}

pub struct ProfileForm {
    pub profile: Option<Profile>,
    pub profile_id: String,
    initial: [bool; 6],
    pub name: Entity<InputState>,
    pub imported_email: String,
    pub default_platform: Platform,
    pub sections: Vec<Section>,
    pub ssh_keys: Vec<SshKeyInfo>,
    pub saving: bool,
    pub progress: Option<SaveProgress>,
    pub disconnect: Option<(Platform, String, String)>,
    pub error: String,
    pub settings: Option<OAuthSettings>,
    pub copied: Option<String>,
    copy_timer: Option<Task<()>>,
    // The folders are a draft written only by Save, so Cancel leaves the
    // machine untouched and a new profile can have folders before it exists.
    pub roots: Vec<RepoRoot>,
    pub repos: Vec<FolderRepo>,
    pub statuses: Vec<FolderStatus>,
    pub repos_loading: bool,
    pub busy: String,
    pub note: Option<RepoNote>,
    pub open_roots: HashMap<String, bool>,
    pub(super) root_selects: HashMap<String, (Vec<Platform>, Entity<Select>)>,
    auto_scanned: bool,
    pub scroll: ScrollHandle,
}

pub fn index(platform: Platform) -> usize {
    PLATFORMS
        .iter()
        .position(|&p| p == platform)
        .expect("known platform")
}

impl ProfileForm {
    pub fn new(
        profile: Option<Profile>,
        prefill: Option<GitIdentity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_value = profile
            .as_ref()
            .map(|p| p.name.clone())
            .or_else(|| prefill.as_ref().map(|id| id.name.clone()))
            .unwrap_or_default();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t(cx, "form.profileNamePlaceholder"))
                .default_value(name_value)
        });
        let sections = PLATFORMS
            .into_iter()
            .map(|platform| {
                let account = profile.as_ref().and_then(|p| p.account(platform));
                Self::section_for(platform, account, window, cx)
            })
            .collect();
        let initial = PLATFORMS.map(|platform| {
            profile
                .as_ref()
                .is_some_and(|p| p.account(platform).is_some())
        });
        let mut form = Self {
            profile_id: profile
                .as_ref()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            default_platform: profile
                .as_ref()
                .and_then(|p| p.default_platform)
                .unwrap_or(Platform::Github),
            profile,
            initial,
            name,
            imported_email: prefill.map(|id| id.email).unwrap_or_default(),
            sections,
            ssh_keys: Vec::new(),
            saving: false,
            progress: None,
            disconnect: None,
            error: String::new(),
            settings: None,
            copied: None,
            copy_timer: None,
            roots: Vec::new(),
            repos: Vec::new(),
            statuses: Vec::new(),
            repos_loading: true,
            busy: String::new(),
            note: None,
            open_roots: HashMap::new(),
            root_selects: HashMap::new(),
            auto_scanned: false,
            scroll: ScrollHandle::new(),
        };
        form.watch_inputs(window, cx);
        form
    }

    fn section_for(
        platform: Platform,
        account: Option<&PlatformAccount>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Section {
        let input = |value: String,
                     placeholder: String,
                     masked: bool,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(masked)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let (first, token) = match platform {
            Platform::Bitbucket => (
                "form.bitbucket.emailPlaceholder",
                "form.bitbucket.tokenPlaceholder",
            ),
            Platform::Azure => ("form.azure.orgPlaceholder", "form.azure.tokenPlaceholder"),
            Platform::Codeberg => ("", "form.codeberg.tokenPlaceholder"),
            Platform::Huggingface => ("", "form.huggingface.tokenPlaceholder"),
            _ => ("", ""),
        };
        let placeholder = |key: &str, cx: &App| {
            if key.is_empty() {
                String::new()
            } else {
                t(cx, key)
            }
        };
        let first = input(String::new(), placeholder(first, cx), false, window, cx);
        let token = input(String::new(), placeholder(token, cx), true, window, cx);
        let https = input(
            String::new(),
            t(cx, "form.https.placeholder"),
            true,
            window,
            cx,
        );
        let git_name = input(
            account.map(|a| a.git_name.clone()).unwrap_or_default(),
            String::new(),
            false,
            window,
            cx,
        );
        let git_email = input(
            account.map(|a| a.git_email.clone()).unwrap_or_default(),
            t(cx, "form.customEmailPlaceholder"),
            false,
            window,
            cx,
        );
        let mut section = Section {
            connected: false,
            connecting: false,
            username: String::new(),
            public_email: String::new(),
            noreply_email: String::new(),
            private_key: String::new(),
            public_key: String::new(),
            generate: true,
            selected_key: String::new(),
            error: SectionError::None,
            key_uploaded: false,
            key_busy: false,
            sign_commits: true,
            signing_error: String::new(),
            username_notice: String::new(),
            https_stored: false,
            https_notice: String::new(),
            device: None,
            countdown: 0,
            https_placeholder: std::cell::Cell::new(None),
            git_name,
            git_email,
            https,
            first,
            token,
            flow: None,
        };
        section.clear();
        if let Some(account) = account {
            section.connected = true;
            section.username = account.username.clone();
            section.private_key = account.ssh_private_key_path.clone();
            section.public_key = account.ssh_public_key_path.clone();
            section.generate = false;
            section.selected_key = account.ssh_private_key_path.clone();
            section.key_uploaded = true;
            // A profile saved before signing existed has it off; turning it
            // on behind the user's back would start signing its commits.
            section.sign_commits = account.sign_commits;
        }
        section
    }

    /// Re-renders on typing where the text decides what is drawn, and hands
    /// an HTTPS token over to be checked when its field is left.
    fn watch_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (ix, section) in self.sections.iter().enumerate() {
            for input in [&section.git_name, &section.git_email, &section.first] {
                cx.subscribe(input, |_, _, _: &InputEvent, cx| cx.notify())
                    .detach();
            }
            cx.subscribe_in(
                &section.https,
                window,
                move |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur) {
                        this.commit_https_token(PLATFORMS[ix], window, cx);
                    }
                },
            )
            .detach();
        }
    }

    pub fn is_edit(&self) -> bool {
        self.profile.is_some()
    }

    pub fn section(&self, platform: Platform) -> &Section {
        &self.sections[index(platform)]
    }

    pub fn section_mut(&mut self, platform: Platform) -> &mut Section {
        &mut self.sections[index(platform)]
    }

    pub fn start(&mut self, cx: &mut Context<Self>) {
        let profile_id = self.profile_id.clone();
        let edit = self.is_edit();
        cx.spawn(async move |this, cx| {
            let keys = blocking(commands::list_ssh_keys).await.unwrap_or_default();
            let settings = blocking(commands::get_settings).await.ok();
            let id = profile_id.clone();
            let stored = if edit {
                blocking(move || commands::https_token_platforms(id))
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let _ = this.update(cx, |this, cx| {
                this.ssh_keys = keys;
                this.settings = settings;
                for platform in stored {
                    this.section_mut(platform).https_stored = true;
                }
                cx.notify();
            });
            let state = super::tokio(commands::get_repo_state()).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(state) = state {
                    this.roots = state
                        .roots
                        .into_iter()
                        .filter(|root| root.profile_id == this.profile_id)
                        .collect();
                }
                this.repos_loading = false;
                this.auto_scan(cx);
                cx.notify();
            });
            let _ = this.update(cx, |this, cx| this.load_doctor(cx));
        })
        .detach();
    }

    /// Opening a profile that already has folders shows what is in them, once.
    fn auto_scan(&mut self, cx: &mut Context<Self>) {
        if self.auto_scanned || self.roots.is_empty() {
            return;
        }
        self.auto_scanned = true;
        let roots = self.roots.clone();
        self.scan_with(roots, cx);
    }

    pub fn load_doctor(&mut self, cx: &mut Context<Self>) {
        let id = self.profile_id.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(report) = tokio(commands::doctor()).await {
                let _ = this.update(cx, |this, cx| {
                    this.statuses = report
                        .folders
                        .into_iter()
                        .filter(|folder| folder.profile_id == id)
                        .collect();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn set_progress(&mut self, progress: SaveProgress, cx: &mut Context<Self>) {
        if self.saving {
            self.progress = Some(progress);
            cx.notify();
        }
    }

    // -- leaving ------------------------------------------------------------------

    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A save in flight owns the window until it answers.
        if self.saving {
            return;
        }
        if self.disconnect.take().is_some() {
            cx.notify();
            return;
        }
        self.cancel(window, cx);
    }

    fn cleanup_unsaved_tokens(&self) {
        let id = self.profile_id.clone();
        if !self.is_edit() {
            crate::runtime().spawn_blocking(move || commands::delete_profile_tokens(id));
            return;
        }
        for (ix, platform) in PLATFORMS.into_iter().enumerate() {
            if !self.initial[ix] && self.sections[ix].connected {
                let id = id.clone();
                crate::runtime()
                    .spawn_blocking(move || commands::delete_platform_token(id, platform));
            }
        }
    }

    pub fn cancel(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.section(Platform::Gitlab).connecting {
            self.cancel_gitlab(cx);
        }
        self.cancel_github(cx);
        self.cleanup_unsaved_tokens();
        cx.emit(FormEvent::Cancel);
    }

    // -- connecting -----------------------------------------------------------------

    pub fn cancel_github(&mut self, cx: &mut Context<Self>) {
        let section = self.section_mut(Platform::Github);
        section.flow = None;
        section.connecting = false;
        section.device = None;
        section.countdown = 0;
        section.error = SectionError::None;
        cx.notify();
    }

    pub fn cancel_gitlab(&mut self, cx: &mut Context<Self>) {
        crate::runtime().spawn_blocking(commands::gitlab_oauth_abort);
        let section = self.section_mut(Platform::Gitlab);
        section.flow = None;
        section.connecting = false;
        section.countdown = 0;
        section.error = SectionError::None;
        cx.notify();
    }

    /// What a freshly connected account fills in. The noreply address hides
    /// a private one, so it wins.
    fn connected(
        &mut self,
        platform: Platform,
        user: PlatformUser,
        keep_noreply: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let noreply = if keep_noreply {
            user.noreply_email.clone().unwrap_or_default()
        } else {
            String::new()
        };
        let public = user.email.clone().unwrap_or_default();
        let email = [&noreply, &public, &self.imported_email]
            .into_iter()
            .find(|email| !email.is_empty())
            .cloned()
            .unwrap_or_default();
        let name = user
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| user.username.clone());
        let section = self.section_mut(platform);
        section.flow = None;
        section.connecting = false;
        section.connected = true;
        section.device = None;
        section.countdown = 0;
        section.username = user.username;
        section.username_notice = user.username_notice.unwrap_or_default();
        section.public_email = public;
        section.noreply_email = noreply;
        let (name_input, email_input) = (section.git_name.clone(), section.git_email.clone());
        name_input.update(cx, |s, cx| s.set_value(name, window, cx));
        email_input.update(cx, |s, cx| s.set_value(email, window, cx));
        cx.notify();
    }

    fn fail(&mut self, platform: Platform, error: String, cx: &mut Context<Self>) {
        let section = self.section_mut(platform);
        section.flow = None;
        section.connecting = false;
        section.device = None;
        section.countdown = 0;
        section.error = SectionError::Message(error);
        cx.notify();
    }

    pub fn connect(&mut self, platform: Platform, window: &mut Window, cx: &mut Context<Self>) {
        match platform {
            Platform::Github => self.connect_github(window, cx),
            Platform::Gitlab => self.connect_gitlab(window, cx),
            _ => self.connect_with_credentials(platform, window, cx),
        }
    }

    fn connect_github(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let client_id = self
            .settings
            .as_ref()
            .map(|s| s.github_client_id.clone())
            .unwrap_or_default();
        if client_id.is_empty() {
            self.section_mut(Platform::Github).error = SectionError::Settings;
            cx.notify();
            return;
        }
        let profile_id = self.profile_id.clone();
        let section = self.section_mut(Platform::Github);
        section.connecting = true;
        section.error = SectionError::None;
        cx.notify();
        let task = cx.spawn_in(window, async move |this, cx| {
            let id = client_id.clone();
            let device = match tokio(commands::github_oauth_start(id)).await {
                Ok(device) => device,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| this.fail(Platform::Github, error, cx));
                    return;
                }
            };
            let expires = if device.expires_in > 0 {
                device.expires_in
            } else {
                GITHUB_FALLBACK_EXPIRY_S
            };
            let uri = device.verification_uri.clone();
            let _ = this.update(cx, |this, cx| {
                let section = this.section_mut(Platform::Github);
                section.device = Some(device.clone());
                section.countdown = expires;
                cx.open_url(&uri);
                cx.notify();
            });
            let started = Instant::now();
            let interval = Duration::from_secs(device.interval + 1);
            let mut next_poll = started + interval;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let left = expires.saturating_sub(started.elapsed().as_secs());
                if this
                    .update(cx, |this, cx| {
                        this.section_mut(Platform::Github).countdown = left;
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                if left == 0 {
                    let message = cx
                        .update(|_, cx| t(cx, "form.authTimedOut"))
                        .unwrap_or_default();
                    let _ = this.update(cx, |this, cx| this.fail(Platform::Github, message, cx));
                    return;
                }
                if Instant::now() < next_poll {
                    continue;
                }
                next_poll += interval;
                let poll = commands::github_oauth_poll(
                    client_id.clone(),
                    device.device_code.clone(),
                    profile_id.clone(),
                );
                match tokio(poll).await {
                    Ok(None) => {}
                    Ok(Some(user)) => {
                        let _ = this.update_in(cx, |this, window, cx| {
                            this.connected(Platform::Github, user, true, window, cx)
                        });
                        return;
                    }
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| this.fail(Platform::Github, error, cx));
                        return;
                    }
                }
            }
        });
        self.section_mut(Platform::Github).flow = Some(task);
    }

    fn connect_gitlab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let client_id = self
            .settings
            .as_ref()
            .map(|s| s.gitlab_client_id.clone())
            .unwrap_or_default();
        if client_id.is_empty() {
            self.section_mut(Platform::Gitlab).error = SectionError::Settings;
            cx.notify();
            return;
        }
        let profile_id = self.profile_id.clone();
        let section = self.section_mut(Platform::Gitlab);
        section.connecting = true;
        section.error = SectionError::None;
        section.countdown = GITLAB_TIMEOUT_S;
        cx.notify();
        let task = cx.spawn_in(window, async move |this, cx| {
            let connect = tokio(commands::gitlab_oauth_connect(client_id, profile_id));
            let countdown = async {
                let started = Instant::now();
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let left = GITLAB_TIMEOUT_S.saturating_sub(started.elapsed().as_secs());
                    let _ = this.update(cx, |this, cx| {
                        this.section_mut(Platform::Gitlab).countdown = left;
                        cx.notify();
                    });
                }
            };
            let result = {
                futures::pin_mut!(connect, countdown);
                match futures::future::select(connect, countdown).await {
                    futures::future::Either::Left((result, _)) => result,
                    futures::future::Either::Right(_) => return,
                }
            };
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(user) => this.connected(Platform::Gitlab, user, true, window, cx),
                Err(error) => this.fail(Platform::Gitlab, error, cx),
            });
        });
        self.section_mut(Platform::Gitlab).flow = Some(task);
    }

    fn connect_with_credentials(
        &mut self,
        platform: Platform,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let section = self.section(platform);
        let first = value(&section.first, cx).trim().to_string();
        let token = value(&section.token, cx).trim().to_string();
        let needs_first = matches!(platform, Platform::Bitbucket | Platform::Azure);
        if token.is_empty() || (needs_first && first.is_empty()) {
            let key = format!("form.{}.errCreds", platform.as_str());
            self.section_mut(platform).error = SectionError::Message(t(cx, &key));
            cx.notify();
            return;
        }
        let profile_id = self.profile_id.clone();
        let section = self.section_mut(platform);
        section.connecting = true;
        section.error = SectionError::None;
        cx.notify();
        let token_input = section.token.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = match platform {
                Platform::Bitbucket => {
                    tokio(commands::connect_bitbucket(profile_id, first, token)).await
                }
                Platform::Azure => tokio(commands::connect_azure(profile_id, first, token)).await,
                _ => tokio(commands::connect_with_token(profile_id, platform, token)).await,
            };
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(user) => {
                    // Bitbucket and Azure DevOps have no noreply address;
                    // Codeberg names one and Hugging Face has none.
                    let keep = !matches!(platform, Platform::Bitbucket | Platform::Azure);
                    this.connected(platform, user, keep, window, cx);
                    token_input.update(cx, |s, cx| s.set_value("", window, cx));
                }
                Err(error) => this.fail(platform, error, cx),
            });
        });
        self.section_mut(platform).flow = Some(task);
    }

    // -- keys ---------------------------------------------------------------------------

    pub fn copy_public_key(&mut self, path: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let Ok(content) = blocking(move || commands::read_public_key(target)).await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                super::copy(content.trim().to_string(), cx);
                this.copied = Some(path);
                this.copy_timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(COPY_HINT).await;
                    let _ = this.update(cx, |this, cx| {
                        this.copied = None;
                        cx.notify();
                    });
                }));
                cx.notify();
            });
        })
        .detach();
    }

    fn key_busy(&mut self, platform: Platform, busy: bool, cx: &mut Context<Self>) -> bool {
        let section = self.section_mut(platform);
        if busy && section.key_busy {
            return false;
        }
        section.key_busy = busy;
        cx.notify();
        true
    }

    pub fn generate_key(&mut self, platform: Platform, cx: &mut Context<Self>) {
        if !self.key_busy(platform, true, cx) {
            return;
        }
        let section = self.section_mut(platform);
        section.error = SectionError::None;
        let email = value(&section.git_email, cx);
        let email = if email.is_empty() {
            "git@git-account-manager".to_string()
        } else {
            email
        };
        let (profile_id, username, sign) = (
            self.profile_id.clone(),
            self.section(platform).username.clone(),
            self.section(platform).sign_commits,
        );
        cx.spawn(async move |this, cx| {
            let result = tokio(commands::generate_and_upload_key(
                platform, profile_id, username, email, sign,
            ))
            .await;
            let keys = blocking(commands::list_ssh_keys).await.unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                this.ssh_keys = keys;
                match result {
                    Ok(pair) => {
                        let section = this.section_mut(platform);
                        section.private_key = pair.private_key_path.clone();
                        section.public_key = pair.public_key_path.clone();
                        if let Some(url) = pair.manual_upload_url {
                            // The platform will not take the key from a token:
                            // hand it over on the page that does, and leave it
                            // selected so "Upload" confirms it arrived.
                            section.generate = false;
                            section.selected_key = pair.private_key_path;
                            section.key_uploaded = false;
                            this.copy_public_key(pair.public_key_path, cx);
                            cx.open_url(&url);
                        } else {
                            section.key_uploaded = true;
                            section.signing_error = pair.signing_error.unwrap_or_default();
                        }
                    }
                    Err(error) => this.section_mut(platform).error = SectionError::Message(error),
                }
                this.key_busy(platform, false, cx);
            });
        })
        .detach();
    }

    pub fn upload_key(&mut self, platform: Platform, sign_only: bool, cx: &mut Context<Self>) {
        let section = self.section(platform);
        if section.public_key.is_empty() || (sign_only && !section.key_uploaded) {
            return;
        }
        if !sign_only && !self.key_busy(platform, true, cx) {
            return;
        }
        let public = self.section(platform).public_key.clone();
        let sign = sign_only || self.section(platform).sign_commits;
        let title = format!("git-account-manager: {}", value(&self.name, cx));
        let profile_id = self.profile_id.clone();
        if !sign_only {
            self.section_mut(platform).error = SectionError::None;
        }
        cx.spawn(async move |this, cx| {
            let result = async {
                let content = blocking(move || commands::read_public_key(public)).await?;
                tokio(commands::upload_ssh_key_to_platform(
                    platform, profile_id, title, content, sign,
                ))
                .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                let section = this.section_mut(platform);
                match result {
                    Ok(signing) => {
                        if !sign_only {
                            section.key_uploaded = true;
                        }
                        section.signing_error = signing.unwrap_or_default();
                    }
                    Err(error) if sign_only => section.signing_error = error,
                    Err(error) => section.error = SectionError::Message(error),
                }
                if !sign_only {
                    this.key_busy(platform, false, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_sign(&mut self, platform: Platform, on: bool, cx: &mut Context<Self>) {
        self.section_mut(platform).sign_commits = on;
        cx.notify();
        // Signing on for an uploaded key has to reach the platform too.
        if on {
            self.upload_key(platform, true, cx);
        }
    }

    // -- tokens ---------------------------------------------------------------------------

    pub fn commit_https_token(
        &mut self,
        platform: Platform,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let token = value(&self.section(platform).https, cx).trim().to_string();
        if token.is_empty() {
            return;
        }
        let profile_id = self.profile_id.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = tokio(commands::save_https_token(profile_id, platform, token)).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(notice) => {
                        let section = this.section_mut(platform);
                        section.https_stored = true;
                        section.https_notice = notice.unwrap_or_default();
                        let input = section.https.clone();
                        input.update(cx, |s, cx| s.set_value("", window, cx));
                    }
                    Err(error) => this.error = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn remove_https_token(&mut self, platform: Platform, cx: &mut Context<Self>) {
        let profile_id = self.profile_id.clone();
        cx.spawn(async move |this, cx| {
            let result = tokio(commands::save_https_token(
                profile_id,
                platform,
                String::new(),
            ))
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        let section = this.section_mut(platform);
                        section.https_stored = false;
                        section.https_notice.clear();
                    }
                    // The field keeps saying a token is stored, because one is.
                    Err(error) => this.error = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    // -- saving ---------------------------------------------------------------------------

    fn account(&self, platform: Platform, cx: &App) -> Option<PlatformAccount> {
        let section = self.section(platform);
        if !section.connected || section.private_key.is_empty() {
            return None;
        }
        Some(PlatformAccount {
            username: section.username.clone(),
            git_name: value(&section.git_name, cx),
            git_email: value(&section.git_email, cx),
            ssh_private_key_path: section.private_key.clone(),
            ssh_public_key_path: section.public_key.clone(),
            sign_commits: section.sign_commits,
            token: None,
        })
    }

    /// The profile as it stands in the form; the folder scan needs this rather
    /// than the saved copy.
    pub fn draft(&self, cx: &App) -> Profile {
        Profile {
            id: self.profile_id.clone(),
            name: value(&self.name, cx).trim().to_string(),
            default_platform: Some(self.default_platform),
            github: self.account(Platform::Github, cx),
            gitlab: self.account(Platform::Gitlab, cx),
            bitbucket: self.account(Platform::Bitbucket, cx),
            azure: self.account(Platform::Azure, cx),
            codeberg: self.account(Platform::Codeberg, cx),
            huggingface: self.account(Platform::Huggingface, cx),
            is_active: self.profile.as_ref().is_some_and(|p| p.is_active),
        }
    }

    pub fn connected_platforms(&self) -> Vec<Platform> {
        PLATFORMS
            .into_iter()
            .filter(|&platform| self.section(platform).connected)
            .collect()
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        if value(&self.name, cx).trim().is_empty() {
            self.error = t(cx, "form.errProfileName");
            cx.notify();
            return;
        }
        if self.connected_platforms().is_empty() {
            self.error = t(cx, "form.errConnectOne");
            cx.notify();
            return;
        }
        self.saving = true;
        self.progress = None;
        self.error.clear();
        cx.notify();
        let profile = self.draft(cx);
        let tokens: Vec<(Platform, String)> = PLATFORMS
            .into_iter()
            .map(|platform| {
                (
                    platform,
                    value(&self.section(platform).https, cx).trim().to_string(),
                )
            })
            .filter(|(_, token)| !token.is_empty())
            .collect();
        let roots = self.roots.clone();
        cx.spawn(async move |this, cx| {
            let id = profile.id.clone();
            let result = async {
                blocking(move || commands::save_profile(profile)).await?;
                for (platform, token) in tokens {
                    tokio(commands::save_https_token(id.clone(), platform, token)).await?;
                }
                tokio(commands::save_profile_folders(id, roots)).await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                this.progress = None;
                match result {
                    Ok(_) => cx.emit(FormEvent::Saved),
                    Err(error) => this.error = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn disconnect_now(&mut self, delete_keys: bool, cx: &mut Context<Self>) {
        let Some((platform, key, public)) = self.disconnect.take() else {
            return;
        };
        let profile_id = self.profile_id.clone();
        let remaining = self
            .connected_platforms()
            .into_iter()
            .any(|other| other != platform);
        cx.spawn(async move |this, cx| {
            if delete_keys && !key.is_empty() {
                if !public.is_empty() {
                    let _ = tokio(commands::remove_ssh_key_from_platform(
                        platform,
                        profile_id.clone(),
                        public,
                    ))
                    .await;
                }
                let _ = blocking(move || commands::delete_ssh_keys(vec![key])).await;
            }
            let id = profile_id.clone();
            let _ = blocking(move || commands::delete_platform_token(id, platform)).await;
            let _ = this.update(cx, |this, cx| {
                this.section_mut(platform).clear();
                cx.notify();
                let Some(profile) = this.profile.clone() else {
                    return;
                };
                // Disconnecting the last account leaves nothing to identify
                // the profile with, so the profile itself goes.
                if !remaining {
                    cx.emit(FormEvent::Delete(profile.id, false));
                    return;
                }
                let mut updated = profile;
                match platform {
                    Platform::Github => updated.github = None,
                    Platform::Gitlab => updated.gitlab = None,
                    Platform::Bitbucket => updated.bitbucket = None,
                    Platform::Azure => updated.azure = None,
                    Platform::Codeberg => updated.codeberg = None,
                    Platform::Huggingface => updated.huggingface = None,
                }
                cx.spawn(async move |this, cx| {
                    if blocking(move || commands::save_profile(updated))
                        .await
                        .is_ok()
                    {
                        let _ = this.update(cx, |_, cx| cx.emit(FormEvent::Saved));
                    }
                })
                .detach();
            });
        })
        .detach();
    }

    pub fn ask_disconnect(&mut self, platform: Platform, cx: &mut Context<Self>) {
        let section = self.section(platform);
        let (key, public) = attached_key(
            section.key_uploaded,
            &section.private_key,
            &section.public_key,
        );
        self.disconnect = Some((platform, key, public));
        cx.notify();
    }

    pub fn import_from_git(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let Ok(identity) = blocking(commands::get_git_identity).await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !identity.name.is_empty() && value(&this.name, cx).trim().is_empty() {
                    let name = this.name.clone();
                    name.update(cx, |s, cx| s.set_value(identity.name, window, cx));
                }
                if !identity.email.is_empty() {
                    this.imported_email = identity.email;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

// -- drawing ----------------------------------------------------------------------

impl ProfileForm {
    fn default_identity(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let connected = self.connected_platforms();
        if connected.len() < 2 {
            return None;
        }
        let active = connected
            .iter()
            .copied()
            .find(|&platform| platform == self.default_platform)
            .or(connected.first().copied())?;
        let section = self.section(active);
        Some(
            panel(p)
                .gap_0()
                .child(
                    div()
                        .mb(px(4.))
                        .ts(SM)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.fg3)
                        .child(t(cx, "form.defaultIdentity")),
                )
                .child(div().mb(px(4.)).ts(XS).child(rich(
                    "default-hint",
                    t(cx, "form.defaultIdentityHint1"),
                    p.fg5,
                    p.fg4,
                    p,
                    Link::None,
                )))
                .child(
                    div()
                        .mb(px(12.))
                        .ts(XS)
                        .text_color(p.fg5)
                        .child(t(cx, "form.defaultIdentityHint2")),
                )
                .child(div().mb(px(12.)).flex().flex_wrap().gap(px(12.)).children(
                    connected.iter().map(|&platform| {
                        let selected = platform == self.default_platform;
                        control(
                            SharedString::from(format!("default-{}", platform.as_str())),
                            p,
                        )
                        .rounded(px(6.))
                        .px(px(12.))
                        .py(px(6.))
                        .ts(SM)
                        .when(selected, |b| b.bg(hex(BLUE_600)).text_color(white()))
                        .when(!selected, |b| b.bg(p.subtle).text_color(p.fg3))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.default_platform = platform;
                            cx.notify();
                        }))
                        .child(platform.label())
                    }),
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .ts(XS)
                        .text_color(p.fg4)
                        .child(format!("{}\u{a0}", t(cx, "form.activeLabel")))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.fg2)
                                .child(value(&section.git_name, cx)),
                        )
                        .child(
                            div()
                                .text_color(p.fg5)
                                .child(format!("\u{a0}<{}>", value(&section.git_email, cx))),
                        ),
                ),
        )
    }

    fn disconnect_dialog(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (platform, key, _) = self.disconnect.as_ref()?;
        let action = |id: &'static str,
                      label: String,
                      variant: Variant,
                      delete: Option<bool>,
                      cx: &mut Context<Self>| {
            dialog_action(id, label, variant, p).on_click(cx.listener(move |this, _, _, cx| {
                match delete {
                    Some(delete_keys) => this.disconnect_now(delete_keys, cx),
                    None => {
                        this.disconnect = None;
                        cx.notify();
                    }
                }
            }))
        };
        let mut actions = Vec::new();
        if key.is_empty() {
            actions.push(action(
                "disconnect",
                t(cx, "form.disconnect"),
                Variant::Danger,
                Some(false),
                cx,
            ));
        } else {
            actions.push(action(
                "disconnect-delete",
                t(cx, "form.disconnectAndDelete"),
                Variant::Danger,
                Some(true),
                cx,
            ));
            actions.push(action(
                "disconnect-keep",
                t(cx, "form.disconnectKeep"),
                Variant::Default,
                Some(false),
                cx,
            ));
        }
        actions.push(action(
            "disconnect-cancel",
            t(cx, "form.cancel"),
            Variant::Cancel,
            None,
            cx,
        ));
        let body = div()
            .child(
                div()
                    .mb(px(12.))
                    .ts(SM)
                    .text_color(p.fg3)
                    .child(t(cx, "form.disconnectBody")),
            )
            .when(!key.is_empty(), |body| {
                body.child(super::app::key_list(
                    t(cx, "form.disconnectKeyLabel"),
                    std::slice::from_ref(key),
                    p,
                ))
            });
        let view = cx.entity().downgrade();
        Some(confirm_dialog(
            f(
                cx,
                "form.disconnectTitle",
                &[("platform", platform.label())],
            ),
            body,
            actions,
            move |_, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.disconnect = None;
                    cx.notify();
                });
            },
            p,
        ))
    }

    fn progress_overlay(&self, p: &Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.saving {
            return None;
        }
        let label = match &self.progress {
            None => t(cx, "form.savingProfile"),
            Some(progress) if progress.stage == "apply" => t(cx, "form.savingApply"),
            Some(progress) if progress.repos_total == 0 => {
                f(cx, "form.savingWalk", &[("folder", &progress.folder)])
            }
            Some(progress) => f(
                cx,
                "form.savingScan",
                &[
                    ("folder", &progress.folder),
                    ("done", &progress.repos_done.to_string()),
                    ("total", &progress.repos_total.to_string()),
                ],
            ),
        };
        let percent = self.progress.as_ref().map(save_percent);
        let bar = match percent {
            None => div()
                .size_full()
                .bg(hex(BLUE_600))
                .with_animation(
                    "pulse",
                    Animation::new(Duration::from_secs(2)).repeat(),
                    |bar, delta| {
                        bar.opacity(0.5 + 0.5 * (delta * std::f32::consts::TAU).cos().abs())
                    },
                )
                .into_any_element(),
            Some(percent) => div()
                .h_full()
                .w(relative(percent as f32 / 100.))
                .bg(hex(BLUE_600))
                .into_any_element(),
        };
        Some(
            deferred(
                backdrop(p.overlay).child(
                    div()
                        .id("progress")
                        .role(Role::AlertDialog)
                        .occlude()
                        .mx(px(16.))
                        .w_full()
                        .max_w(px(448.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.dialog)
                        .p(px(20.))
                        .shadow(shadow_2xl())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .ts(BASE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.fg)
                                .child(spinner(16., p.fg))
                                .child(t(cx, "form.saving")),
                        )
                        .child(
                            div()
                                .mt(px(16.))
                                .h(px(8.))
                                .overflow_hidden()
                                .rounded_full()
                                .bg(p.subtle)
                                .child(bar),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .flex()
                                .items_baseline()
                                .justify_between()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .ts(SM)
                                        .text_color(p.fg3)
                                        .child(label),
                                )
                                .children(percent.map(|percent| {
                                    div()
                                        .flex_none()
                                        .font_family(MONO_FONT)
                                        .ts(SM)
                                        .text_color(p.fg4)
                                        .child(format!("{percent}%"))
                                })),
                        ),
                ),
            )
            .with_priority(3),
        )
    }
}

impl Render for ProfileForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window, cx);
        self.sync_root_selects(cx);
        let close_hover = p.fg2;
        let connected = self.connected_platforms();
        let (link, link_hover) = (p.link, p.link_hover);
        let mut body = div().p(px(24.)).flex().flex_col().gap(px(16.)).child(
            div()
                .child(
                    div()
                        .mb(px(4.))
                        .ts(SM)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.fg3)
                        .child(t(cx, "form.profileName")),
                )
                .child(field(&self.name, Field::Md, &p, window, cx)),
        );
        if !self.is_edit() {
            body = body.child(
                div().flex().child(
                    control("import", &p)
                        .ts(XS)
                        .text_color(link)
                        .hover(move |s| s.text_color(link_hover).underline())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.import_from_git(window, cx)),
                        )
                        .child(t(cx, "form.importFromGit")),
                ),
            );
        }
        for platform in PLATFORMS {
            body = body.child(self.platform_section(platform, &p, window, cx));
        }
        body = body.children(self.default_identity(&p, cx));
        if !connected.is_empty() {
            body = body.child(self.repos_panel(&connected, &p, cx));
        }
        if !self.error.is_empty() {
            body = body.child(
                div()
                    .rounded(px(6.))
                    .bg(p.danger_bg)
                    .p(px(12.))
                    .ts(SM)
                    .text_color(p.danger_fg)
                    .child(self.error.clone()),
            );
        }
        let save_label = if self.saving {
            t(cx, "form.saving")
        } else if self.is_edit() {
            t(cx, "form.saveChanges")
        } else {
            t(cx, "form.createProfile")
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(p.border)
                    .px(px(24.))
                    .py(px(16.))
                    .child(
                        div()
                            .ts(LG)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.fg)
                            .child(t(
                                cx,
                                if self.is_edit() {
                                    "form.editTitle"
                                } else {
                                    "form.newTitle"
                                },
                            )),
                    )
                    .child(
                        control("close", &p)
                            .group("close")
                            .aria_label(t(cx, "form.cancel"))
                            .tooltip(tip(&t(cx, "form.cancel"), &p))
                            .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx)))
                            .child(
                                icon(CLOSE, 20., p.fg4)
                                    .group_hover("close", move |s| s.text_color(close_hover)),
                            ),
                    ),
            )
            .child(scroll_area("form", &self.scroll, &p, body))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_end()
                    .gap(px(12.))
                    .border_t_1()
                    .border_color(p.border)
                    .px(px(24.))
                    .py(px(16.))
                    .child(
                        btn_subtle("cancel", &p)
                            .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx)))
                            .child(t(cx, "form.cancel")),
                    )
                    .child(
                        btn_primary("save", &p, self.saving)
                            .gap(px(8.))
                            .when(self.saving, |b| b.child(spinner(14., white())))
                            .when(!self.saving, |b| {
                                b.on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                            })
                            .child(save_label),
                    ),
            )
            .children(self.disconnect_dialog(&p, cx))
            .children(self.progress_overlay(&p, cx))
    }
}
