//! Everything the webview can call.
//!
//! A command that changes state runs its whole load-mutate-save-sync sequence
//! inside `storage::with_lock`, so two overlapping commands cannot each write a
//! state built from the same stale read. See `storage::with_lock`.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::Emitter;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::git::{self, GitIdentity};
use crate::models::{
    slugify, AppState, DeviceCodeResponse, GuardSettings, OAuthSettings, Platform, PlatformUser,
    Profile, RepoRoot, SshKeyInfo, SshKeyPair, PLATFORMS,
};
use crate::tray::{self, TrayLabels};
use crate::{
    credential, doctor, gh, guard, hooks, oauth, openssh_integration, platform, repos, secrets,
    ssh, storage,
};

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
/// region in line with the stored state. The active profile is the machine's
/// default, for every repository a folder rule does not claim.
fn sync_machine(state: &AppState) -> Result<(), String> {
    ssh::update_ssh_config(&state.profiles)?;

    let active = state.profiles.iter().find(|p| p.is_active);

    if state.oauth.switch_gh_account {
        if let Some(login) = active.and_then(|p| p.github.as_ref()).map(|a| &a.username) {
            // A missing gh, or a login it is not signed in to, is the expected
            // failure here and must not undo the switch of everything else;
            // the settings page shows which logins gh knows.
            let _ = gh::switch_account(login);
        }
    }

    // With the fuse armed there is no machine-wide identity to write: a
    // repository no folder rule claims is meant to refuse the commit.
    if let Some(active) = guard::machine_default(&state.guard, &state.profiles) {
        if let Some((name, email)) = active.active_identity() {
            git::set_global_identity(name, email)?;
        }
        // Follows the identity: signing with the profile that just stopped
        // being active would produce a signature the new author cannot own.
        git::set_global_signing(active.active_account().and_then(|a| a.signing_key()))?;
    }

    // A helper binary that is missing, or a host the new profile has no token
    // for, must not undo the switch of everything else; the settings page is
    // where that failure is reported.
    let _ = credential::apply(state);

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
        // The folders this profile claimed go with it. Their repositories keep
        // nothing of their own, so dropping the rule is the whole undo: they
        // fall back to the active profile like any unclaimed repository.
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
    sign: bool,
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

    let mut pair = ssh::generate_key(&email, &key_name)?;
    let pub_key = ssh::read_public_key(&pair.public_key_path)?;
    let title = format!("git-account-manager: {} ({})", username, platform.label());
    platform::upload_ssh_key(platform, &token, &title, &pub_key).await?;

    if sign {
        pair.signing_error = platform::upload_signing_key(platform, &token, &title, &pub_key)
            .await
            .err();
    }

    Ok(pair)
}

#[tauri::command]
pub async fn upload_ssh_key_to_platform(
    platform: Platform,
    profile_id: String,
    title: String,
    key_content: String,
    sign: bool,
) -> Result<Option<String>, String> {
    let token = secrets::get_token(&profile_id, platform)?;
    platform::upload_ssh_key(platform, &token, &title, &key_content).await?;

    if !sign {
        return Ok(None);
    }
    Ok(
        platform::upload_signing_key(platform, &token, &title, &key_content)
            .await
            .err(),
    )
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
    if settings.use_https_credential_helper {
        credential::ensure_helper_available()?;
    }

    storage::with_lock(|| {
        let mut state = storage::load_state()?;
        state.oauth = settings;
        storage::save_state(&state)?;
        openssh_integration::apply(state.oauth.use_openssh_for_git_tools)?;
        credential::apply(&state)
    })
}

// -- HTTPS credentials ------------------------------------------------------

/// The token git is handed for this account's HTTPS remotes.
///
/// Kept out of `save_profile`: like the Bitbucket connect form, a secret goes
/// straight to the credential store and never through the state file. An empty
/// value removes what was there, which is how the form clears a token.
#[tauri::command]
pub fn save_https_token(
    profile_id: String,
    platform: Platform,
    token: String,
) -> Result<(), String> {
    let token = token.trim().to_string();
    if token.is_empty() {
        secrets::delete_https_token(&profile_id, platform)?;
    } else {
        secrets::set_https_token(&profile_id, platform, &token)?;
    }

    storage::with_lock(|| credential::apply(&storage::load_state()?))
}

/// Which of a profile's platforms hold an HTTPS token, for a form that shows
/// whether one is set without reading it back out.
#[tauri::command]
pub fn https_token_platforms(profile_id: String) -> Result<Vec<Platform>, String> {
    let mut set = Vec::new();
    for platform in PLATFORMS {
        if secrets::get_https_token(&profile_id, platform)?.is_some() {
            set.push(platform);
        }
    }
    Ok(set)
}

