//! The window's root: the profile list and whichever page sits over it.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures::channel::mpsc::unbounded;
use futures::StreamExt;
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::form::{FormEvent, ProfileForm};
use super::settings::{SettingsEvent, SettingsPage};
use super::theme::{
    alpha, hex, Palette, Prefs, Theme, AMBER_500, BLUE_600, EMERALD_100, EMERALD_500, EMERALD_600,
};
use super::widgets::*;
use super::{blocking, f, palette, t, tokio};
use crate::commands::{self, SaveProgress};
use crate::doctor::FolderWatch;
use crate::git::GitIdentity;
use crate::models::{Platform, Profile, PLATFORMS};
use crate::updater::{self, Update};

const TOAST: Duration = Duration::from_secs(3);

/// How often the folders are checked while the window is open. The point is
/// that a folder someone moved in Explorer surfaces here rather than on the
/// next wrong commit.
const WATCH: Duration = Duration::from_secs(60);

pub const THEMES: [(Theme, &str, &str); 3] = [
    (Theme::Light, "theme.light", SUN),
    (Theme::Dark, "theme.dark", MOON),
    (Theme::System, "theme.system", MONITOR),
];

pub fn platform_icon(platform: Platform) -> &'static str {
    match platform {
        Platform::Github => GITHUB,
        Platform::Gitlab => GITLAB,
        Platform::Bitbucket => BITBUCKET,
        Platform::Azure => AZURE,
        Platform::Codeberg => CODEBERG,
        Platform::Huggingface => HUGGINGFACE,
    }
}

pub enum Page {
    List,
    Form(Entity<ProfileForm>),
    Settings(Entity<SettingsPage>),
}

#[derive(Clone)]
pub enum Banner {
    Hidden,
    Available(Update),
    Downloading(u32),
    Failed(String),
}

pub struct AppView {
    pub profiles: Vec<Profile>,
    pub loading: bool,
    pub problems: HashMap<String, usize>,
    /// Each finding with where a missing folder appears to have gone.
    pub watched: Vec<(FolderWatch, Option<String>)>,
    pub alert_open: bool,
    alert_busy: String,
    /// What has already been raised, so a problem the user chose to leave
    /// alone does not reopen the dialog every minute.
    announced: HashSet<String>,
    pub toast: Option<String>,
    toast_timer: Option<Task<()>>,
    pub theme_open: bool,
    pub page: Page,
    pub banner: Banner,
    delete_target: Option<String>,
    pub version: String,
    scroll: ScrollHandle,
    _watch: Option<Task<()>>,
}

