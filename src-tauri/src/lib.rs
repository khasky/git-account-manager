mod git;
mod models;
mod oauth;
mod openssh_integration;
mod platform;
mod secrets;
mod ssh;
mod storage;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

use git::GitIdentity;
use models::{DeviceCodeResponse, OAuthSettings, PlatformUser, Profile, SshKeyInfo, SshKeyPair};

// ---- Profile CRUD ----

#[tauri::command]
fn get_profiles() -> Result<Vec<Profile>, String> {
    Ok(storage::load_state()?.profiles)
}

#[tauri::command]
fn save_profile(app: tauri::AppHandle, mut profile: Profile) -> Result<(), String> {
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
    ssh::update_ssh_config(&state.profiles)?;

    if let Some(active) = state.profiles.iter().find(|p| p.is_active) {
        if let Some((name, email)) = active.active_identity() {
            git::set_global_identity(name, email)?;
        }
    }
    refresh_tray(&app);
    Ok(())
}

fn delete_removed_platform_tokens(existing: &Profile, next: &Profile) -> Result<(), String> {
    for platform in ["github", "gitlab", "bitbucket"] {
        if has_platform(existing, platform) && !has_platform(next, platform) {
            secrets::delete_token(&existing.id, platform)?;
        }
    }
    Ok(())
}

fn has_platform(profile: &Profile, platform: &str) -> bool {
    match platform {
        "github" => profile.github.is_some(),
        "gitlab" => profile.gitlab.is_some(),
        "bitbucket" => profile.bitbucket.is_some(),
        _ => false,
    }
}

#[tauri::command]
fn delete_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let state = storage::load_state()?;
    let has_github_remaining = state
        .profiles
        .iter()
        .any(|p| p.id != id && p.github.is_some());
    let has_gitlab_remaining = state
        .profiles
        .iter()
        .any(|p| p.id != id && p.gitlab.is_some());
    let has_bitbucket_remaining = state
        .profiles
        .iter()
        .any(|p| p.id != id && p.bitbucket.is_some());

    let mut hosts_to_clean: Vec<&str> = Vec::new();
    if !has_github_remaining {
        hosts_to_clean.push("github.com");
    }
    if !has_gitlab_remaining {
        hosts_to_clean.push("gitlab.com");
    }
    if !has_bitbucket_remaining {
        hosts_to_clean.push("bitbucket.org");
    }
    if !hosts_to_clean.is_empty() {
        ssh::clean_known_hosts(&hosts_to_clean).ok();
    }

    let mut state = state;
    state.profiles.retain(|p| p.id != id);
    storage::save_state(&state)?;
    secrets::delete_profile_tokens(&id)?;
    ssh::update_ssh_config(&state.profiles)?;
    refresh_tray(&app);
    Ok(())
}

