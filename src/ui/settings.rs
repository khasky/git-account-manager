//! The settings page: appearance, the guard rails and the integrations.

use std::time::Duration;

use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::app::THEMES;
use super::i18n::{I18n, LANGUAGES};
use super::theme::{Palette, Prefs};
use super::widgets::*;
use super::{blocking, f, palette, t};
use crate::commands;
use crate::gh::GhProbe;
use crate::guard::GuardStatus;
use crate::models::{GuardSettings, OAuthSettings};
use crate::openssh_integration::OpenSshIntegrationProbe;

const SAVED_HINT: Duration = Duration::from_secs(2);

// -- autostart ------------------------------------------------------------------

/// The same registration the previous release's autostart plugin made: the
/// product name and the executable, so an existing entry is recognised.
fn launcher() -> Result<auto_launch::AutoLaunch, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[allow(unused_mut)]
    let mut path = exe.display().to_string();
    // The login item is the bundle, not the executable inside it.
    #[cfg(target_os = "macos")]
    if let Some((bundle, _)) = path.split_once(".app/") {
        path = format!("{bundle}.app");
    }
    #[cfg(target_os = "linux")]
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        path = appimage;
    }
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder
        .set_app_name("Git Account Manager")
        .set_app_path(&path);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);
    builder.build().map_err(|e| e.to_string())
}

fn autostart_enabled() -> bool {
    launcher()
        .and_then(|l| l.is_enabled().map_err(|e| e.to_string()))
        .unwrap_or(false)
}

fn set_autostart(on: bool) -> Result<(), String> {
    let launcher = launcher()?;
    if on {
        launcher.enable()
    } else {
        launcher.disable()
    }
    .map_err(|e| e.to_string())
}

// -- page -----------------------------------------------------------------------

pub enum SettingsEvent {
    Back,
}

impl EventEmitter<SettingsEvent> for SettingsPage {}

pub struct SettingsPage {
    github_id: Entity<InputState>,
    gitlab_id: Entity<InputState>,
    language: Entity<Select>,
    pub use_openssh: bool,
    pub switch_gh: bool,
    pub use_https: bool,
    /// Whether the active profile has a token the helper could answer with.
    pub has_https_token: bool,
    pub gh: Option<GhProbe>,
    pub openssh: Option<OpenSshIntegrationProbe>,
    pub autostart: bool,
    pub guard: Option<GuardSettings>,
    pub guard_status: Option<GuardStatus>,
    saving: bool,
    saved: bool,
    saved_timer: Option<Task<()>>,
    save_error: String,
    scroll: ScrollHandle,
}

