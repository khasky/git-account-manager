//! The system-tray menu: the one place a profile can be switched without
//! opening the window.

use tauri::Manager;

use crate::models::Profile;
use crate::storage;

pub const TRAY_ID: &str = "main";

/// Localized labels for the tray menu.
///
/// The menu is rebuilt from scratch on every change — Tauri cannot patch an
/// individual item — and the translations live in the webview, so the frontend
/// pushes the active language's strings here via `set_tray_labels` and
/// `refresh` reads them back when assembling the menu.
#[derive(Clone)]
pub struct TrayLabels {
    pub show: String,
    pub quit: String,
    pub active_prefix: String,
    pub no_active: String,
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

/// Builds the full menu: a disabled header showing the active identity, one
/// clickable entry per profile (a check mark marks the active one; id
/// `activate:<profile-id>`), then Show and Quit.
pub fn build_menu(
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

pub fn tooltip(profiles: &[Profile]) -> String {
    match profiles.iter().find(|p| p.is_active) {
        Some(p) => format!("Git Account Manager \u{2014} {}", p.name),
        None => "Git Account Manager".to_string(),
    }
}

/// Rebuilds the menu and tooltip from current state. Idempotent; call after any
/// change to profiles or to the active identity.
pub fn refresh(app: &tauri::AppHandle) {
    let profiles = storage::load_state()
        .map(|s| s.profiles)
        .unwrap_or_default();
    let labels = app
        .state::<std::sync::Mutex<TrayLabels>>()
        .lock()
        .map(|l| l.clone())
        .unwrap_or_default();

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app, &profiles, &labels) {
            let _ = tray.set_menu(Some(menu));
        }
        let _ = tray.set_tooltip(Some(tooltip(&profiles)));
    }
}