fn activate_profile_core(id: &str) -> Result<(), String> {
    let mut state = storage::load_state()?;
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

#[tauri::command]
fn activate_profile(app: tauri::AppHandle, id: String) -> Result<(), String> {
    activate_profile_core(&id)?;
    refresh_tray(&app);
    Ok(())
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
    profile_id: String,
    public_key_path: String,
) -> Result<(), String> {
    let token = secrets::get_token(&profile_id, &platform)?;
    let pub_key = ssh::read_public_key(&public_key_path)?;
    platform::delete_ssh_key_from_platform(&platform, &token, &pub_key).await
}

/// Lowercase hostname safe for SSH key filenames (alphanumeric + hyphens).
fn hostname_slug_for_key() -> String {
    let raw = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let s = raw
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() {
        "unknown".to_string()
    } else {
        s
    }
}

#[tauri::command]
async fn generate_and_upload_key(
    platform: String,
    profile_id: String,
    username: String,
    email: String,
) -> Result<SshKeyPair, String> {
    let token = secrets::get_token(&profile_id, &platform)?;
    let slug = username.to_lowercase().replace(' ', "-");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pc_slug = hostname_slug_for_key();
    let key_name = format!("id_ed25519_gam_{}_{}_{}_{}", pc_slug, platform, slug, ts);

    let pair = ssh::generate_key(&email, &key_name)?;
    let pub_key = ssh::read_public_key(&pair.public_key_path)?;
    let title = format!("git-account-manager: {} ({})", username, platform);
    platform::upload_ssh_key(&platform, &token, &title, &pub_key).await?;

    Ok(pair)
}

// ---- Platform Verification ----

#[tauri::command]
async fn connect_bitbucket(
    profile_id: String,
    email: String,
    api_token: String,
) -> Result<PlatformUser, String> {
    let token = format!("{}:{}", email.trim(), api_token.trim());
    let user = platform::verify_token("bitbucket", &token).await?;
    secrets::set_token(&profile_id, "bitbucket", &token)?;
    Ok(user)
}

#[tauri::command]
async fn upload_ssh_key_to_platform(
    platform: String,
    profile_id: String,
    title: String,
    key_content: String,
) -> Result<(), String> {
    let token = secrets::get_token(&profile_id, &platform)?;
    platform::upload_ssh_key(&platform, &token, &title, &key_content).await
}

#[tauri::command]
fn delete_platform_token(profile_id: String, platform: String) -> Result<(), String> {
    secrets::delete_token(&profile_id, &platform)
}

#[tauri::command]
fn delete_profile_tokens(profile_id: String) -> Result<(), String> {
    secrets::delete_profile_tokens(&profile_id)
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
    profile_id: String,
) -> Result<Option<PlatformUser>, String> {
    let Some(token) = oauth::github_device_poll(&client_id, &device_code).await? else {
        return Ok(None);
    };
    let user = platform::verify_token("github", &token).await?;
    secrets::set_token(&profile_id, "github", &token)?;
    Ok(Some(user))
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
async fn gitlab_oauth_connect(
    app: tauri::AppHandle,
    client_id: String,
    profile_id: String,
) -> Result<PlatformUser, String> {
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
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).map_err(|e| {
        format!(
            "Cannot bind to port {} (is the app already running?): {}",
            port, e
        )
    })?;
    let redirect_uri = format!("http://localhost:{}/callback", port);

    let auth_url = oauth::build_gitlab_auth_url(&client_id, &redirect_uri, &challenge);

    let _ = app.clipboard().write_text(auth_url.clone());

    open::that(&auth_url).map_err(|e| format!("Failed to open browser: {}", e))?;

    let cancel_for_wait = cancel.clone();
    let code =
        tokio::task::spawn_blocking(move || oauth::wait_for_callback(listener, cancel_for_wait))
            .await
            .map_err(|e| e.to_string())??;

    let token = oauth::gitlab_exchange_code(&client_id, &code, &redirect_uri, &verifier).await?;
    let user = platform::verify_token("gitlab", &token).await?;
    secrets::set_token(&profile_id, "gitlab", &token)?;
    Ok(user)
}

// ---- Settings ----

#[tauri::command]
fn get_settings() -> Result<OAuthSettings, String> {
    let mut oauth = storage::load_state()?.oauth;
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
    #[cfg(windows)]
    if settings.use_openssh_for_git_tools {
        openssh_integration::ensure_ssh_available()?;
    }

    let mut state = storage::load_state()?;
    state.oauth = settings;
    storage::save_state(&state)?;

    #[cfg(windows)]
    openssh_integration::apply(state.oauth.use_openssh_for_git_tools)?;

    Ok(())
}

#[tauri::command]
fn openssh_integration_probe() -> openssh_integration::OpenSshIntegrationProbe {
    openssh_integration::probe()
}

// ---- Git Identity ----

#[tauri::command]
fn get_git_identity() -> Result<GitIdentity, String> {
    git::get_global_identity()
}

// ---- Tray ----

// Localized labels for the tray menu. The menu is rebuilt from scratch on every
// change (Tauri cannot patch individual items) and the translations live in the
// webview, so the frontend pushes the active language's strings here via
// `set_tray_labels`; `refresh_tray` reads them when assembling the menu.
#[derive(Clone)]
struct TrayLabels {
    show: String,
    quit: String,
    active_prefix: String,
    no_active: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            show: "Show Window".to_string(),
            quit: "Close Git Account Manager".to_string(),
            active_prefix: "Active:".to_string(),
            no_active: "No active profile".to_string(),
        }
    }
}