impl SettingsPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let github_id = input_state(
            t(cx, "settings.github.placeholder").to_string(),
            false,
            window,
            cx,
        );
        let gitlab_id = input_state(
            t(cx, "settings.gitlab.placeholder").to_string(),
            false,
            window,
            cx,
        );
        for input in [&github_id, &gitlab_id] {
            cx.subscribe(input, |_, _, _: &InputEvent, cx| cx.notify())
                .detach();
        }
        let current = cx.global::<I18n>().lang;
        let label = t(cx, "settings.language");
        let language = cx.new(|_| {
            Select::new(
                LANGUAGES
                    .iter()
                    .map(|(_, name, _)| SharedString::from(*name))
                    .collect(),
                LANGUAGES
                    .iter()
                    .position(|(code, _, _)| *code == current)
                    .unwrap_or(0),
                false,
                label.into(),
            )
        });
        cx.subscribe(&language, |_, _, event: &SelectChanged, cx| {
            super::set_language(LANGUAGES[event.0].0, cx);
        })
        .detach();
        Self {
            github_id,
            gitlab_id,
            language,
            use_openssh: false,
            switch_gh: true,
            use_https: false,
            has_https_token: true,
            gh: None,
            openssh: None,
            autostart: false,
            guard: None,
            guard_status: None,
            saving: false,
            saved: false,
            saved_timer: None,
            save_error: String::new(),
            scroll: ScrollHandle::new(),
        }
    }

    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let settings = blocking(commands::get_settings).await;
            let has_token = blocking(|| {
                let profiles = commands::get_profiles()?;
                let Some(active) = profiles.into_iter().find(|p| p.is_active) else {
                    return Ok(true);
                };
                commands::https_token_platforms(active.id).map(|platforms| !platforms.is_empty())
            })
            .await
            .unwrap_or(true);
            let gh = blocking(commands::gh_probe).await;
            let openssh = blocking(commands::openssh_integration_probe).await;
            let autostart = blocking(autostart_enabled).await;
            let guard = super::tokio(commands::get_repo_state())
                .await
                .ok()
                .map(|s| s.guard);
            let _ = this.update_in(cx, |this, window, cx| {
                if let Ok(settings) = settings {
                    this.set_settings(settings, window, cx);
                }
                this.has_https_token = has_token;
                this.gh = Some(gh);
                this.openssh = Some(openssh);
                this.autostart = autostart;
                this.guard = guard;
                cx.notify();
            });
            let status = super::tokio(commands::doctor()).await.ok().map(|r| r.guard);
            let _ = this.update(cx, |this, cx| {
                this.guard_status = status;
                cx.notify();
            });
        })
        .detach();
    }

    /// Fills the page from stored settings.
    pub fn set_settings(
        &mut self,
        settings: OAuthSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.use_openssh = settings.use_openssh_for_git_tools;
        self.switch_gh = settings.switch_gh_account;
        self.use_https = settings.use_https_credential_helper;
        self.github_id.update(cx, |s, cx| {
            s.set_value(settings.github_client_id, window, cx)
        });
        self.gitlab_id.update(cx, |s, cx| {
            s.set_value(settings.gitlab_client_id, window, cx)
        });
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        cx.emit(SettingsEvent::Back);
    }

    fn toggle_autostart(&mut self, cx: &mut Context<Self>) {
        let on = !self.autostart;
        cx.spawn(async move |this, cx| {
            if blocking(move || set_autostart(on)).await.is_ok() {
                let _ = this.update(cx, |this, cx| {
                    this.autostart = on;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// The guard rails touch the machine's Git config, so each applies the
    /// moment it is toggled rather than waiting for Save.
    fn toggle_guard(&mut self, unset_global: bool, cx: &mut Context<Self>) {
        let Some(previous) = self.guard.clone() else {
            return;
        };
        let mut next = previous.clone();
        if unset_global {
            next.unset_global_identity = !next.unset_global_identity;
        } else {
            next.guard_commits = !next.guard_commits;
        }
        self.guard = Some(next.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = blocking(move || commands::save_guard_settings(next)).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.save_error.clear(),
                    Err(error) => {
                        this.guard = Some(previous);
                        this.save_error = error;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let settings = OAuthSettings {
            github_client_id: value(&self.github_id, cx).trim().to_string(),
            gitlab_client_id: value(&self.gitlab_id, cx).trim().to_string(),
            use_openssh_for_git_tools: self.use_openssh,
            switch_gh_account: self.switch_gh,
            use_https_credential_helper: self.use_https,
        };
        self.saving = true;
        self.save_error.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = blocking(move || commands::save_settings(settings)).await;
            let probe = blocking(commands::openssh_integration_probe).await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.openssh = Some(probe);
                        this.saved = true;
                        this.saved_timer = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(SAVED_HINT).await;
                            let _ = this.update(cx, |this, cx| {
                                this.saved = false;
                                cx.notify();
                            });
                        }));
                    }
                    Err(error) => {
                        this.save_error = if error.is_empty() {
                            t(cx, "settings.saveError").to_string()
                        } else {
                            error
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// `panel space-y-3`.
pub fn panel(p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.raised_40)
        .p(px(16.))
}

pub fn panel_title(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg2)
        .child(text.into())
}

/// `text-xs leading-relaxed`.
pub fn relaxed(p: &Palette, color: Hsla) -> Div {
    let _ = p;
    div()
        .text_size(px(12.))
        .line_height(px(19.5))
        .text_color(color)
}

impl SettingsPage {
    /// A labelled switch: the label and its hint on the left, the control on
    /// the right.
    fn row(
        label: impl Into<SharedString>,
        hint: impl IntoElement,
        control: impl IntoElement,
        p: &Palette,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .min_w_0()
                    .child(div().ts(SM).text_color(p.fg3).child(label.into()))
                    .child(div().ts(XS).text_color(p.fg5).child(hint)),
            )
            .child(control)
    }

    fn general(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let preference = cx.global::<Prefs>().theme;
        let last = THEMES.len() - 1;
        let segments = div()
            .flex_none()
            .flex()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .children(
                THEMES
                    .iter()
                    .enumerate()
                    .map(|(index, &(theme, key, glyph))| {
                        let selected = theme == preference;
                        let (hover_bg, hover_fg) = (p.raised, p.fg2);
                        let color = if selected { p.selected_fg } else { p.fg4 };
                        control(SharedString::from(format!("segment-{key}")), p)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(12.))
                            .py(px(6.))
                            .ts(XS)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(color)
                            .when(index == 0, |b| b.rounded_l(px(7.)))
                            .when(index == last, |b| b.rounded_r(px(7.)))
                            .when(selected, |b| b.bg(p.selected_bg))
                            .when(!selected, |b| {
                                b.hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                            })
                            .tooltip(tip(&t(cx, key), p))
                            .on_click(move |_, _, cx| super::set_theme(theme, cx))
                            .child(icon(glyph, 16., color))
                            .child(t(cx, key).to_string())
                    }),
            );
        panel(p)
            .child(panel_title(t(cx, "settings.general"), p))
            .child(Self::row(
                t(cx, "settings.theme"),
                t(cx, "settings.themeHint").to_string(),
                segments,
                p,
            ))
            .child(Self::row(
                t(cx, "settings.autostart"),
                t(cx, "settings.autostartHint").to_string(),
                toggle("autostart", self.autostart, p, ToggleSize::Md)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_autostart(cx))),
                p,
            ))
            .child(Self::row(
                t(cx, "settings.language"),
                t(cx, "settings.languageHint").to_string(),
                self.language.clone(),
                p,
            ))
    }

    fn guard_panel(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let guard = self.guard.as_ref()?;
        let rows = [
            (
                true,
                "settings.guard.unsetGlobal",
                "settings.guard.unsetGlobalHint",
                guard.unset_global_identity,
            ),
            (
                false,
                "settings.guard.guardCommits",
                "settings.guard.guardCommitsHint",
                guard.guard_commits,
            ),
        ];
        let mut panel = panel(p)
            .child(panel_title(t(cx, "settings.guard.title"), p))
            .child(relaxed(p, p.fg4).child(t(cx, "settings.guard.intro").to_string()));
        for (unset, label, hint, on) in rows {
            panel = panel.child(Self::row(
                t(cx, label),
                rich(hint, t(cx, hint), p.fg5, p.fg4, p, Link::None),
                toggle(label, on, p, ToggleSize::Md)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_guard(unset, cx))),
                p,
            ));
        }
        let Some(status) = self.guard_status.as_ref() else {
            return Some(panel);
        };
        let entry = |term: String, value: String, color: Hsla| {
            div()
                .flex()
                .justify_between()
                .gap(px(16.))
                .child(div().flex_none().text_color(p.fg4).child(term))
                .child(div().min_w_0().text_right().text_color(color).child(value))
        };
        let (on, off) = (
            t(cx, "repos.on").to_string(),
            t(cx, "repos.off").to_string(),
        );
        let hooks = match status.hooks.as_str() {
            "global" => t(cx, "repos.hookGlobal").to_string(),
            "foreign" => format!(
                "{}: {}",
                t(cx, "repos.hookForeign"),
                status.hooks_path.clone().unwrap_or_default()
            ),
            _ => t(cx, "repos.hookOff").to_string(),
        };
        let list = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .border_t_1()
            .border_color(p.border)
            .pt(px(12.))
            .ts(XS)
            .child(entry(
                t(cx, "repos.globalIdentity").to_string(),
                if status.global_email.is_empty() {
                    t(cx, "repos.globalNone").to_string()
                } else {
                    status.global_email.clone()
                },
                p.fg3,
            ))
            .child(entry(
                t(cx, "repos.useConfigOnly").to_string(),
                if status.use_config_only {
                    on.clone()
                } else {
                    off.clone()
                },
                if status.use_config_only || !guard.unset_global_identity {
                    p.fg3
                } else {
                    p.danger_fg
                },
            ))
            .child(entry(
                t(cx, "repos.includesRegion").to_string(),
                if status.rules_written { on } else { off },
                p.fg3,
            ))
            .child(entry(
                t(cx, "repos.hooksPath").to_string(),
                hooks,
                if status.hooks == "global" || !guard.guard_commits {
                    p.fg3
                } else {
                    p.danger_fg
                },
            ))
            .children(status.ssh_hosts.iter().map(|owner| {
                entry(
                    owner.host.clone(),
                    owner
                        .profile
                        .clone()
                        .unwrap_or_else(|| t(cx, "repos.hostNoKey").to_string()),
                    if owner.profile.is_some() {
                        p.fg3
                    } else {
                        p.danger_fg
                    },
                )
            }));
        Some(panel.child(list))
    }

    fn tortoise_panel(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let probe = self.openssh.as_ref().filter(|probe| probe.available)?;
        let found = match &probe.ssh_exe {
            Some(path) => div().ts(XS).child(rich(
                "tortoise-detected",
                f(cx, "settings.tortoise.detected", &[("path", path)]),
                p.fg5,
                p.fg3,
                p,
                Link::None,
            )),
            None => div().ts(XS).child(rich(
                "tortoise-missing",
                t(cx, "settings.tortoise.notFound"),
                p.warn,
                p.fg3,
                p,
                Link::Href("https://git-scm.com/download/win".into()),
            )),
        };
        Some(
            panel(p)
                .child(panel_title(t(cx, "settings.tortoise.title"), p))
                .child(relaxed(p, p.fg4).child(rich(
                    "tortoise-intro",
                    t(cx, "settings.tortoise.intro"),
                    p.fg4,
                    p.fg3,
                    p,
                    Link::None,
                )))
                .child(found)
                .child(Self::row(
                    t(cx, "settings.tortoise.toggle"),
                    rich(
                        "tortoise-hint",
                        t(cx, "settings.tortoise.toggleHint"),
                        p.fg5,
                        p.fg4,
                        p,
                        Link::None,
                    ),
                    toggle("openssh", self.use_openssh, p, ToggleSize::Md).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.use_openssh = !this.use_openssh;
                            cx.notify();
                        },
                    )),
                    p,
                ))
                .when(self.use_openssh && probe.ssh_exe.is_none(), |panel| {
                    panel.child(div().ts(XS).child(rich(
                        "tortoise-fail",
                        t(cx, "settings.tortoise.willFail"),
                        p.fail,
                        p.fg3,
                        p,
                        Link::None,
                    )))
                }),
        )
    }

    fn gh_panel(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let probe = self.gh.as_ref().map(|probe| {
            if !probe.available {
                return div().ts(XS).child(rich(
                    "gh-missing",
                    t(cx, "settings.gh.notFound"),
                    p.warn,
                    p.fg3,
                    p,
                    Link::Href("https://cli.github.com/".into()),
                ));
            }
            let text = if probe.logins.is_empty() {
                rich(
                    "gh-none",
                    t(cx, "settings.gh.noAccounts"),
                    p.fg5,
                    p.fg4,
                    p,
                    Link::None,
                )
            } else {
                f(
                    cx,
                    "settings.gh.accounts",
                    &[
                        ("logins", &probe.logins.join(", ")),
                        ("active", probe.active.as_deref().unwrap_or("")),
                    ],
                )
                .into_any_element()
            };
            div().ts(XS).text_color(p.fg5).child(text)
        });
        panel(p)
            .child(panel_title(t(cx, "settings.gh.title"), p))
            .child(relaxed(p, p.fg4).child(rich(
                "gh-intro",
                t(cx, "settings.gh.intro"),
                p.fg4,
                p.fg3,
                p,
                Link::None,
            )))
            .children(probe)
            .child(Self::row(
                t(cx, "settings.gh.toggle"),
                rich(
                    "gh-hint",
                    t(cx, "settings.gh.toggleHint"),
                    p.fg5,
                    p.fg4,
                    p,
                    Link::None,
                ),
                toggle("switch-gh", self.switch_gh, p, ToggleSize::Md).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.switch_gh = !this.switch_gh;
                        cx.notify();
                    },
                )),
                p,
            ))
    }

    fn https_panel(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        panel(p)
            .child(panel_title(t(cx, "settings.https.title"), p))
            .child(relaxed(p, p.fg4).child(rich(
                "https-intro",
                t(cx, "settings.https.intro"),
                p.fg4,
                p.fg3,
                p,
                Link::None,
            )))
            .child(Self::row(
                t(cx, "settings.https.toggle"),
                rich(
                    "https-hint",
                    t(cx, "settings.https.toggleHint"),
                    p.fg5,
                    p.fg4,
                    p,
                    Link::None,
                ),
                toggle("https-helper", self.use_https, p, ToggleSize::Md).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.use_https = !this.use_https;
                        cx.notify();
                    },
                )),
                p,
            ))
            .when(self.use_https && !self.has_https_token, |panel| {
                panel.child(div().ts(XS).child(rich(
                    "https-needs",
                    t(cx, "settings.https.needsToken"),
                    p.warn,
                    p.fg3,
                    p,
                    Link::None,
                )))
            })
    }

    fn oauth_panel(
        &self,
        key: &'static str,
        url: &'static str,
        input: &Entity<InputState>,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let steps = (1..=6).map(|n| {
            let text = format!("{n}. {}", t(cx, &format!("settings.{key}.step{n}")));
            let link = if n == 1 {
                Link::Href(url.into())
            } else {
                Link::None
            };
            div().child(rich(
                SharedString::from(format!("{key}-step{n}")),
                &text,
                p.fg4,
                p.fg3,
                p,
                match link {
                    Link::Href(url) => Link::Action(Box::new(move |_, cx| cx.open_url(&url))),
                    other => other,
                },
            ))
        });
        panel(p)
            .child(panel_title(t(cx, &format!("settings.{key}.title")), p))
            .child(
                div()
                    .ts(XS)
                    .text_color(p.fg4)
                    .child(t(cx, &format!("settings.{key}.required")).to_string()),
            )
            .child(div().flex().flex_col().gap(px(4.)).ts(XS).children(steps))
            .child(field(input, Field::Md, p, window, cx))
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window, cx);
        let (back_hover, github, gitlab) = (p.fg2, self.github_id.clone(), self.gitlab_id.clone());
        let body = div()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(self.general(&p, cx))
            .children(self.guard_panel(&p, cx))
            .children(self.tortoise_panel(&p, cx))
            .child(self.gh_panel(&p, cx))
            .child(self.https_panel(&p, cx))
            .child(self.oauth_panel(
                "github",
                "https://github.com/settings/developers",
                &github,
                &p,
                window,
                cx,
            ))
            .child(self.oauth_panel(
                "gitlab",
                "https://gitlab.com/-/user_settings/applications",
                &gitlab,
                &p,
                window,
                cx,
            ));
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
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .px(px(24.))
                    .py(px(16.))
                    .child(
                        control("back", &p)
                            .group("back")
                            .aria_label(t(cx, "settings.title").to_string())
                            .tooltip(tip(&t(cx, "settings.title"), &p))
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx)))
                            .child(
                                icon(BACK, 20., p.fg4)
                                    .group_hover("back", move |s| s.text_color(back_hover)),
                            ),
                    )
                    .child(
                        div()
                            .ts(LG)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.fg)
                            .child(t(cx, "settings.title").to_string()),
                    ),
            )
            .child(scroll_area("settings", &self.scroll, &p, body))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(12.))
                    .border_t_1()
                    .border_color(p.border)
                    .px(px(24.))
                    .py(px(16.))
                    .when(self.saved, |row| {
                        row.child(
                            div()
                                .mr_auto()
                                .ts(SM)
                                .text_color(p.success_fg)
                                .child(t(cx, "settings.saved").to_string()),
                        )
                    })
                    .when(!self.save_error.is_empty(), |row| {
                        row.child(
                            div()
                                .mr_auto()
                                .max_w(px(448.))
                                .ts(SM)
                                .text_color(p.fail)
                                .child(self.save_error.clone()),
                        )
                    })
                    .child(
                        btn_subtle("cancel", &p)
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx)))
                            .child(t(cx, "settings.cancel").to_string()),
                    )
                    .child(
                        btn_primary("save", &p, self.saving)
                            .when(!self.saving, |b| {
                                b.on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                            })
                            .child(
                                t(
                                    cx,
                                    if self.saving {
                                        "settings.saving"
                                    } else {
                                        "settings.save"
                                    },
                                )
                                .to_string(),
                            ),
                    ),
            )
    }
}
