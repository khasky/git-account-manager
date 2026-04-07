mod git;
mod models;
mod oauth;
mod platform;
mod ssh;
mod storage;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

use git::GitIdentity;
use models::{DeviceCodeResponse, OAuthSettings, PlatformUser, Profile, SshKeyInfo, SshKeyPair};

// ---- Profile CRUD ----

#[tauri::command]
fn get_profiles() -> Result<Vec<Profile>, String> {
    Ok(storage::load_state().profiles)
}

#[tauri::command]
fn save_profile(mut profile: Profile) -> Result<(), String> {
    let mut state = storage::load_state();
    let is_new = !state.profiles.iter().any(|p| p.id == profile.id);
    let has_active = state.profiles.iter().any(|p| p.is_active);

    if is_new && !has_active {
        profile.is_active = true;
    }

    if let Some(existing) = state.profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile;
    } else {
        state.profiles.push(profile);
    }
    storage::save_state(&state)?;
    ssh::update_ssh_config(&state.profiles)?;

    if let Some(active) = state.profiles.iter().find(|p| p.is_active) {
        if let Some((name, email)) = active.active_identity() {
            git::set_global_identity(name, email)?;
        }
    }
    Ok(())
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    let state = storage::load_state();
    let has_github_remaining = state.profiles.iter().any(|p| p.id != id && p.github.is_some());
    let has_gitlab_remaining = state.profiles.iter().any(|p| p.id != id && p.gitlab.is_some());

    let mut hosts_to_clean: Vec<&str> = Vec::new();
    if !has_github_remaining { hosts_to_clean.push("github.com"); }
    if !has_gitlab_remaining { hosts_to_clean.push("gitlab.com"); }
    if !hosts_to_clean.is_empty() {
        ssh::clean_known_hosts(&hosts_to_clean).ok();
    }

    let mut state = state;
    state.profiles.retain(|p| p.id != id);
    storage::save_state(&state)?;
    ssh::update_ssh_config(&state.profiles)
}

#[tauri::command]
fn activate_profile(id: String) -> Result<(), String> {
    let mut state = storage::load_state();
    for p in &mut state.profiles {
        p.is_active = p.id == id;
    }
    storage::save_state(&state)?;

    if let Some(active) = state.profiles.iter().find(|p| p.is_active) {
        if let Some((name, email)) = active.active_identity() {
            git::set_global_identity(name, email)?;
        }
    }
    ssh::update_ssh_config(&state.profiles)
}

// ---- SSH Keys ----

#[tauri::command]
fn generate_ssh_key(email: String, key_name: String) -> Result<SshKeyPair, String> {
    ssh::generate_key(&email, &key_name)
}

#[tauri::command]
fn list_ssh_keys() -> Result<Vec<SshKeyInfo>, String> {
    ssh::list_keys()
}

#[tauri::command]
fn read_public_key(path: String) -> Result<String, String> {
    ssh::read_public_key(&path)
}

#[tauri::command]
fn delete_ssh_keys(paths: Vec<String>) -> Result<(), String> {
    for path in &paths {
        ssh::delete_key_pair(path)?;
    }
    Ok(())
}

#[tauri::command]
async fn remove_ssh_key_from_platform(
    platform: String,
    token: String,
    public_key_path: String,
) -> Result<(), String> {
    let pub_key = ssh::read_public_key(&public_key_path)?;
    platform::delete_ssh_key_from_platform(&platform, &token, &pub_key).await
}

#[tauri::command]
async fn generate_and_upload_key(
    platform: String,
    token: String,
    username: String,
    email: String,
) -> Result<SshKeyPair, String> {
    let slug = username.to_lowercase().replace(' ', "-");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key_name = format!("id_ed25519_gam_{}_{}_{}", platform, slug, ts);

    let pair = ssh::generate_key(&email, &key_name)?;
    let pub_key = ssh::read_public_key(&pair.public_key_path)?;
    let title = format!("git-account-manager: {} ({})", username, platform);
    platform::upload_ssh_key(&platform, &token, &title, &pub_key).await?;

    Ok(pair)
}

// ---- Platform Verification ----

#[tauri::command]
async fn verify_platform_token(platform: String, token: String) -> Result<PlatformUser, String> {
    platform::verify_token(&platform, &token).await
}