#[tauri::command]
pub fn gh_probe() -> gh::GhProbe {
    gh::probe()
}

#[tauri::command]
pub fn openssh_integration_probe() -> openssh_integration::OpenSshIntegrationProbe {
    openssh_integration::probe()
}

#[tauri::command]
pub fn get_git_identity() -> Result<GitIdentity, String> {
    git::get_global_identity()
}

// -- folders ----------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct RepoState {
    roots: Vec<RepoRoot>,
    guard: GuardSettings,
}

/// Runs blocking work off the main thread.
///
/// Tauri executes a synchronous command on the main thread, so anything that
/// shells out to Git holds the window frozen for as long as it takes — a scan
/// walks the disk, the doctor spawns several Git processes per folder.
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

#[tauri::command]
pub async fn get_repo_state() -> Result<RepoState, String> {
    off_main(|| {
        storage::with_lock(|| {
            let state = storage::load_state()?;
            Ok(RepoState {
                roots: state.repo_roots,
                guard: state.guard,
            })
        })
    })
    .await
}

/// Everything the folders would cover, for the form to show before anything is
/// saved. The profile being edited overrides what is on disk: a new one is not
/// in the state at all, and an edited one may have just gained the account whose
/// host decides whether a repository sits under a foreign platform.
#[tauri::command]
pub async fn scan_profile_folders(
    profile: Profile,
    roots: Vec<RepoRoot>,
) -> Result<Vec<repos::FolderRepo>, String> {
    off_main(move || {
        let state = storage::with_lock(storage::load_state)?;
        let mut profiles: Vec<Profile> = state
            .profiles
            .iter()
            .filter(|p| p.id != profile.id)
            .cloned()
            .collect();
        profiles.push(profile);
        Ok(repos::scan(&roots, &profiles))
    })
    .await
}

#[derive(serde::Serialize)]
pub struct SaveFoldersReport {
    pub folders: usize,
    pub repos: usize,
}

/// The event a running save reports itself on, so the window can hold the user
/// out of a form whose folders are still being read.
pub const SAVE_PROGRESS_EVENT: &str = "save-progress";

/// How far a save has got. The counts travel as they are rather than as a
/// percentage: which share of the bar a folder owns is the window's to decide,
/// and only the window knows the wording for the step being reported.
#[derive(Clone, serde::Serialize)]
pub struct SaveProgress {
    /// `scan` while a folder's repositories are read, `apply` once the state is
    /// written and the git rules go out.
    pub stage: &'static str,
    pub folder: String,
    pub folder_index: usize,
    pub folder_count: usize,
    pub repos_done: usize,
    pub repos_total: usize,
}

