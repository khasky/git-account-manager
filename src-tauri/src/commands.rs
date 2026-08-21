//! Everything the webview can call.
//!
//! A command that changes state runs its whole load-mutate-save-sync sequence
//! inside `storage::with_lock`, so two overlapping commands cannot each write a
//! state built from the same stale read. See `storage::with_lock`.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::git::{self, GitIdentity};
use crate::models::{
    slugify, AppState, DeviceCodeResponse, GuardSettings, OAuthSettings, Platform, PlatformUser,
    Profile, RepoBinding, RepoRoot, SshKeyInfo, SshKeyPair, PLATFORMS,
};
use crate::tray::{self, TrayLabels};
use crate::{guard, oauth, openssh_integration, platform, repos, secrets, ssh, storage};

// -- profiles ---------------------------------------------------------------

#[tauri::command]
pub fn get_profiles() -> Result<Vec<Profile>, String> {
    storage::with_lock(|| Ok(storage::load_state()?.profiles))
}

#[tauri::command]
pub fn save_profile(app: tauri::AppHandle, mut profile: Profile) -> Result<(), String> {
    storage::with_lock(|| {
        let mut state = storage::load_state()?;
        let is_new = !state.profiles.iter().any(|p| p.id == profile.id);
        let has_active = state.profiles.iter().any(|p| p.is_active);

        if is_new && !has_active {
            profile.is_active = true;
        }

        if let Some(existing) = state.profiles.iter_mut().find(|p| p.id == profile.id) {
            delete_removed_platform_tokens(existing, &profile)?;
            *existing = profile;
        } else {
            state.profiles.push(profile);
        }
        storage::save_state(&state)?;
        sync_machine(&state)
    })?;
    tray::refresh(&app);
    Ok(())
}

/// Brings `~/.ssh/config`, the global identity and the generated `~/.gitconfig`
/// region in line with the stored state. With the global-identity fuse enabled
/// no machine-wide identity is written at all — repositories carry their own.
fn sync_machine(state: &AppState) -> Result<(), String> {
    ssh::update_ssh_config(&state.profiles, state.guard.own_bare_ssh_hosts)?;

    if !state.guard.unset_global_identity {
        if let Some(active) = state.profiles.iter().find(|p| p.is_active) {
            if let Some((name, email)) = active.active_identity() {
                git::set_global_identity(name, email)?;
            }
        }
    }

    guard::apply(&state.guard, &state.profiles, &state.repo_roots)
}