#[tauri::command]
async fn upload_ssh_key_to_platform(
    platform: String,
    token: String,
    title: String,
    key_content: String,
) -> Result<(), String> {
    platform::upload_ssh_key(&platform, &token, &title, &key_content).await
}

// ---- OAuth: GitHub Device Flow ----

#[tauri::command]
async fn github_oauth_start(client_id: String) -> Result<DeviceCodeResponse, String> {
    oauth::github_device_start(&client_id).await
}

#[tauri::command]
async fn github_oauth_poll(
    client_id: String,
    device_code: String,
) -> Result<Option<String>, String> {
    oauth::github_device_poll(&client_id, &device_code).await
}

// ---- OAuth: GitLab PKCE ----

fn gitlab_oauth_cancel_slot() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn register_gitlab_oauth_cancel(flag: Arc<AtomicBool>) {
    if let Ok(mut g) = gitlab_oauth_cancel_slot().lock() {
        *g = Some(flag);
    }
}

fn clear_gitlab_oauth_cancel_slot() {
    if let Ok(mut g) = gitlab_oauth_cancel_slot().lock() {
        *g = None;
    }
}

#[tauri::command]
fn gitlab_oauth_abort() {
    if let Ok(guard) = gitlab_oauth_cancel_slot().lock() {
        if let Some(flag) = guard.as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

#[tauri::command]
async fn gitlab_oauth_connect(app: tauri::AppHandle, client_id: String) -> Result<String, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    register_gitlab_oauth_cancel(cancel.clone());
    struct ClearGitlabOauthSlot;
    impl Drop for ClearGitlabOauthSlot {
        fn drop(&mut self) {
            clear_gitlab_oauth_cancel_slot();
        }
    }
    let _clear_slot = ClearGitlabOauthSlot;

    let (verifier, challenge) = oauth::generate_pkce();

    let port = oauth::GITLAB_CALLBACK_PORT;
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|e| format!("Cannot bind to port {} (is the app already running?): {}", port, e))?;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    let auth_url = oauth::build_gitlab_auth_url(&client_id, &redirect_uri, &challenge);

    let _ = app.clipboard().write_text(auth_url.clone());

    open::that(&auth_url).map_err(|e| format!("Failed to open browser: {}", e))?;

    let cancel_for_wait = cancel.clone();
    let code = tokio::task::spawn_blocking(move || {
        oauth::wait_for_callback(listener, cancel_for_wait)
    })
    .await
    .map_err(|e| e.to_string())??;

    oauth::gitlab_exchange_code(&client_id, &code, &redirect_uri, &verifier).await
}

// ---- Settings ----

#[tauri::command]
fn get_settings() -> Result<OAuthSettings, String> {
    let mut oauth = storage::load_state().oauth;
    let defaults = OAuthSettings::default();
    if oauth.github_client_id.is_empty() {
        oauth.github_client_id = defaults.github_client_id;
    }
    if oauth.gitlab_client_id.is_empty() {
        oauth.gitlab_client_id = defaults.gitlab_client_id;
    }
    Ok(oauth)
}

#[tauri::command]
fn save_settings(settings: OAuthSettings) -> Result<(), String> {
    let mut state = storage::load_state();
    state.oauth = settings;
    storage::save_state(&state)
}

// ---- Git Identity ----

#[tauri::command]
fn get_git_identity() -> Result<GitIdentity, String> {
    git::get_global_identity()
}

// ---- App Entry ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
            use tauri::tray::TrayIconBuilder;

            let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit =
                MenuItem::with_id(app, "quit", "Close Git Account Manager", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &sep, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/32x32.png"
                ))?)
                .menu(&menu)
                .tooltip("Git Account Manager")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            save_profile,
            delete_profile,
            activate_profile,
            generate_ssh_key,
            list_ssh_keys,
            read_public_key,
            delete_ssh_keys,
            remove_ssh_key_from_platform,
            generate_and_upload_key,
            verify_platform_token,
            upload_ssh_key_to_platform,
            github_oauth_start,
            github_oauth_poll,
            gitlab_oauth_connect,
            gitlab_oauth_abort,
            get_settings,
            save_settings,
            get_git_identity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
