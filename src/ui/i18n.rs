//! The interface strings, one JSON file per language under `locales/`.

use gpui_kit::Global;
use serde_json::Value;

/// Every language, with its name in its own script, in picker order.
pub const LANGUAGES: [(&str, &str, &str); 11] = [
    ("en", "English", include_str!("../../locales/en.json")),
    (
        "zh-Hans",
        "简体中文",
        include_str!("../../locales/zh-Hans.json"),
    ),
    (
        "zh-Hant",
        "繁體中文",
        include_str!("../../locales/zh-Hant.json"),
    ),
    ("ja", "日本語", include_str!("../../locales/ja.json")),
    ("ko", "한국어", include_str!("../../locales/ko.json")),
    ("es", "Español", include_str!("../../locales/es.json")),
    (
        "pt-BR",
        "Português (BR)",
        include_str!("../../locales/pt-BR.json"),
    ),
    ("ru", "Русский", include_str!("../../locales/ru.json")),
    ("fr", "Français", include_str!("../../locales/fr.json")),
    ("de", "Deutsch", include_str!("../../locales/de.json")),
    ("uk", "Українська", include_str!("../../locales/uk.json")),
];

pub struct I18n {
    pub lang: &'static str,
    messages: Value,
    fallback: Value,
}

impl Global for I18n {}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let (code, _, json) = LANGUAGES
            .iter()
            .find(|(code, _, _)| *code == lang)
            .unwrap_or(&LANGUAGES[0]);
        Self {
            lang: code,
            messages: serde_json::from_str(json).expect("locale files are valid JSON"),
            fallback: serde_json::from_str(LANGUAGES[0].2).expect("locale files are valid JSON"),
        }
    }

    /// The string at a dotted key, such as `form.https.label`.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        lookup(&self.messages, key)
            .or_else(|| lookup(&self.fallback, key))
            .unwrap_or(key)
    }

    /// `t` with `{name}` placeholders filled in.
    pub fn f(&self, key: &str, vars: &[(&str, &str)]) -> String {
        fmt(self.t(key), vars)
    }
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.')
        .try_fold(root, |node, part| node.get(part))
        .and_then(Value::as_str)
}

/// Replaces `{name}` tokens with their values; unknown ones stay as written.
pub fn fmt(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end)
                if after[..end]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && end > 0 =>
            {
                let name = &after[..end];
                match vars.iter().find(|(key, _)| *key == name) {
                    Some((_, value)) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The supported language closest to the system's, as the webview's
/// `navigator.language` was matched.
pub fn detect(system: &str) -> &'static str {
    let nav = system.to_lowercase().replace('_', "-");
    if nav.starts_with("zh") {
        return if nav.contains("tw") || nav.contains("hk") || nav.contains("hant") {
            "zh-Hant"
        } else {
            "zh-Hans"
        };
    }
    if nav.starts_with("pt") {
        return "pt-BR";
    }
    let base = nav.split('-').next().unwrap_or("");
    ["en", "ja", "ko", "es", "ru", "fr", "de", "uk"]
        .into_iter()
        .find(|code| *code == base)
        .unwrap_or("en")
}

/// The user's interface language as a BCP 47 tag, such as `ru-RU`.
#[cfg(target_os = "windows")]
pub fn system_language() -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
        fn LCIDToLocaleName(locale: u32, name: *mut u16, length: i32, flags: u32) -> i32;
    }
    let mut buffer = [0u16; 85];
    let length = unsafe {
        LCIDToLocaleName(
            u32::from(GetUserDefaultUILanguage()),
            buffer.as_mut_ptr(),
            buffer.len() as i32,
            0,
        )
    };
    if length > 1 {
        String::from_utf16_lossy(&buffer[..length as usize - 1])
    } else {
        "en".into()
    }
}

#[cfg(target_os = "macos")]
pub fn system_language() -> String {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en".into())
}

#[cfg(target_os = "linux")]
pub fn system_language() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty() && value != "C" && value != "POSIX")
        .and_then(|value| value.split('.').next().map(str::to_owned))
        .unwrap_or_else(|| "en".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    keys(child, &format!("{prefix}{key}."), out);
                }
            }
            _ => out.push(prefix.trim_end_matches('.').to_string()),
        }
    }

    #[test]
    fn every_language_has_exactly_the_english_keys_and_placeholders() {
        let english: Value = serde_json::from_str(LANGUAGES[0].2).unwrap();
        let mut expected = Vec::new();
        keys(&english, "", &mut expected);
        for (code, _, json) in LANGUAGES {
            let messages: Value = serde_json::from_str(json).unwrap();
            let mut found = Vec::new();
            keys(&messages, "", &mut found);
            assert_eq!(found, expected, "{code} keys differ from English");
            for key in &expected {
                let tokens = |text: &str| {
                    let mut tokens: Vec<String> = text
                        .split('{')
                        .skip(1)
                        .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string()))
                        .collect();
                    tokens.sort();
                    tokens
                };
                assert_eq!(
                    tokens(lookup(&messages, key).unwrap()),
                    tokens(lookup(&english, key).unwrap()),
                    "{code} {key} placeholders differ"
                );
            }
        }
    }

    #[test]
    fn placeholders_are_filled_and_unknown_ones_kept() {
        assert_eq!(fmt("Hi {name}, {x}", &[("name", "Ann")]), "Hi Ann, {x}");
        assert_eq!(fmt("{a}{a}", &[("a", "1")]), "11");
        assert_eq!(fmt("a { b", &[]), "a { b");
        assert_eq!(I18n::new("ru").t("app.newProfile"), "Новый профиль");
        assert_eq!(I18n::new("xx").lang, "en");
    }

    #[test]
    fn system_languages_map_like_the_browser_locale_did() {
        assert_eq!(detect("zh-TW"), "zh-Hant");
        assert_eq!(detect("zh-CN"), "zh-Hans");
        assert_eq!(detect("pt-PT"), "pt-BR");
        assert_eq!(detect("ru_RU"), "ru");
        assert_eq!(detect("de-AT"), "de");
        assert_eq!(detect("it-IT"), "en");
    }
}