impl AppView {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            profiles: Vec::new(),
            loading: true,
            problems: HashMap::new(),
            watched: Vec::new(),
            alert_open: false,
            alert_busy: String::new(),
            announced: HashSet::new(),
            toast: None,
            toast_timer: None,
            theme_open: false,
            page: Page::List,
            banner: Banner::Hidden,
            delete_target: None,
            version: env!("CARGO_PKG_VERSION").into(),
            scroll: ScrollHandle::new(),
            _watch: None,
        }
    }

    /// Reads the profiles, starts the folder watch and asks for an update.
    pub fn start(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.load_profiles(cx);
        self._watch = Some(cx.spawn(async move |this, cx| loop {
            if this.update(cx, |this, cx| this.check_folders(cx)).is_err() {
                break;
            }
            cx.background_executor().timer(WATCH).await;
        }));
        cx.spawn(async move |this, cx| match tokio(updater::check()).await {
            Ok(Some(update)) => {
                let _ = this.update(cx, |this, cx| {
                    this.banner = Banner::Available(update);
                    cx.notify();
                });
            }
            Ok(None) => {}
            // Offline machines cannot reach the endpoint; not worth a word.
            Err(error) => eprintln!("Update check failed: {error}"),
        })
        .detach();
    }

    pub fn load_profiles(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let profiles = blocking(commands::get_profiles).await;
            let _ = this.update(cx, |this, cx| {
                match profiles {
                    Ok(profiles) => this.profiles = profiles,
                    Err(error) => eprintln!("Failed to load profiles: {error}"),
                }
                this.loading = false;
                cx.notify();
            });
            // The card shows how many of its folders stopped holding. The
            // report walks each folder, so the list renders before it lands.
            match tokio(commands::doctor()).await {
                Ok(report) => {
                    let mut counts: HashMap<String, usize> = HashMap::new();
                    for folder in report.folders.iter().filter(|folder| !folder.ok) {
                        *counts.entry(folder.profile_id.clone()).or_default() += 1;
                    }
                    let _ = this.update(cx, |this, cx| {
                        this.problems = counts;
                        cx.notify();
                    });
                }
                Err(error) => eprintln!("Failed to read folder health: {error}"),
            }
        })
        .detach();
    }

    /// A folder that stops matching its rule is not something the user goes
    /// looking for, so the window comes forward for it — once per problem.
    pub fn check_folders(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let rows = match tokio(commands::watch_folders()).await {
                Ok(rows) => rows,
                Err(error) => {
                    eprintln!("Failed to check folders: {error}");
                    return;
                }
            };
            let mut located = Vec::new();
            for folder in rows.into_iter().filter(|row| row.state != "ok") {
                let moved_to = if folder.state == "missing" {
                    tokio(commands::locate_folder(folder.path.clone()))
                        .await
                        .ok()
                        .flatten()
                } else {
                    None
                };
                located.push((folder, moved_to));
            }
            let _ = this.update(cx, |this, cx| {
                let keys: HashSet<String> = located
                    .iter()
                    .map(|(row, _)| format!("{}|{}", row.path, row.state))
                    .collect();
                let fresh = keys.iter().any(|key| !this.announced.contains(key));
                this.announced = keys;
                this.watched = located;
                if fresh {
                    this.alert_open = true;
                    cx.defer(super::focus_window);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn save_progress(&mut self, progress: SaveProgress, cx: &mut Context<Self>) {
        if let Page::Form(form) = &self.page {
            form.update(cx, |form, cx| form.set_progress(progress, cx));
        }
    }

    /// Restarts the timer rather than adding one, so an early second toast is
    /// not cleared by the first one's timeout.
    pub fn show_toast(&mut self, message: String, cx: &mut Context<Self>) {
        self.toast = Some(message);
        self.toast_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST).await;
            let _ = this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn toast_error(&mut self, error: String, cx: &mut Context<Self>) {
        let message = f(cx, "app.toastError", &[("error", &error)]);
        self.show_toast(message, cx);
    }

    fn open_form(
        &mut self,
        profile: Option<Profile>,
        prefill: Option<GitIdentity>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = cx.new(|cx| {
            let mut form = ProfileForm::new(profile, prefill, window, cx);
            form.start(cx);
            form
        });
        cx.subscribe_in(&form, window, |this, _, event: &FormEvent, window, cx| {
            this.on_form_event(event.clone(), window, cx)
        })
        .detach();
        self.page = Page::Form(form);
        cx.notify();
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.new(|cx| {
            let mut page = SettingsPage::new(window, cx);
            page.start(window, cx);
            page
        });
        cx.subscribe(&page, |this, _, event: &SettingsEvent, cx| match event {
            SettingsEvent::Back => {
                this.page = Page::List;
                cx.notify();
            }
        })
        .detach();
        self.page = Page::Settings(page);
        cx.notify();
    }

    fn on_form_event(&mut self, event: FormEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            FormEvent::Saved => {
                self.load_profiles(cx);
                self.page = Page::List;
                let message = t(cx, "app.toastProfileSaved").to_string();
                self.show_toast(message, cx);
            }
            FormEvent::Cancel => {
                self.page = Page::List;
                cx.notify();
            }
            FormEvent::Settings => self.open_settings(window, cx),
            FormEvent::Delete(id, keys) => self.delete(id, keys, cx),
        }
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_form(None, None, window, cx);
    }

    pub fn edit(&mut self, profile: Profile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_form(Some(profile), None, window, cx);
        // Someone about to change one folder is told first that another moved.
        self.check_folders(cx);
    }

    fn import_from_git(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let identity = blocking(commands::get_git_identity)
                .await
                .unwrap_or(GitIdentity {
                    name: String::new(),
                    email: String::new(),
                });
            let _ = this.update_in(cx, |this, window, cx| {
                this.open_form(None, Some(identity), window, cx)
            });
        })
        .detach();
    }

    fn activate(&mut self, id: String, cx: &mut Context<Self>) {
        let name = self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.name.clone())
            .filter(|name| !name.is_empty());
        cx.spawn(async move |this, cx| {
            let result = blocking(move || commands::activate_profile(&id)).await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.load_profiles(cx);
                    let name = name.unwrap_or_else(|| t(cx, "app.toastProfileFallback"));
                    let message = f(cx, "app.toastActivated", &[("name", &name)]);
                    this.show_toast(message, cx);
                }
                Err(error) => this.toast_error(error, cx),
            });
        })
        .detach();
    }

    fn delete(&mut self, id: String, delete_keys: bool, cx: &mut Context<Self>) {
        let Some(profile) = self.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let name = profile.name.clone();
            let result = async {
                if delete_keys {
                    let mut keys = Vec::new();
                    for platform in PLATFORMS {
                        let Some(account) = profile.account(platform) else {
                            continue;
                        };
                        keys.push(account.ssh_private_key_path.clone());
                        if !account.ssh_public_key_path.is_empty() {
                            let _ = tokio(commands::remove_ssh_key_from_platform(
                                platform,
                                profile.id.clone(),
                                account.ssh_public_key_path.clone(),
                            ))
                            .await;
                        }
                    }
                    if !keys.is_empty() {
                        blocking(move || commands::delete_ssh_keys(keys)).await?;
                    }
                }
                let id = profile.id.clone();
                blocking(move || commands::delete_profile(id)).await
            }
            .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.load_profiles(cx);
                    this.page = Page::List;
                    let message = f(cx, "app.toastDeleted", &[("name", &name)]);
                    this.show_toast(message, cx);
                }
                Err(error) => this.toast_error(error, cx),
            });
        })
        .detach();
    }

    fn set_default(&mut self, id: String, platform: Platform, cx: &mut Context<Self>) {
        let Some(mut profile) = self.profiles.iter().find(|p| p.id == id).cloned() else {
            return;
        };
        profile.default_platform = Some(platform);
        cx.spawn(async move |this, cx| {
            let result = blocking(move || commands::save_profile(profile)).await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.load_profiles(cx);
                    let message = f(
                        cx,
                        "app.toastDefaultIdentity",
                        &[("platform", platform.label())],
                    );
                    this.show_toast(message, cx);
                }
                Err(error) => this.toast_error(error, cx),
            });
        })
        .detach();
    }

    fn run_alert_action(
        &mut self,
        key: String,
        task: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.alert_busy = key;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio(task).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.load_profiles(cx);
                        this.check_folders(cx);
                    }
                    Err(error) => this.toast_error(error, cx),
                }
                this.alert_busy.clear();
                cx.notify();
            });
        })
        .detach();
    }

    fn install_update(&mut self, update: Update, cx: &mut Context<Self>) {
        self.banner = Banner::Downloading(0);
        cx.notify();
        let (progress, mut downloads) = unbounded();
        cx.spawn(async move |this, cx| {
            let download = tokio(async move {
                updater::download(&update, |done, total| {
                    let _ = progress.unbounded_send((done, total));
                })
                .await
            });
            let watcher = async {
                while let Some((done, total)) = downloads.next().await {
                    let percent = match total {
                        Some(total) if total > 0 => {
                            (done as f64 / total as f64 * 100.).round().min(100.) as u32
                        }
                        _ => 0,
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.banner = Banner::Downloading(percent);
                        cx.notify();
                    });
                }
            };
            let (result, ()) = futures::join!(download, watcher);
            let result = match result {
                Ok(bytes) => blocking(move || updater::install(&bytes)).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => cx.update(|cx| cx.quit()),
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.banner = Banner::Failed(error);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

// -- drawing ------------------------------------------------------------------

impl AppView {
    fn update_banner(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let later = |id: &'static str, p: &Palette, cx: &mut Context<Self>| {
            let (hover_bg, hover_fg) = (p.subtle, p.fg2);
            control(id, p)
                .flex_none()
                .rounded(px(6.))
                .bg(p.raised)
                .px(px(12.))
                .py(px(6.))
                .ts(XS)
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.fg3)
                .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.banner = Banner::Hidden;
                    cx.notify();
                }))
                .child(t(cx, "update.later").to_string())
        };
        let row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .border_b_1()
            .border_color(p.border)
            .bg(alpha(hex(BLUE_600), 0.1))
            .px(px(24.))
            .py(px(10.))
            .ts(SM);
        match self.banner.clone() {
            Banner::Hidden => None,
            Banner::Available(update) => {
                let text = f(
                    cx,
                    "update.newVersion",
                    &[("version", &update.version), ("current", &self.version)],
                );
                Some(
                    row.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .text_color(p.fg2)
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.fg)
                                    .child(t(cx, "update.available").to_string()),
                            )
                            .child(format!("\u{a0}\u{2014} {text}")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                btn_small_blue("update-now", p, false)
                                    .py(px(6.))
                                    .ts(XS)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.install_update(update.clone(), cx)
                                    }))
                                    .child(t(cx, "update.updateNow").to_string()),
                            )
                            .child(later("update-later", p, cx)),
                    ),
                )
            }
            Banner::Downloading(percent) => Some(row.child(div().text_color(p.fg2).child(f(
                cx,
                "update.downloading",
                &[("percent", &percent.to_string())],
            )))),
            Banner::Failed(error) => Some(
                row.child(div().text_color(hex(super::theme::RED_400)).child(f(
                    cx,
                    "update.failed",
                    &[("error", &error)],
                )))
                .child(later("update-dismiss", p, cx)),
            ),
        }
    }

    fn header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let preference = cx.global::<Prefs>().theme;
        let current = THEMES
            .iter()
            .find(|(theme, _, _)| *theme == preference)
            .map(|(_, _, glyph)| *glyph)
            .unwrap_or(MONITOR);
        let menu = self.theme_open.then(|| {
            deferred(
                div()
                    .id("theme-menu")
                    .occlude()
                    .absolute()
                    .top(px(40.))
                    .right_0()
                    .w(px(144.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.dialog)
                    .py(px(4.))
                    .shadow(shadow_lg())
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.theme_open = false;
                        cx.notify();
                    }))
                    .children(THEMES.iter().map(|&(theme, key, glyph)| {
                        let selected = theme == preference;
                        let hover = p.raised;
                        let color = if selected { p.selected_fg } else { p.fg3 };
                        control(key, p)
                            .flex()
                            .w_full()
                            .items_center()
                            .gap(px(8.))
                            .px(px(12.))
                            .py(px(8.))
                            .ts(SM)
                            .text_color(color)
                            .when(selected, |row| row.bg(p.selected_bg))
                            .when(!selected, |row| row.hover(move |s| s.bg(hover)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                super::set_theme(theme, cx);
                                this.theme_open = false;
                                cx.notify();
                            }))
                            .child(icon(glyph, 16., color))
                            .child(t(cx, key).to_string())
                    })),
            )
            .with_priority(1)
        });
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
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .ts(XL)
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.fg)
                            .child("Git Account Manager"),
                    )
                    .child(
                        div()
                            .rounded_full()
                            .bg(p.raised)
                            .px(px(8.))
                            .py(px(2.))
                            .ts(XS)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.fg4)
                            .child(format!("v{}", self.version)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        btn_icon("github", GITHUB, t(cx, "app.githubRepo").to_string(), p)
                            .tooltip(tip(&t(cx, "app.githubRepo"), p))
                            .on_click(|_, _, cx| cx.open_url(super::REPOSITORY)),
                    )
                    .child(
                        div()
                            .relative()
                            .child(
                                btn_icon("theme", current, t(cx, "app.themeTitle").to_string(), p)
                                    .aria_expanded(self.theme_open)
                                    .tooltip(tip(&t(cx, "app.themeTitle"), p))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.theme_open = !this.theme_open;
                                        cx.notify();
                                    })),
                            )
                            .children(menu),
                    )
                    .child(
                        btn_icon("settings", GEAR, t(cx, "app.oauthSettings").to_string(), p)
                            .tooltip(tip(&t(cx, "app.oauthSettings"), p))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                            ),
                    )
                    .child(
                        btn_primary("new-profile", p, false)
                            .gap(px(6.))
                            .on_click(cx.listener(|this, _, window, cx| this.add(window, cx)))
                            .child(icon(PLUS, 16., white()))
                            .child(t(cx, "app.newProfile").to_string()),
                    ),
            )
    }

    fn badge(
        &self,
        profile: &Profile,
        platform: Platform,
        is_default: bool,
        can_click: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let account = profile.account(platform).expect("connected platform");
        let username = account.username.clone();
        let (link, link_hover) = (p.link, p.link_hover);
        let body_top = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .ts(SM)
            .text_color(p.fg3)
            .child(icon(platform_icon(platform), 16., p.fg4))
            .child(
                div()
                    .flex()
                    .child(format!("{}:\u{a0}", platform.label()))
                    .child(
                        div()
                            .id("username")
                            .cursor_pointer()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(link)
                            .hover(move |s| s.text_color(link_hover).underline())
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.open_url(&super::helpers::profile_url(platform, &username));
                            })
                            .child(format!("@{}", account.username)),
                    ),
            )
            .when(is_default && can_click, |row| {
                row.child(
                    div()
                        .ml_auto()
                        .ts(X10)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.link)
                        .child(t(cx, "card.default").to_string()),
                )
            });
        let body_bottom = div()
            .pl(px(24.))
            .ts(XS)
            .text_color(p.fg4)
            .child(format!("{} <{}>", account.git_name, account.git_email));
        let id = SharedString::from(format!("badge-{}-{}", profile.id, platform.as_str()));
        if !can_click {
            return div()
                .id(id)
                .rounded(px(6.))
                .border_1()
                .border_color(transparent_black())
                .px(px(12.))
                .py(px(8.))
                .child(body_top)
                .child(body_bottom);
        }
        let hover = p.border_s;
        let profile_id = profile.id.clone();
        let title = f(
            cx,
            "card.setDefaultTitle",
            &[("platform", platform.label())],
        );
        control(id, p)
            .rounded(px(6.))
            .border_1()
            .px(px(12.))
            .py(px(8.))
            .when(is_default, |b| {
                b.border_color(p.selected_border).bg(p.selected_bg)
            })
            .when(!is_default, |b| {
                b.border_color(p.border)
                    .bg(p.raised_40)
                    .hover(move |s| s.border_color(hover))
            })
            .tooltip(tip(&title, p))
            .on_click(
                cx.listener(move |this, _, _, cx| {
                    this.set_default(profile_id.clone(), platform, cx)
                }),
            )
            .child(body_top)
            .child(body_bottom)
    }

    fn card(&self, profile: &Profile, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let connected: Vec<Platform> = PLATFORMS
            .into_iter()
            .filter(|&platform| profile.account(platform).is_some())
            .collect();
        let default = profile
            .default_platform
            .or(connected.first().copied())
            .unwrap_or(Platform::Bitbucket);
        let can_choose = connected.len() >= 2;
        let problems = self.problems.get(&profile.id).copied().unwrap_or(0);
        let hover = p.border_s;
        let small = |id: &str, p: &Palette| {
            control(SharedString::from(format!("{id}-{}", profile.id)), p)
                .rounded(px(6.))
                .px(px(12.))
                .py(px(6.))
                .ts(XS)
                .font_weight(FontWeight::MEDIUM)
        };
        let (edit_profile, delete_id, activate_id) =
            (profile.clone(), profile.id.clone(), profile.id.clone());
        let pill_profile = profile.clone();
        let (subtle_hover, danger_hover) = (p.hover, p.danger_hover);
        div()
            .id(SharedString::from(format!("card-{}", profile.id)))
            .rounded(px(8.))
            .border_1()
            .p(px(16.))
            .when(profile.is_active, |c| {
                c.border_color(p.active_border).bg(p.active_bg)
            })
            .when(!profile.is_active, |c| {
                c.border_color(p.border)
                    .bg(p.raised_60)
                    .hover(move |s| s.border_color(hover))
            })
            .child(
                div()
                    .mb(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .ts(LG)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.fg)
                                    .child(profile.name.clone()),
                            )
                            .when(profile.is_active, |row| {
                                row.child(
                                    div()
                                        .rounded_full()
                                        .bg(alpha(hex(EMERALD_600), 0.8))
                                        .px(px(8.))
                                        .py(px(2.))
                                        .ts(XS)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(hex(EMERALD_100))
                                        .child(t(cx, "card.active").to_string()),
                                )
                            }),
                    )
                    .when(problems > 0, |row| {
                        let hover = alpha(hex(AMBER_500), 0.25);
                        row.child(
                            control(SharedString::from(format!("issues-{}", profile.id)), p)
                                .rounded_full()
                                .bg(alpha(hex(AMBER_500), 0.15))
                                .px(px(8.))
                                .py(px(2.))
                                .ts(XS)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.issues)
                                .hover(move |s| s.bg(hover))
                                .tooltip(tip(&t(cx, "card.problemsTitle"), p))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit(pill_profile.clone(), window, cx)
                                }))
                                .child(f(cx, "card.problems", &[("count", &problems.to_string())])),
                        )
                    }),
            )
            .when(!connected.is_empty(), |c| {
                c.child(div().mb(px(12.)).flex().flex_col().gap(px(6.)).children(
                    connected.iter().map(|&platform| {
                        self.badge(profile, platform, default == platform, can_choose, p, cx)
                    }),
                ))
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .when(!profile.is_active, |row| {
                        row.child(
                            small("activate", p)
                                .bg(hex(EMERALD_600))
                                .text_color(white())
                                .hover(|s| s.bg(hex(EMERALD_500)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.activate(activate_id.clone(), cx)
                                }))
                                .child(t(cx, "card.activate").to_string()),
                        )
                    })
                    .child(
                        small("edit", p)
                            .bg(p.subtle)
                            .text_color(p.fg2)
                            .hover(move |s| s.bg(subtle_hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(edit_profile.clone(), window, cx)
                            }))
                            .child(t(cx, "card.edit").to_string()),
                    )
                    .child(
                        small("delete", p)
                            .bg(p.subtle)
                            .text_color(p.danger_fg)
                            .hover(move |s| s.bg(danger_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete_target = Some(delete_id.clone());
                                cx.notify();
                            }))
                            .child(t(cx, "card.delete").to_string()),
                    ),
            )
    }

    fn list(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.loading {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .py(px(80.))
                .text_color(p.fg4)
                .child(t(cx, "app.loading").to_string())
                .into_any_element();
        }
        if self.profiles.is_empty() {
            let (link, link_hover) = (p.link, p.link_hover);
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .py(px(80.))
                .child(icon(PEOPLE, 64., p.fg6).mb(px(16.)))
                .child(
                    div()
                        .mb(px(8.))
                        .ts(LG)
                        .text_color(p.fg4)
                        .child(t(cx, "app.noProfiles").to_string()),
                )
                .child(
                    div()
                        .mb(px(16.))
                        .ts(SM)
                        .text_color(p.fg5)
                        .child(t(cx, "app.noProfilesHint").to_string()),
                )
                .child(
                    btn_primary("create", p, false)
                        .on_click(cx.listener(|this, _, window, cx| this.add(window, cx)))
                        .child(t(cx, "app.createProfile").to_string()),
                )
                .child(
                    control("import", p)
                        .mt(px(12.))
                        .ts(SM)
                        .text_color(link)
                        .hover(move |s| s.text_color(link_hover).underline())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.import_from_git(window, cx)),
                        )
                        .child(t(cx, "form.importFromGit").to_string()),
                )
                .into_any_element();
        }
        div()
            .mx_auto()
            .w_full()
            .max_w(px(672.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(
                self.profiles
                    .iter()
                    .map(|profile| self.card(profile, p, cx)),
            )
            .into_any_element()
    }

    fn delete_dialog(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let profile = self
            .profiles
            .iter()
            .find(|profile| Some(&profile.id) == self.delete_target.as_ref())?;
        let keys: Vec<String> = PLATFORMS
            .into_iter()
            .filter_map(|platform| profile.account(platform))
            .map(|account| account.ssh_private_key_path.clone())
            .filter(|path| !path.is_empty())
            .collect();
        let id = profile.id.clone();
        let action = |key: &'static str,
                      label: String,
                      variant: Variant,
                      keys: Option<bool>,
                      cx: &mut Context<Self>| {
            let id = id.clone();
            dialog_action(key, label, variant, p).on_click(cx.listener(move |this, _, _, cx| {
                this.delete_target = None;
                if let Some(delete_keys) = keys {
                    this.delete(id.clone(), delete_keys, cx);
                }
                cx.notify();
            }))
        };
        let mut actions = Vec::new();
        if keys.is_empty() {
            actions.push(action(
                "delete",
                t(cx, "card.deleteProfile"),
                Variant::Danger,
                Some(false),
                cx,
            ));
        } else {
            actions.push(action(
                "delete-keys",
                t(cx, "card.deleteAndKeys"),
                Variant::Danger,
                Some(true),
                cx,
            ));
            actions.push(action(
                "delete-keep",
                t(cx, "card.deleteKeepKeys"),
                Variant::Default,
                Some(false),
                cx,
            ));
        }
        actions.push(action(
            "cancel",
            t(cx, "card.cancel"),
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
                    .child(t(cx, "card.deleteBody").to_string()),
            )
            .when(!keys.is_empty(), |body| {
                body.child(key_list(t(cx, "card.deleteKeysLabel"), &keys, p))
            });
        let view = cx.entity().downgrade();
        Some(confirm_dialog(
            f(cx, "card.deleteTitle", &[("name", &profile.name)]),
            body,
            actions,
            move |_, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.delete_target = None;
                    cx.notify();
                });
            },
            p,
        ))
    }

    fn folder_alert(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if !self.alert_open || self.watched.is_empty() {
            return None;
        }
        let busy = !self.alert_busy.is_empty();
        let rows = self.watched.iter().map(|(folder, moved_to)| {
            let label = match folder.state.as_str() {
                "missing" => t(cx, "repos.watchMissing"),
                "no-rule" => t(cx, "repos.watchNoRule"),
                "guard-off" => t(cx, "repos.watchGuardOff"),
                _ => String::new(),
            };
            let path = folder.path.clone();
            let mut actions = div().mt(px(8.)).flex().flex_wrap().gap(px(8.));
            if let Some(moved) = moved_to.clone() {
                let (from, key) = (path.clone(), format!("relink:{path}"));
                let spinning = self.alert_busy == key;
                actions = actions.child(
                    btn_small_blue(SharedString::from(key.clone()), p, busy)
                        .when(spinning, |b| b.child(spinner(11., white())))
                        .when(!busy, |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                let (from, to) = (from.clone(), moved.clone());
                                this.run_alert_action(
                                    key.clone(),
                                    commands::relink_folder(from, to),
                                    cx,
                                )
                            }))
                        })
                        .child(t(cx, "repos.relink").to_string()),
                );
            }
            let profile_id = folder.profile_id.clone();
            actions = actions.child(
                btn_raised_sm(SharedString::from(format!("open:{path}")), p, busy)
                    .when(!busy, |b| {
                        b.on_click(cx.listener(move |this, _, window, cx| {
                            this.alert_open = false;
                            if let Some(profile) =
                                this.profiles.iter().find(|p| p.id == profile_id).cloned()
                            {
                                this.edit(profile, window, cx);
                            }
                            cx.notify();
                        }))
                    })
                    .child(t(cx, "repos.openProfile").to_string()),
            );
            if folder.state == "missing" {
                let key = format!("forget:{path}");
                let spinning = self.alert_busy == key;
                let target = path.clone();
                actions = actions.child(
                    btn_quiet_danger(SharedString::from(key.clone()), p, busy)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .px(px(12.))
                        .py(px(4.))
                        .ts(X11)
                        .when(spinning, |b| b.child(spinner(11., p.fg4)))
                        .when(!busy, |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                this.run_alert_action(
                                    key.clone(),
                                    commands::forget_folder(target.clone()),
                                    cx,
                                )
                            }))
                        })
                        .child(t(cx, "repos.forgetFolder").to_string()),
                );
            }
            div()
                .rounded(px(6.))
                .bg(p.raised)
                .p(px(12.))
                .child(
                    div()
                        .font_family(MONO_FONT)
                        .ts(XS)
                        .text_color(p.fg2)
                        .truncate()
                        .child(folder.path.clone()),
                )
                .child(
                    div()
                        .mt(px(4.))
                        .ts(X11)
                        .text_color(p.danger_fg)
                        .child(label),
                )
                .child(
                    div()
                        .ts(X11)
                        .text_color(p.fg5)
                        .child(folder.profile_name.clone()),
                )
                .when_some(moved_to.as_ref(), |row, moved| {
                    row.child(div().mt(px(4.)).ts(X11).text_color(p.fg4).child(f(
                        cx,
                        "repos.movedFound",
                        &[("path", moved)],
                    )))
                })
                .child(actions)
        });
        Some(
            backdrop(hsla(0., 0., 0., 0.5)).p(px(24.)).child(
                div()
                    .id("folder-alert")
                    .role(Role::AlertDialog)
                    .occlude()
                    .max_h_full()
                    .w_full()
                    .max_w(px(512.))
                    .overflow_y_scroll()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.dialog)
                    .p(px(20.))
                    .shadow(shadow_xl())
                    .child(
                        div()
                            .ts(BASE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.fg)
                            .child(t(cx, "repos.alertTitle").to_string()),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .ts(XS)
                            .text_color(p.fg4)
                            .child(t(cx, "repos.alertHint").to_string()),
                    )
                    .child(
                        div()
                            .mt(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .children(rows),
                    )
                    .child(
                        div().mt(px(16.)).flex().justify_end().child(
                            btn_raised_sm("alert-dismiss", p, false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.alert_open = false;
                                    cx.notify();
                                }))
                                .child(t(cx, "repos.alertDismiss").to_string()),
                        ),
                    ),
            ),
        )
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        if self.delete_target.take().is_some() {
            cx.notify();
            return;
        }
        match &self.page {
            Page::Form(form) => form.update(cx, |form, cx| form.escape(window, cx)),
            Page::Settings(_) => {
                self.page = Page::List;
                cx.notify();
            }
            Page::List => {}
        }
    }
}