fn delete_removed_platform_tokens(existing: &Profile, next: &Profile) -> Result<(), String> {
    for platform in PLATFORMS {
        if existing.account(platform).is_some() && next.account(platform).is_none() {
            secrets::delete_token(&existing.id, platform)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn delete_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    storage::with_lock(|| {
        let mut state = storage::load_state()?;

        // A host key only goes once no profile left can still reach that platform.
        let hosts_to_clean: Vec<&str> = PLATFORMS
            .iter()
            .filter(|platform| {
                !state
                    .profiles
                    .iter()
                    .any(|p| p.id != id && p.account(**platform).is_some())
            })
            .map(|platform| platform.canonical_host())
            .collect();
        if !hosts_to_clean.is_empty() {
            ssh::clean_known_hosts(&hosts_to_clean);
        }

        state.profiles.retain(|p| p.id != id);
        // A binding whose profile is gone would leave a stale allow-list behind
        // that blocks every push, so the repository is released before the
        // profile drops.
        for binding in state.bindings.iter().filter(|b| b.profile_id == id) {
            repos::clear_binding(&binding.path).ok();
        }
        state.bindings.retain(|b| b.profile_id != id);
        state.repo_roots.retain(|r| r.profile_id != id);
        storage::save_state(&state)?;
        secrets::delete_profile_tokens(&id)?;
        sync_machine(&state)
    })?;
    tray::refresh(&app);
    Ok(())
}

/// Switching the active profile, without the app handle the tray callback has
/// no way to supply.
pub fn activate_profile_core(id: &str) -> Result<(), String> {
    storage::with_lock(|| {
        let mut state = storage::load_state()?;
        for p in &mut state.profiles {
            p.is_active = p.id == id;
        }
        storage::save_state(&state)?;
        sync_machine(&state)
    })
}

#[tauri::command]
pub fn activate_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    activate_profile_core(&id)?;
    tray::refresh(&app);
    Ok(())
}

// -- SSH keys ---------------------------------------------------------------

#[tauri::command]
pub fn generate_ssh_key(email: String, key_name: String) -> Result<SshKeyPair, String> {
    ssh::generate_key(&email, &key_name)
}

#[tauri::command]
pub fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, String> {
    ssh::list_keys()
}

#[tauri::command]
pub fn read_public_key(path: String) -> Result<String, String> {
    ssh::read_public_key(&path)
}

#[tauri::command]
pub fn delete_ssh_keys(paths: Vec<String>) -> Result<(), String> {
    for path in &paths {
        ssh::delete_key_pair(path)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_ssh_key_from_platform(
    platform: Platform,
    profile_id: String,
    public_key_path: String,
) -> Result<(), String> {
    let token = secrets::get_token(&profile_id, platform)?;
    let pub_key = ssh::read_public_key(&public_key_path)?;
    platform::delete_ssh_key_from_platform(platform, &token, &pub_key).await
}

/// This machine's name, reduced to something safe inside an SSH key filename.
fn hostname_slug_for_key() -> String {
    let raw = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    slugify(&raw)
}

#[tauri::command]
pub async fn generate_and_upload_key(
    platform: Platform,
    profile_id: String,
    username: String,
    email: String,
) -> Result<SshKeyPair, String> {
    let token = secrets::get_token(&profile_id, platform)?;
    let slug = slugify(&username);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pc_slug = hostname_slug_for_key();
    let key_name = format!(
        "id_ed25519_gam_{}_{}_{}_{}",
        pc_slug,
        platform.as_str(),
        slug,
        ts
    );

    let pair = ssh::generate_key(&email, &key_name)?;
    let pub_key = ssh::read_public_key(&pair.public_key_path)?;
    let title = format!("git-account-manager: {} ({})", username, platform.label());
    platform::upload_ssh_key(platform, &token, &title, &pub_key).await?;

    Ok(pair)
}

#[tauri::command]
pub async fn upload_ssh_key_to_platform(
    platform: Platform,
    profile_id: String,
    title: String,
    key_content: String,
) -> Result<(), String> {
    let token = secrets::get_token(&profile_id, platform)?;
    platform::upload_ssh_key(platform, &token, &title, &key_content).await
}

// -- accounts ---------------------------------------------------------------

#[tauri::command]
pub async fn connect_bitbucket(
    profile_id: String,
    email: String,
    api_token: String,
) -> Result<PlatformUser, String> {
    let token = format!("{}:{}", email.trim(), api_token.trim());
    let user = platform::verify_token(Platform::Bitbucket, &token).await?;
    secrets::set_token(&profile_id, Platform::Bitbucket, &token)?;
    Ok(user)
}

#[tauri::command]
pub fn delete_platform_token(profile_id: String, platform: Platform) -> Result<(), String> {
    secrets::delete_token(&profile_id, platform)
}

#[tauri::command]
pub fn delete_profile_tokens(profile_id: String) -> Result<(), String> {
    secrets::delete_profile_tokens(&profile_id)
}

#[tauri::command]
pub async fn github_oauth_start(client_id: String) -> Result<DeviceCodeResponse, String> {
    oauth::github_device_start(&client_id).await
}

#[tauri::command]
pub async fn github_oauth_poll(
    client_id: String,
    device_code: String,
    profile_id: String,
) -> Result<Option<PlatformUser>, String> {
    let Some(token) = oauth::github_device_poll(&client_id, &device_code).await? else {
        return Ok(None);
    };
    let user = platform::verify_token(Platform::Github, &token).await?;
    secrets::set_token(&profile_id, Platform::Github, &token)?;
    Ok(Some(user))
}

#[tauri::command]
pub fn gitlab_oauth_abort() {
    oauth::abort();
}

#[tauri::command]
pub async fn gitlab_oauth_connect(
    app: tauri::AppHandle,
    client_id: String,
    profile_id: String,
) -> Result<PlatformUser, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    oauth::register_cancel(cancel.clone());
    struct ClearSlotOnExit;
    impl Drop for ClearSlotOnExit {
        fn drop(&mut self) {
            oauth::clear_cancel_slot();
        }
    }
    let _clear_slot = ClearSlotOnExit;

    let (verifier, challenge) = oauth::generate_pkce();
    let state = oauth::generate_state();

    let port = oauth::GITLAB_CALLBACK_PORT;
    let listeners = oauth::bind_callback_listeners(port)?;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    let auth_url = oauth::build_gitlab_auth_url(&client_id, &redirect_uri, &challenge, &state);

    let _ = app.clipboard().write_text(auth_url.clone());

    open::that(&auth_url).map_err(|e| format!("Failed to open browser: {}", e))?;

    let cancel_for_wait = cancel.clone();
    let code = tokio::task::spawn_blocking(move || {
        oauth::wait_for_callback(listeners, cancel_for_wait, &state)
    })
    .await
    .map_err(|e| e.to_string())??;

    let token = oauth::gitlab_exchange_code(&client_id, &code, &redirect_uri, &verifier).await?;
    let user = platform::verify_token(Platform::Gitlab, &token).await?;
    secrets::set_token(&profile_id, Platform::Gitlab, &token)?;
    Ok(user)
}

// -- settings ---------------------------------------------------------------

#[tauri::command]
pub fn get_settings() -> Result<OAuthSettings, String> {
    storage::with_lock(|| {
        let mut oauth = storage::load_state()?.oauth;
        let defaults = OAuthSettings::default();
        if oauth.github_client_id.is_empty() {
            oauth.github_client_id = defaults.github_client_id;
        }
        if oauth.gitlab_client_id.is_empty() {
            oauth.gitlab_client_id = defaults.gitlab_client_id;
        }
        Ok(oauth)
    })
}

#[tauri::command]
pub fn save_settings(settings: OAuthSettings) -> Result<(), String> {
    // Not gated on the platform here: `openssh_integration` answers for that
    // itself and does nothing off Windows. Repeating the `cfg` at the call site
    // only made those functions look unused everywhere else.
    if settings.use_openssh_for_git_tools {
        openssh_integration::ensure_ssh_available()?;
    }

    storage::with_lock(|| {
        let mut state = storage::load_state()?;
        state.oauth = settings;
        storage::save_state(&state)?;
        openssh_integration::apply(state.oauth.use_openssh_for_git_tools)
    })
}

#[tauri::command]
pub fn openssh_integration_probe() -> openssh_integration::OpenSshIntegrationProbe {
    openssh_integration::probe()
}

#[tauri::command]
pub fn get_git_identity() -> Result<GitIdentity, String> {
    git::get_global_identity()
}

// -- repositories -----------------------------------------------------------

#[derive(serde::Serialize)]
pub struct RepoState {
    roots: Vec<RepoRoot>,
    bindings: Vec<RepoBinding>,
    guard: GuardSettings,
}

/// Runs blocking work off the main thread.
///
/// Tauri executes a synchronous command on the main thread, so anything that
/// shells out to Git holds the window frozen for as long as it takes — a scan
/// walks the disk, the doctor spawns several Git processes per repository.
/// Declaring the command `async` and handing the body to `spawn_blocking` keeps
/// the UI responsive while the work runs.
async fn off_main<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub struct DoctorReport {
    guard: guard::GuardStatus,
    repos: Vec<repos::RepoStatus>,
}

#[tauri::command]
pub async fn get_repo_state() -> Result<RepoState, String> {
    off_main(|| {
        storage::with_lock(|| {
            let state = storage::load_state()?;
            Ok(RepoState {
                roots: state.repo_roots,
                bindings: state.bindings,
                guard: state.guard,
            })
        })
    })
    .await
}

#[tauri::command]
pub async fn apply_profile_repos(plan: repos::RepoPlan) -> Result<repos::ApplyReport, String> {
    off_main(move || {
        storage::with_lock(move || {
            let mut state = storage::load_state()?;
            let report = repos::apply_plan(
                &state.profiles,
                &mut state.repo_roots,
                &mut state.bindings,
                plan,
            )?;
            storage::save_state(&state)?;
            guard::apply(&state.guard, &state.profiles, &state.repo_roots)?;
            Ok(report)
        })
    })
    .await
}

#[tauri::command]
pub fn save_guard_settings(settings: GuardSettings) -> Result<(), String> {
    storage::with_lock(|| {
        let mut state = storage::load_state()?;
        let was_fused = state.guard.unset_global_identity;
        state.guard = settings;
        storage::save_state(&state)?;
        // Releasing the fuse is only correct as an explicit switch-off;
        // `sync_machine` must never undo it on its own, or a manual
        // `user.useConfigOnly` would be wiped on the next profile switch.
        if was_fused && !state.guard.unset_global_identity {
            guard::relax_global_identity()?;
        }
        sync_machine(&state)
    })
}

/// Scans one profile's folders while the profile is still being edited, so the
/// form's copy of it overrides what is on disk: a new profile is not in the
/// state at all, and an edited one may have just gained the account the evidence
/// ladder needs to recognise its own namespace.
#[tauri::command]
pub fn scan_profile_repositories(
    profile: Profile,
    roots: Vec<RepoRoot>,
) -> Result<Vec<repos::DiscoveredRepo>, String> {
    let state = storage::with_lock(storage::load_state)?;
    let mut profiles: Vec<Profile> = state
        .profiles
        .iter()
        .filter(|p| p.id != profile.id)
        .cloned()
        .collect();
    profiles.push(profile);
    Ok(repos::scan(&roots, &profiles, &state.bindings))
}

fn profile_by_id(state: &AppState, id: &str) -> Result<Profile, String> {
    state
        .profiles
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("Unknown profile: {}", id))
}

#[tauri::command]
pub async fn fix_repository(path: String) -> Result<repos::BindResult, String> {
    off_main(move || {
        storage::with_lock(move || {
            let mut state = storage::load_state()?;
            let profile_id = state
                .bindings
                .iter()
                .find(|b| b.path == path)
                .map(|b| b.profile_id.clone())
                .ok_or_else(|| format!("No binding for {}", path))?;
            let profile = profile_by_id(&state, &profile_id)?;
            let binding = state
                .bindings
                .iter_mut()
                .find(|b| b.path == path)
                .expect("looked up a moment ago");
            let result = repos::apply_binding(binding, &profile)?;
            // Re-applying can be what first pins the remote, and the address it
            // replaced is only recoverable if it is written down now.
            storage::save_state(&state)?;
            Ok(result)
        })
    })
    .await
}

/// Widens one repository's allow-list. Used to accept an address the history
/// check flagged — a bot, a co-author — without weakening any other repository.
#[tauri::command]
pub async fn allow_email_in_repository(
    path: String,
    email: String,
) -> Result<repos::BindResult, String> {
    off_main(move || {
        storage::with_lock(move || {
            let email = email.trim().to_string();
            if email.is_empty() {
                return Err("Email is empty".to_string());
            }

            let mut state = storage::load_state()?;
            let profile_id = state
                .bindings
                .iter()
                .find(|b| b.path == path)
                .map(|b| b.profile_id.clone())
                .ok_or_else(|| format!("No binding for {}", path))?;
            // Resolved before the mutable borrow: the profile is read from the
            // same state the binding lives in.
            let profile = profile_by_id(&state, &profile_id)?;

            let binding = state
                .bindings
                .iter_mut()
                .find(|b| b.path == path)
                .expect("looked up a moment ago");
            if !binding.extra_allowed_emails.contains(&email) {
                binding.extra_allowed_emails.push(email);
            }
            let result = repos::apply_binding(binding, &profile)?;
            storage::save_state(&state)?;
            Ok(result)
        })
    })
    .await
}

#[tauri::command]
pub async fn doctor() -> Result<DoctorReport, String> {
    off_main(|| {
        // The lock guards the state file, and reading it takes milliseconds; the
        // Git work that follows takes seconds and touches nothing shared. Holding
        // the lock across both made every other command queue behind the report.
        let state = storage::with_lock(storage::load_state)?;
        let repos = repos::inspect_all(&state.bindings, &state.profiles);
        Ok(DoctorReport {
            guard: guard::status(&state.guard),
            repos,
        })
    })
    .await
}

#[tauri::command]
pub async fn probe_ssh_alias(host: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || ssh::probe_host(&host))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn verify_repo_access(
    profile_id: String,
    platform: Platform,
    owner: String,
    repo: String,
) -> Result<repos::RepoReach, String> {
    let state = storage::with_lock(storage::load_state)?;
    let profile = state
        .profiles
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or("Profile not found")?;
    tokio::task::spawn_blocking(move || repos::reach(&profile, platform, &owner, &repo))
        .await
        .map_err(|e| e.to_string())
}

// -- tray -------------------------------------------------------------------

#[tauri::command]
pub fn set_tray_labels(
    app: tauri::AppHandle,
    show: String,
    quit: String,
    active_prefix: String,
    no_active: String,
    labels: tauri::State<'_, std::sync::Mutex<TrayLabels>>,
) -> Result<(), String> {
    {
        let mut l = labels.lock().map_err(|e| e.to_string())?;
        l.show = show;
        l.quit = quit;
        l.active_prefix = active_prefix;
        l.no_active = no_active;
    }
    tray::refresh(&app);
    Ok(())
}