const TRAY_ID: &str = "main";

/// Builds the full tray menu: a disabled header showing the active identity, one
/// clickable entry per profile (a check mark marks the active one; id
/// `activate:<profile-id>`), then Show and Quit.
fn build_tray_menu(
    app: &tauri::AppHandle,
    profiles: &[Profile],
    labels: &TrayLabels,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

    let menu = Menu::new(app)?;

    let active = profiles.iter().find(|p| p.is_active);
    let header_text = match active {
        Some(p) => match p.active_identity() {
            Some((name, email)) => format!("{} {} <{}>", labels.active_prefix, name, email),
            None => format!("{} {}", labels.active_prefix, p.name),
        },
        None => labels.no_active.clone(),
    };
    let header = MenuItem::with_id(app, "tray_header", header_text, false, None::<&str>)?;
    menu.append(&header)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    for p in profiles {
        let label = if p.is_active {
            format!("\u{2713} {}", p.name)
        } else {
            format!("   {}", p.name)
        };
        let item = MenuItem::with_id(app, format!("activate:{}", p.id), label, true, None::<&str>)?;
        menu.append(&item)?;
    }
    if !profiles.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    let show = MenuItem::with_id(app, "show", &labels.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?;
    menu.append(&show)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;

    Ok(menu)
}

fn tray_tooltip(profiles: &[Profile]) -> String {
    match profiles.iter().find(|p| p.is_active) {
        Some(p) => format!("Git Account Manager \u{2014} {}", p.name),
        None => "Git Account Manager".to_string(),
    }
}

/// Rebuilds the tray menu and tooltip from current state. Idempotent; call after
/// any change to profiles or to the active identity.
fn refresh_tray(app: &tauri::AppHandle) {
    let profiles = storage::load_state()
        .map(|s| s.profiles)
        .unwrap_or_default();
    let labels = app
        .state::<std::sync::Mutex<TrayLabels>>()
        .lock()
        .map(|l| l.clone())
        .unwrap_or_default();

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_tray_menu(app, &profiles, &labels) {
            let _ = tray.set_menu(Some(menu));
        }
        let _ = tray.set_tooltip(Some(tray_tooltip(&profiles)));
    }
}

#[tauri::command]
fn set_tray_labels(
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
    refresh_tray(&app);
    Ok(())
}

// ---- App Entry ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Enforce a single running instance. The plugin must be registered before
    // any other so a second launch is rejected immediately; that second process
    // exits and this callback fires on the already-running instance, where we
    // restore and focus the window (it may be hidden in the tray or minimized).
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            #[cfg(windows)]
            {
                let openssh_enabled = storage::load_state()
                    .map(|s| s.oauth.use_openssh_for_git_tools)
                    .unwrap_or(false);
                if openssh_enabled {
                    let _ = openssh_integration::apply(true);
                }
            }

            use tauri::tray::TrayIconBuilder;

            app.manage(std::sync::Mutex::new(TrayLabels::default()));

            let initial_profiles = storage::load_state()
                .map(|s| s.profiles)
                .unwrap_or_default();
            let initial_menu =
                build_tray_menu(app.handle(), &initial_profiles, &TrayLabels::default())?;
            let initial_tooltip = tray_tooltip(&initial_profiles);

            let _tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/32x32.png"
                ))?)
                .menu(&initial_menu)
                .tooltip(initial_tooltip)
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
                    "tray_header" => {}
                    other => {
                        if let Some(profile_id) = other.strip_prefix("activate:") {
                            if activate_profile_core(profile_id).is_ok() {
                                refresh_tray(app);
                                let _ = app.emit("profiles-changed", ());
                            }
                        }
                    }
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
            connect_bitbucket,
            upload_ssh_key_to_platform,
            delete_platform_token,
            delete_profile_tokens,
            github_oauth_start,
            github_oauth_poll,
            gitlab_oauth_connect,
            gitlab_oauth_abort,
            get_settings,
            save_settings,
            openssh_integration_probe,
            get_git_identity,
            set_tray_labels,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
