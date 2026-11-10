//! One platform's section of the profile form.

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::form::{ProfileForm, Section, SectionError};
use super::helpers::{
    azure_keys_page_url, clock, profile_url, token_page_url, HUGGINGFACE_KEYS_URL,
};
use super::settings::panel;
use super::theme::{hex, Palette, BLUE_500, BLUE_600, EMERALD_500, EMERALD_600};
use super::widgets::*;
use super::{f, t};
use crate::models::Platform;

/// The blue full-width "Connect with …" button.
fn connect_button(id: &'static str, p: &Palette, disabled: bool) -> Stateful<Div> {
    control(id, p)
        .w_full()
        .flex()
        .justify_center()
        .rounded(px(6.))
        .bg(hex(BLUE_600))
        .px(px(16.))
        .py(px(10.))
        .ts(SM)
        .font_weight(FontWeight::MEDIUM)
        .text_color(white())
        .when(disabled, |b| b.opacity(0.5).cursor_default())
        .when(!disabled, |b| b.hover(|s| s.bg(hex(BLUE_500))))
}

fn label(text: String, p: &Palette) -> Div {
    div().mb(px(4.)).ts(XS).text_color(p.fg4).child(text)
}

impl ProfileForm {
    fn section_error(
        &self,
        platform: Platform,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let section = self.section(platform);
        match &section.error {
            SectionError::None => None,
            SectionError::Settings => {
                let view = cx.entity().downgrade();
                Some(div().ts(XS).child(rich(
                    SharedString::from(format!("settings-error-{}", platform.as_str())),
                    f(
                        cx,
                        "form.errSettingsRequired",
                        &[("platform", platform.label())],
                    ),
                    p.danger_fg,
                    p.fg3,
                    p,
                    Link::Action(Box::new(move |_, cx| {
                        let _ = view.update(cx, |_, cx| cx.emit(super::form::FormEvent::Settings));
                    })),
                )))
            }
            SectionError::Message(text) => {
                Some(div().ts(XS).text_color(p.danger_fg).child(text.clone()))
            }
        }
    }

    /// Bitbucket offers no noreply address, so the one that hides a private
    /// address has to be set up on the account first.
    fn email_privacy(platform: Platform, p: &Palette, cx: &App) -> Option<Div> {
        (platform == Platform::Bitbucket).then(|| {
            div().ts(XS).child(rich(
                "bitbucket-privacy",
                t(cx, "form.bitbucket.emailPrivacy"),
                p.fg5,
                p.fg3,
                p,
                Link::Action(Box::new(|_, cx| {
                    cx.open_url("https://bitbucket.org/account/settings/email/")
                })),
            ))
        })
    }