/// The key files a dialog is about to delete.
pub fn key_list(label: impl Into<SharedString>, keys: &[String], p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().ts(XS).text_color(p.fg4).child(label.into()))
        .children(keys.iter().map(|key| {
            div()
                .rounded(px(4.))
                .bg(p.raised)
                .px(px(8.))
                .py(px(4.))
                .font_family(MONO_FONT)
                .ts(XS)
                .text_color(p.fg3)
                .child(file_name(key).to_string())
        }))
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window, cx);
        let page: AnyElement = match &self.page {
            Page::Form(form) => form.clone().into_any_element(),
            Page::Settings(settings) => settings.clone().into_any_element(),
            Page::List => div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .children(self.update_banner(&p, cx))
                .child(self.header(&p, cx))
                .child(scroll_area(
                    "main",
                    &self.scroll,
                    &p,
                    div().p(px(24.)).child(self.list(&p, cx)),
                ))
                .into_any_element(),
        };
        let toast = matches!(self.page, Page::List)
            .then(|| self.toast.clone())
            .flatten()
            .map(|message| {
                div()
                    .absolute()
                    .right(px(16.))
                    .bottom(px(16.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.raised)
                    .px(px(16.))
                    .py(px(8.))
                    .ts(SM)
                    .text_color(p.fg2)
                    .shadow(shadow_lg())
                    .child(message)
            });
        div()
            .id("root")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .font_family(UI_FONT)
            .ts(BASE)
            .on_key_down(cx.listener(Self::on_key))
            .child(page)
            .children(toast)
            .children(self.delete_dialog(&p, cx))
            .children(self.folder_alert(&p, cx))
    }
}