/// Replaces one profile's folders. Another profile's are left alone, so two
/// profiles edited in turn do not overwrite each other.
///
/// Each folder is scanned once here, and what was found is stored with it: that
/// list is how a folder is recognised again after it moves.
#[tauri::command]
pub async fn save_profile_folders(
    app: tauri::AppHandle,
    profile_id: String,
    roots: Vec<RepoRoot>,
) -> Result<SaveFoldersReport, String> {
    off_main(move || {
        storage::with_lock(move || {
            let mut state = storage::load_state()?;
            let mut repo_count = 0;
            let mut stored: Vec<RepoRoot> = Vec::new();
            let folder_count = roots.len();
            for (folder_index, mut root) in roots.into_iter().enumerate() {
                root.path = guard::normalize_folder(&root.path);
                // Two spellings of one folder normalize to the same path, and a
                // second rule for it would only repeat the first.
                if stored.iter().any(|r: &RepoRoot| r.path == root.path) {
                    continue;
                }
                // One folder cannot belong to two accounts: git would apply
                // whichever rule it read last, and which one that is depends on
                // nothing the user can see. Taking the other profile's folder
                // away silently would be worse, so the save says no instead.
                if let Some(other) = state
                    .repo_roots
                    .iter()
                    .find(|r| r.profile_id != profile_id && r.path == root.path)
                {
                    let owner = state
                        .profiles
                        .iter()
                        .find(|p| p.id == other.profile_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("another profile");
                    return Err(format!("{} already belongs to {}", root.path, owner));
                }
                root.profile_id = profile_id.clone();
                let mut report = |repos_done, repos_total| {
                    let _ = app.emit(
                        SAVE_PROGRESS_EVENT,
                        SaveProgress {
                            stage: "scan",
                            folder: root.path.clone(),
                            folder_index,
                            folder_count,
                            repos_done,
                            repos_total,
                        },
                    );
                };
                report(0, 0);
                let found = repos::scan_one_reporting(&root, &state.profiles, &mut report);
                repo_count += found.len();
                root.fingerprint = repos::fingerprint(&found);
                stored.push(root);
            }

            state.repo_roots.retain(|r| r.profile_id != profile_id);
            state.repo_roots.extend(stored);
            let folders = state
                .repo_roots
                .iter()
                .filter(|r| r.profile_id == profile_id)
                .count();
            let _ = app.emit(
                SAVE_PROGRESS_EVENT,
                SaveProgress {
                    stage: "apply",
                    folder: String::new(),
                    folder_index: folder_count,
                    folder_count,
                    repos_done: 0,
                    repos_total: 0,
                },
            );
            storage::save_state(&state)?;
            sync_machine(&state)?;
            Ok(SaveFoldersReport {
                folders,
                repos: repo_count,
            })
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
        // Releasing the fuse is only correct as an explicit switch-off:
        // `sync_machine` must never undo it on its own, or a `user.useConfigOnly`
        // set by hand would be wiped on the next profile switch.
        if was_fused && !state.guard.unset_global_identity {
            guard::relax_global_identity()?;
        }
        sync_machine(&state)?;
        if state.guard.guard_commits {
            hooks::ensure_in_force()?;
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn doctor() -> Result<doctor::DoctorReport, String> {
    off_main(|| {
        // The lock guards the state file, and reading it takes milliseconds; the
        // Git work that follows takes seconds and touches nothing shared. Holding
        // the lock across both made every other command queue behind the report.
        let state = storage::with_lock(storage::load_state)?;
        Ok(doctor::report(
            &state.guard,
            &state.profiles,
            &state.repo_roots,
        ))
    })
    .await
}

/// The cheap half of the doctor, for the window to ask on a timer: is each
/// folder still where its rule says, and is the rule still in force.
#[tauri::command]
pub async fn watch_folders() -> Result<Vec<doctor::FolderWatch>, String> {
    off_main(|| {
        let state = storage::with_lock(storage::load_state)?;
        Ok(doctor::watch(
            &state.guard,
            &state.profiles,
            &state.repo_roots,
        ))
    })
    .await
}

/// Where a folder went, asked only when something is about to be shown about
/// it: the search walks the disk, and the check on the timer must not.
#[tauri::command]
pub async fn locate_folder(path: String) -> Result<Option<String>, String> {
    off_main(move || {
        let state = storage::with_lock(storage::load_state)?;
        let Some(root) = state.repo_roots.iter().find(|r| r.path == path) else {
            return Ok(None);
        };
        Ok(doctor::find_moved(root, &state.profiles))
    })
    .await
}

/// Puts one folder back under its rule: the rule is rewritten, the guard is put
/// back in force, and the repositories under it stop carrying their own copy of
/// what the rule supplies. Answers with how many of them had to be cleaned.
#[tauri::command]
pub async fn fix_folder(path: String) -> Result<usize, String> {
    off_main(move || {
        storage::with_lock(move || {
            let state = storage::load_state()?;
            let root = state
                .repo_roots
                .iter()
                .find(|r| r.path == path)
                .ok_or_else(|| format!("No folder at {}", path))?;
            let cleaned = doctor::clean_overrides(root, &state.profiles)?;
            sync_machine(&state)?;
            Ok(cleaned)
        })
    })
    .await
}

/// Points a folder's rule at where the folder actually is now.
#[tauri::command]
pub async fn relink_folder(path: String, new_path: String) -> Result<(), String> {
    off_main(move || {
        storage::with_lock(move || {
            let mut state = storage::load_state()?;
            let profiles = state.profiles.clone();
            let root = state
                .repo_roots
                .iter_mut()
                .find(|r| r.path == path)
                .ok_or_else(|| format!("No folder at {}", path))?;
            root.path = guard::normalize_folder(&new_path);
            // Recorded against the new location: what is there now is what the
            // folder should be recognised by the next time it moves.
            let found = repos::scan_one(root, &profiles);
            root.fingerprint = repos::fingerprint(&found);
            storage::save_state(&state)?;
            sync_machine(&state)
        })
    })
    .await
}

#[tauri::command]
pub async fn forget_folder(path: String) -> Result<(), String> {
    off_main(move || {
        storage::with_lock(move || {
            let mut state = storage::load_state()?;
            state.repo_roots.retain(|r| r.path != path);
            storage::save_state(&state)?;
            sync_machine(&state)
        })
    })
    .await
}

/// Brings the window up for a finding the user did not go looking for. Done
/// here rather than from the page, which would need window permissions of its
/// own for the one thing it has to do from the background.
#[tauri::command]
pub fn focus_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
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