    fn connect_view(
        &self,
        platform: Platform,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let section = self.section(platform);
        let label = platform.label();
        let id = platform.as_str();
        if matches!(platform, Platform::Github | Platform::Gitlab) {
            let cancel = |p: &Palette, cx: &mut Context<Self>| {
                let (hover_bg, hover_fg) = (p.hover, p.fg);
                control(SharedString::from(format!("cancel-auth-{id}")), p)
                    .flex_none()
                    .rounded(px(6.))
                    .bg(p.subtle)
                    .px(px(12.))
                    .py(px(4.))
                    .ts(XS)
                    .text_color(p.fg3)
                    .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                    .on_click(cx.listener(move |this, _, _, cx| match platform {
                        Platform::Github => this.cancel_github(cx),
                        _ => this.cancel_gitlab(cx),
                    }))
                    .child(t(cx, "form.cancel"))
            };
            let waiting = |key: &str, cx: &mut Context<Self>| {
                div()
                    .flex()
                    .ts(XS)
                    .text_color(p.fg4)
                    .child(t(cx, key))
                    .when(section.countdown > 0, |row| {
                        row.child(
                            div()
                                .ml(px(4.))
                                .text_color(p.fg5)
                                .child(format!("({})", clock(section.countdown))),
                        )
                    })
            };
            let info = || {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.info_border)
                    .bg(p.info_bg)
                    .p(px(12.))
            };
            let body = if let Some(device) = &section.device {
                info()
                    .child(
                        div()
                            .ts(SM)
                            .text_color(p.fg3)
                            .child(t(cx, "form.enterCode")),
                    )
                    .child(
                        // `tracking-widest`: a tenth of the size between letters.
                        div()
                            .flex()
                            .font_family(MONO_FONT)
                            .ts(XXL)
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.link)
                            .children(
                                device
                                    .user_code
                                    .chars()
                                    .map(|c| div().mr(px(2.4)).child(c.to_string())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(waiting("form.waitingAuth", cx))
                            .child(cancel(p, cx)),
                    )
            } else if section.connecting {
                info()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(waiting("form.waitingBrowser", cx))
                            .child(cancel(p, cx)),
                    )
                    .when(platform == Platform::Gitlab, |info| {
                        info.child(
                            div()
                                .ts(XS)
                                .text_color(p.fg5)
                                .child(t(cx, "form.gitlabClipboardHint")),
                        )
                    })
            } else {
                div().child(
                    connect_button(
                        if platform == Platform::Github {
                            "connect-github"
                        } else {
                            "connect-gitlab"
                        },
                        p,
                        false,
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.connect(platform, window, cx)),
                    )
                    .child(f(cx, "form.connectWith", &[("platform", label)])),
                )
            };
            return div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(body)
                .children(self.section_error(platform, p, cx));
        }

        let (help, first, scopes) = match platform {
            Platform::Azure => ("form.azure.help", true, "form.azure.scopesHint"),
            Platform::Codeberg => ("form.codeberg.help", false, "form.codeberg.scopesHint"),
            Platform::Huggingface => (
                "form.huggingface.help",
                false,
                "form.huggingface.scopesHint",
            ),
            _ => ("form.bitbucket.help", true, "form.bitbucket.scopesHint"),
        };
        let organization = if platform == Platform::Azure {
            value(&section.first, cx).trim().to_string()
        } else {
            String::new()
        };
        let help_url = token_page_url(platform, &organization);
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().ts(XS).child(rich(
                SharedString::from(format!("help-{id}")),
                t(cx, help),
                p.fg5,
                p.fg3,
                p,
                Link::Action(Box::new(move |_, cx| cx.open_url(&help_url))),
            )))
            .when(first, |col| {
                col.child(field(&section.first, Field::Sm, p, window, cx))
            })
            .child(field(&section.token, Field::Sm, p, window, cx))
            .child(div().ts(XS).text_color(p.fg5).child(t(cx, scopes)))
            .children(Self::email_privacy(platform, p, cx))
            .child(
                connect_button(
                    match platform {
                        Platform::Bitbucket => "connect-bitbucket",
                        Platform::Azure => "connect-azure",
                        Platform::Codeberg => "connect-codeberg",
                        _ => "connect-huggingface",
                    },
                    p,
                    section.connecting,
                )
                .when(!section.connecting, |b| {
                    b.on_click(
                        cx.listener(move |this, _, window, cx| this.connect(platform, window, cx)),
                    )
                })
                .child(if section.connecting {
                    t(cx, "form.waitingAuth")
                } else {
                    f(cx, "form.connectWith", &[("platform", label)])
                }),
            )
            .children(self.section_error(platform, p, cx))
    }

    fn email_choice(
        &self,
        platform: Platform,
        email: &str,
        badge: String,
        badge_colors: (Hsla, Hsla),
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let section = self.section(platform);
        let selected = value(&section.git_email, cx) == email;
        let input = section.git_email.clone();
        let pick = email.to_string();
        control(
            SharedString::from(format!("email-{}-{email}", platform.as_str())),
            p,
        )
        .flex()
        .w_full()
        .items_center()
        .gap(px(8.))
        .rounded(px(6.))
        .border_1()
        .px(px(10.))
        .py(px(6.))
        .ts(XS)
        .when(selected, |b| {
            b.border_color(p.selected_border)
                .bg(p.selected_bg)
                .text_color(p.selected_fg)
        })
        .when(!selected, |b| {
            b.border_color(p.border_s).bg(p.input).text_color(p.fg3)
        })
        .on_click(cx.listener(move |_, _, window, cx| {
            let pick = pick.clone();
            input.update(cx, |s, cx| s.set_value(pick, window, cx));
            cx.notify();
        }))
        .child(
            div()
                .flex_none()
                .rounded(px(4.))
                .px(px(4.))
                .py(px(2.))
                .ts(Size(10., 15.))
                .font_weight(FontWeight::MEDIUM)
                .bg(badge_colors.0)
                .text_color(badge_colors.1)
                .child(badge),
        )
        .child(div().min_w_0().truncate().child(email.to_string()))
    }

    fn key_view(&self, platform: Platform, p: &Palette, cx: &mut Context<Self>) -> Div {
        let section = self.section(platform);
        let label = platform.label();
        let id = platform.as_str();
        if !section.private_key.is_empty() && section.key_uploaded {
            let public = section.public_key.clone();
            let copied = self.copied.as_deref() == Some(public.as_str()) && !public.is_empty();
            let hover = p.hover;
            return div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .rounded(px(6.))
                .border_1()
                .border_color(p.active_border)
                .bg(p.active_bg)
                .px(px(12.))
                .py(px(8.))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(icon(CHECK, 16., p.success_icon))
                        .child(div().ts(XS).text_color(p.success_fg).child(f(
                            cx,
                            "form.uploadedTo",
                            &[
                                ("file", file_name(&section.private_key)),
                                ("platform", label),
                            ],
                        ))),
                )
                .when(!public.is_empty(), |row| {
                    row.child(
                        control(SharedString::from(format!("copy-key-{id}")), p)
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(p.border_s)
                            .bg(p.input)
                            .px(px(10.))
                            .py(px(4.))
                            .ts(XS)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.fg2)
                            .hover(move |s| s.bg(hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy_public_key(public.clone(), cx)
                            }))
                            .child(icon(COPY, 14., p.fg2))
                            .child(t(
                                cx,
                                if copied {
                                    "form.copied"
                                } else {
                                    "form.copyPublicKey"
                                },
                            )),
                    )
                });
        }

        let toggle_button =
            |key: &'static str, selected: bool, generate: bool, cx: &mut Context<Self>| {
                control(SharedString::from(format!("{key}-{id}")), p)
                    .rounded(px(6.))
                    .px(px(8.))
                    .py(px(4.))
                    .ts(XS)
                    .when(selected, |b| b.bg(hex(BLUE_600)).text_color(white()))
                    .when(!selected, |b| b.bg(p.subtle).text_color(p.fg3))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let section = this.section_mut(platform);
                        section.generate = generate;
                        // Leaving a browsed key behind would let a path the account
                        // never attached reach the profile it is saved into.
                        if generate {
                            section.selected_key.clear();
                            section.private_key.clear();
                            section.public_key.clear();
                        }
                        cx.notify();
                    }))
                    .child(t(cx, key))
            };
        let manual = match platform {
            Platform::Azure => {
                let url = azure_keys_page_url(&section.username);
                Some(("form.azure.keyManual", url))
            }
            Platform::Huggingface => Some((
                "form.huggingface.keyManual",
                HUGGINGFACE_KEYS_URL.to_string(),
            )),
            _ => None,
        };
        let busy = section.key_busy;
        let body = if section.generate {
            control(SharedString::from(format!("generate-{id}")), p)
                .w_full()
                .flex()
                .justify_center()
                .rounded(px(6.))
                .bg(hex(EMERALD_600))
                .px(px(12.))
                .py(px(8.))
                .ts(SM)
                .font_weight(FontWeight::MEDIUM)
                .text_color(white())
                .when(busy, |b| {
                    b.opacity(0.5).cursor(CursorStyle::OperationNotAllowed)
                })
                .when(!busy, |b| {
                    b.hover(|s| s.bg(hex(EMERALD_500))).on_click(
                        cx.listener(move |this, _, _, cx| this.generate_key(platform, cx)),
                    )
                })
                .child(if busy {
                    t(cx, "form.keyBusy")
                } else {
                    f(cx, "form.generateAddTo", &[("platform", label)])
                })
                .into_any_element()
        } else {
            let list = if self.ssh_keys.is_empty() {
                div()
                    .ts(XS)
                    .text_color(p.fg5)
                    .child(t(cx, "form.noSshKeys"))
                    .into_any_element()
            } else {
                div()
                    .id(SharedString::from(format!("keys-{id}")))
                    .max_h(px(112.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(self.ssh_keys.iter().map(|key| {
                        let selected = section.selected_key == key.private_key_path;
                        let copied = self.copied.as_deref() == Some(key.public_key_path.as_str());
                        let pick = key.clone();
                        let public = key.public_key_path.clone();
                        let hover_fg = p.fg2;
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                control(
                                    SharedString::from(format!(
                                        "key-{id}-{}",
                                        key.private_key_path
                                    )),
                                    p,
                                )
                                .min_w_0()
                                .flex_1()
                                .rounded(px(6.))
                                .border_1()
                                .px(px(8.))
                                .py(px(6.))
                                .ts(XS)
                                .when(selected, |b| {
                                    b.border_color(p.selected_border)
                                        .bg(p.selected_bg)
                                        .text_color(p.selected_fg)
                                })
                                .when(!selected, |b| {
                                    b.border_color(p.border_s).bg(p.input).text_color(p.fg3)
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let section = this.section_mut(platform);
                                    section.selected_key = pick.private_key_path.clone();
                                    section.private_key = pick.private_key_path.clone();
                                    section.public_key = pick.public_key_path.clone();
                                    cx.notify();
                                }))
                                .child(key.name.clone()),
                            )
                            .child(
                                control(
                                    SharedString::from(format!(
                                        "copy-{id}-{}",
                                        key.public_key_path
                                    )),
                                    p,
                                )
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .border_1()
                                .px(px(8.))
                                .py(px(6.))
                                .tooltip(tip(&t(cx, "form.copyPublicKey"), p))
                                .when(copied, |b| {
                                    b.border_color(p.selected_border).bg(p.selected_bg)
                                })
                                .when(!copied, |b| b.border_color(p.border_s).bg(p.input))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy_public_key(public.clone(), cx)
                                }))
                                .group(SharedString::from(format!("copy-{}", key.public_key_path)))
                                .child(
                                    icon(COPY, 14., if copied { p.selected_fg } else { p.fg4 })
                                        .when(!copied, |i| {
                                            i.group_hover(
                                                SharedString::from(format!(
                                                    "copy-{}",
                                                    key.public_key_path
                                                )),
                                                move |s| s.text_color(hover_fg),
                                            )
                                        }),
                                ),
                            )
                    }))
                    .into_any_element()
            };
            let hover = p.hover;
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(list)
                .when(
                    !section.private_key.is_empty() && !section.key_uploaded,
                    |col| {
                        col.child(
                            div().flex().child(
                                control(SharedString::from(format!("upload-{id}")), p)
                                    .rounded(px(6.))
                                    .bg(p.subtle)
                                    .px(px(12.))
                                    .py(px(6.))
                                    .ts(XS)
                                    .text_color(p.fg2)
                                    .when(busy, |b| b.opacity(0.5))
                                    .when(!busy, |b| {
                                        b.hover(move |s| s.bg(hover)).on_click(cx.listener(
                                            move |this, _, _, cx| {
                                                this.upload_key(platform, false, cx)
                                            },
                                        ))
                                    })
                                    .child(if busy {
                                        t(cx, "form.keyBusy")
                                    } else {
                                        f(cx, "form.uploadTo", &[("platform", label)])
                                    }),
                            ),
                        )
                    },
                )
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(toggle_button(
                        "form.generateUpload",
                        section.generate,
                        true,
                        cx,
                    ))
                    .child(toggle_button(
                        "form.useExisting",
                        !section.generate,
                        false,
                        cx,
                    )),
            )
            .children(manual.map(|(key, url)| {
                div().ts(XS).child(rich(
                    SharedString::from(format!("manual-{id}")),
                    t(cx, key),
                    p.fg5,
                    p.fg3,
                    p,
                    Link::Action(Box::new(move |_, cx| cx.open_url(&url))),
                ))
            }))
            .child(body)
    }

    fn https_view(
        &self,
        platform: Platform,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let section = self.section(platform);
        if section.https_placeholder.get() != Some(section.https_stored) {
            section.https_placeholder.set(Some(section.https_stored));
            let placeholder = t(
                cx,
                if section.https_stored {
                    "form.https.storedPlaceholder"
                } else {
                    "form.https.placeholder"
                },
            );
            section.https.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx)
            });
        }
        let hint = match platform {
            Platform::Github => "form.https.hintGithub",
            Platform::Gitlab => "form.https.hintGitlab",
            Platform::Bitbucket => "form.https.hintBitbucket",
            Platform::Azure => "form.https.hintAzure",
            Platform::Codeberg => "form.https.hintCodeberg",
            Platform::Huggingface => "form.https.hintHuggingface",
        };
        let url = token_page_url(platform, &section.username);
        let danger = p.danger_fg;
        div()
            .child(label(t(cx, "form.https.label"), p))
            .child(field(&section.https, Field::Sm, p, window, cx))
            .child(div().mt(px(4.)).ts(XS).child(rich(
                SharedString::from(format!("https-hint-{}", platform.as_str())),
                t(cx, hint),
                p.fg5,
                p.fg4,
                p,
                Link::Action(Box::new(move |_, cx| cx.open_url(&url))),
            )))
            .when(!section.https_notice.is_empty(), |col| {
                col.child(div().mt(px(4.)).ts(XS).text_color(p.warn).child(f(
                    cx,
                    "form.https.unverified",
                    &[("reason", &section.https_notice)],
                )))
            })
            .when(section.https_stored, |col| {
                col.child(
                    div().flex().child(
                        control(
                            SharedString::from(format!("https-remove-{}", platform.as_str())),
                            p,
                        )
                        .mt(px(4.))
                        .ts(XS)
                        .text_color(danger)
                        .hover(|s| s.underline())
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.remove_https_token(platform, cx)
                            }),
                        )
                        .child(t(cx, "form.https.remove")),
                    ),
                )
            })
    }

    fn connected_view(
        &self,
        platform: Platform,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let section: &Section = self.section(platform);
        let id = platform.as_str();
        let has_choices = !section.noreply_email.is_empty()
            || !section.public_email.is_empty()
            || !self.imported_email.is_empty();
        let email = if has_choices {
            let mut choices = div().flex().flex_col().gap(px(6.));
            if !section.noreply_email.is_empty() {
                choices = choices.child(self.email_choice(
                    platform,
                    &section.noreply_email,
                    t(cx, "form.noreplyBadge"),
                    (p.badge_ok_bg, p.badge_ok_fg),
                    p,
                    cx,
                ));
            }
            if !section.public_email.is_empty() && section.public_email != section.noreply_email {
                choices = choices.child(self.email_choice(
                    platform,
                    &section.public_email,
                    t(cx, "form.publicBadge"),
                    (p.subtle, p.fg3),
                    p,
                    cx,
                ));
            }
            let imported = &self.imported_email;
            if !imported.is_empty()
                && *imported != section.noreply_email
                && *imported != section.public_email
            {
                choices = choices.child(self.email_choice(
                    platform,
                    imported,
                    "git config".into(),
                    (p.subtle, p.fg3),
                    p,
                    cx,
                ));
            }
            choices.child(field(&section.git_email, Field::Xs, p, window, cx))
        } else {
            div().child(field(&section.git_email, Field::Sm, p, window, cx))
        };
        let sign = section.sign_commits;
        let signing_error = (!section.signing_error.is_empty()).then(|| {
            // Codeberg and Hugging Face sign fine but will not show the badge
            // until the user acts there: their notice is shown as it is.
            if matches!(platform, Platform::Codeberg | Platform::Huggingface) {
                div()
                    .mt(px(4.))
                    .ts(XS)
                    .text_color(p.warn)
                    .child(section.signing_error.clone())
            } else {
                div().mt(px(4.)).ts(XS).text_color(p.danger_fg).child(f(
                    cx,
                    "form.signingFailed",
                    &[
                        ("error", &section.signing_error),
                        ("platform", platform.label()),
                    ],
                ))
            }
        });
        let danger = p.danger_fg;
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().child(label(t(cx, "form.gitName"), p)).child(field(
                &section.git_name,
                Field::Sm,
                p,
                window,
                cx,
            )))
            .child(
                div()
                    .child(label(t(cx, "form.gitEmail"), p))
                    .child(email)
                    .children(
                        Self::email_privacy(platform, p, cx).map(|privacy| privacy.mt(px(6.))),
                    ),
            )
            .child(
                div()
                    .child(label(t(cx, "form.sshKey"), p))
                    .child(self.key_view(platform, p, cx)),
            )
            .child(self.https_view(platform, p, window, cx))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .ts(XS)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.fg3)
                                    .child(t(cx, "form.signCommits")),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .ts(XS)
                                    .text_color(p.fg5)
                                    .child(t(cx, "form.signCommitsHint")),
                            )
                            .children(signing_error),
                    )
                    .child(
                        checkbox(SharedString::from(format!("sign-{id}")), sign, p)
                            .mt(px(2.))
                            .aria_label(t(cx, "form.signCommits"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_sign(platform, !sign, cx)
                            })),
                    ),
            )
            .children(self.section_error(platform, p, cx))
            .child(
                div().flex().child(
                    control(SharedString::from(format!("disconnect-{id}")), p)
                        .ts(XS)
                        .text_color(danger)
                        .hover(|s| s.underline())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.ask_disconnect(platform, cx)),
                        )
                        .child(t(cx, "form.disconnect")),
                ),
            )
    }

    pub fn platform_section(
        &self,
        platform: Platform,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let section = self.section(platform);
        let username = section.username.clone();
        let (link, link_hover) = (p.link, p.link_hover);
        panel(p)
            .gap_0()
            .child(
                div()
                    .mb(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.fg2)
                            .child(platform.label()),
                    )
                    .when(section.connected, |row| {
                        row.child(
                            control(
                                SharedString::from(format!("profile-{}", platform.as_str())),
                                p,
                            )
                            .ts(SM)
                            .text_color(link)
                            .hover(move |s| s.text_color(link_hover).underline())
                            .on_click(move |_, _, cx| {
                                cx.open_url(&profile_url(platform, &username))
                            })
                            .child(format!("@{}", section.username)),
                        )
                    }),
            )
            .when(
                section.connected && !section.username_notice.is_empty(),
                |col| {
                    col.child(div().mb(px(12.)).ts(XS).text_color(p.danger_fg).child(f(
                        cx,
                        "form.usernameNotice",
                        &[
                            ("username", &section.username),
                            ("error", &section.username_notice),
                        ],
                    )))
                },
            )
            .child(if section.connected {
                self.connected_view(platform, p, window, cx)
            } else {
                self.connect_view(platform, p, window, cx)
            })
    }
}
