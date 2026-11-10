//! The stylesheet's colours and the preferences that pick between them.

use std::path::PathBuf;

use gpui_kit::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// What the webview kept in its local storage: the theme and the language.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub theme: Theme,
    /// Unset until the user picks one; until then the system language decides.
    #[serde(default)]
    pub language: Option<String>,
}

impl Global for Prefs {}

fn prefs_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("git-account-manager")
        .join("ui.json")
}

impl Prefs {
    pub fn load() -> Self {
        std::fs::read_to_string(prefs_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = prefs_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

pub fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

pub fn hexa(value: u32) -> Hsla {
    rgba(value).into()
}

/// A colour at a fraction of its opacity, as Tailwind's `/15` does.
pub fn alpha(color: Hsla, a: f32) -> Hsla {
    color.opacity(a)
}

/// Tailwind's palette, for the classes the stylesheet used directly.
pub const BLUE_500: u32 = 0x2b7fff;
pub const BLUE_600: u32 = 0x155dfc;
pub const EMERALD_100: u32 = 0xd0fae5;
pub const EMERALD_500: u32 = 0x00bc7d;
pub const EMERALD_600: u32 = 0x009966;
pub const RED_400: u32 = 0xff6467;
pub const RED_500: u32 = 0xfb2c36;
pub const RED_600: u32 = 0xe7000b;
pub const AMBER_400: u32 = 0xffb900;
pub const AMBER_500: u32 = 0xfe9a00;
pub const AMBER_600: u32 = 0xe17100;
pub const AMBER_700: u32 = 0xbb4d00;

/// `[data-theme]` in the stylesheet, plus the `dark:` variants it used.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Hsla,
    pub raised: Hsla,
    pub raised_60: Hsla,
    pub raised_40: Hsla,
    pub subtle: Hsla,
    pub hover: Hsla,
    pub fg: Hsla,
    pub fg2: Hsla,
    pub fg3: Hsla,
    pub fg4: Hsla,
    pub fg5: Hsla,
    pub fg6: Hsla,
    pub border: Hsla,
    pub border_s: Hsla,
    pub scrollbar: Hsla,
    pub overlay: Hsla,
    pub dialog: Hsla,
    pub input: Hsla,
    pub link: Hsla,
    pub link_hover: Hsla,
    pub active_bg: Hsla,
    pub active_border: Hsla,
    pub selected_bg: Hsla,
    pub selected_border: Hsla,
    pub selected_fg: Hsla,
    pub info_bg: Hsla,
    pub info_border: Hsla,
    pub danger_bg: Hsla,
    pub danger_hover: Hsla,
    pub danger_fg: Hsla,
    pub success_fg: Hsla,
    pub success_icon: Hsla,
    pub badge_ok_bg: Hsla,
    pub badge_ok_fg: Hsla,
    pub toggle_off: Hsla,
    /// `text-amber-600 dark:text-amber-400`: warnings.
    pub warn: Hsla,
    /// `text-red-600 dark:text-red-400`: failures outside the danger tokens.
    pub fail: Hsla,
    /// `text-amber-700 dark:text-amber-400`: the issues pill.
    pub issues: Hsla,
    /// The native checkbox when unchecked, per `color-scheme`.
    pub check_bg: Hsla,
    pub check_border: Hsla,
}

pub fn is_dark(theme: Theme, window: &Window) -> bool {
    match theme {
        Theme::Light => false,
        Theme::Dark => true,
        Theme::System => matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: hex(0x0d1117),
                raised: hex(0x161b22),
                raised_60: hexa(0x161b2299),
                raised_40: hexa(0x161b2266),
                subtle: hex(0x21262d),
                hover: hex(0x30363d),
                fg: hex(0xe6edf3),
                fg2: hex(0xc9d1d9),
                fg3: hex(0xb1bac4),
                fg4: hex(0x8b949e),
                fg5: hex(0x6e7681),
                fg6: hex(0x484f58),
                border: hex(0x30363d),
                border_s: hex(0x484f58),
                scrollbar: hex(0x30363d),
                overlay: hexa(0x00000099),
                dialog: hex(0x0d1117),
                input: hex(0x0d1117),
                link: hex(0x58a6ff),
                link_hover: hex(0x79c0ff),
                active_bg: hexa(0x064e3b4d),
                active_border: hexa(0x34d39980),
                selected_bg: hexa(0x1e3a8a4d),
                selected_border: hexa(0x60a5fa80),
                selected_fg: hex(0xbfdbfe),
                info_bg: hexa(0x17255466),
                info_border: hex(0x1e3a5f),
                danger_bg: hexa(0x7f1d1d4d),
                danger_hover: hexa(0x7f1d1d80),
                danger_fg: hex(0xf87171),
                success_fg: hex(0x6ee7b7),
                success_icon: hex(0x34d399),
                badge_ok_bg: hexa(0x065f4699),
                badge_ok_fg: hex(0x6ee7b7),
                toggle_off: hex(0x484f58),
                warn: hex(AMBER_400),
                fail: hex(RED_400),
                issues: hex(AMBER_400),
                check_bg: hex(0x3b3b3b),
                check_border: hex(0x858585),
            }
        } else {
            Self {
                bg: hex(0xffffff),
                raised: hex(0xf6f8fa),
                raised_60: hexa(0xeaeef2cc),
                raised_40: hexa(0xeaeef299),
                subtle: hex(0xe1e4e8),
                hover: hex(0xd0d7de),
                fg: hex(0x1f2328),
                fg2: hex(0x32383f),
                fg3: hex(0x57606a),
                fg4: hex(0x656d76),
                fg5: hex(0x8b949e),
                fg6: hex(0xafb8c1),
                border: hex(0xd0d7de),
                border_s: hex(0xafb8c1),
                scrollbar: hex(0xc0c8d0),
                overlay: hexa(0x0000004d),
                dialog: hex(0xffffff),
                input: hex(0xffffff),
                link: hex(0x0969da),
                link_hover: hex(0x0550ae),
                active_bg: hexa(0xdafbe199),
                active_border: hexa(0x16a34a80),
                selected_bg: hexa(0xddf4ff99),
                selected_border: hexa(0x3b82f680),
                selected_fg: hex(0x1e40af),
                info_bg: hex(0xdbeafe),
                info_border: hex(0x93c5fd),
                danger_bg: hex(0xfee2e2),
                danger_hover: hex(0xfecaca),
                danger_fg: hex(0xcf222e),
                success_fg: hex(0x116329),
                success_icon: hex(0x1a7f37),
                badge_ok_bg: hex(0xd1fae5),
                badge_ok_fg: hex(0x065f46),
                toggle_off: hex(0xafb8c1),
                warn: hex(AMBER_600),
                fail: hex(RED_600),
                issues: hex(AMBER_700),
                check_bg: hex(0xffffff),
                check_border: hex(0x767676),
            }
        }
    }
}
