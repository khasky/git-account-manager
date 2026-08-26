mod commands;
mod git;
mod guard;
mod http;
mod models;
mod oauth;
mod openssh_integration;
mod platform;
mod proc;
mod repos;
mod secrets;
mod ssh;
mod storage;
mod tray;

use tauri::Emitter;
use tauri::Manager;

use tray::{TrayLabels, TRAY_ID};

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
                tray::build_menu(app.handle(), &initial_profiles, &TrayLabels::default())?;
            let initial_tooltip = tray::tooltip(&initial_profiles);

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
                            if commands::activate_profile_core(profile_id).is_ok() {
                                tray::refresh(app);
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
            commands::get_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::activate_profile,
            commands::generate_ssh_key,
            commands::list_ssh_keys,
            commands::read_public_key,
            commands::delete_ssh_keys,
            commands::remove_ssh_key_from_platform,
            commands::generate_and_upload_key,
            commands::connect_bitbucket,
            commands::upload_ssh_key_to_platform,
            commands::delete_platform_token,
            commands::delete_profile_tokens,
            commands::github_oauth_start,
            commands::github_oauth_poll,
            commands::gitlab_oauth_connect,
            commands::gitlab_oauth_abort,
            commands::get_settings,
            commands::save_settings,
            commands::openssh_integration_probe,
            commands::get_git_identity,
            commands::set_tray_labels,
            commands::get_repo_state,
            commands::save_guard_settings,
            commands::scan_profile_repositories,
            commands::apply_profile_repos,
            commands::fix_repository,
            commands::allow_email_in_repository,
            commands::doctor,
            commands::probe_ssh_alias,
            commands::verify_repo_access,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
