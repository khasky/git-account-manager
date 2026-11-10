//! The folders a profile owns, and what the doctor found wrong with them.

use gpui_kit::prelude::*;
use gpui_kit::*;

use super::form::{ProfileForm, RepoNote};
use super::helpers::tree_row;
use super::settings::panel;
use super::theme::Palette;
use super::widgets::*;
use super::{f, t, tokio};
use crate::commands;
use crate::models::{Platform, RepoRoot};

impl ProfileForm {
    /// Runs one folder action at a time: they rewrite the machine's Git config.
    fn run<T: Send + 'static>(
        &mut self,
        key: String,
        task: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
        then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.busy = key.clone();
        self.note = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio(task).await;
            let _ = this.update(cx, |this, cx| {
                this.busy.clear();
                match result {
                    Ok(value) => then(this, value, cx),
                    Err(error) => {
                        this.note = Some(RepoNote {
                            key,
                            ok: false,
                            text: f(cx, "repos.error", &[("error", &error)]),
                        })
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn scan_with(&mut self, roots: Vec<RepoRoot>, cx: &mut Context<Self>) {
        let profile = self.draft(cx);
        let scanned = roots.clone();
        self.run(
            "scan".into(),
            commands::scan_profile_folders(profile, roots),
            move |this, found, _| {
                this.repos = found;
                this.open_roots = scanned.into_iter().map(|root| (root.path, true)).collect();
            },
            cx,
        );
    }

    fn add_folder(&mut self, platforms: Vec<Platform>, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let path = path
                .display()
                .to_string()
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_string();
            let _ = this.update(cx, |this, cx| {
                if this.roots.iter().any(|root| root.path == path) {
                    return;
                }
                this.roots.push(RepoRoot {
                    path,
                    profile_id: this.profile_id.clone(),
                    platform: platforms
                        .iter()
                        .copied()
                        .find(|&platform| platform == this.default_platform)
                        .or(platforms.first().copied())
                        .unwrap_or(Platform::Github),
                    fingerprint: Vec::new(),
                });
                let roots = this.roots.clone();
                this.scan_with(roots, cx);
            });
        })
        .detach();
    }

    fn remove_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        self.roots.retain(|root| root.path != path);
        self.repos.retain(|repo| repo.root_path != path);
        cx.notify();
    }

    fn fix_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let key = format!("fix:{path}");
        let note_key = key.clone();
        self.run(
            key,
            commands::fix_folder(path),
            move |this, cleaned, cx| {
                this.note = Some(RepoNote {
                    key: note_key,
                    ok: true,
                    text: f(cx, "repos.fixed", &[("count", &cleaned.to_string())]),
                });
                this.load_doctor(cx);
            },
            cx,
        );
    }

    fn relink_folder(&mut self, path: String, new_path: String, cx: &mut Context<Self>) {
        let (from, to) = (path.clone(), new_path.clone());
        self.run(
            format!("relink:{path}"),
            commands::relink_folder(path, new_path),
            move |this, (), cx| {
                for root in this.roots.iter_mut().filter(|root| root.path == from) {
                    root.path = to.clone();
                }
                this.load_doctor(cx);
            },
            cx,
        );
    }

    fn forget_folder(&mut self, path: String, cx: &mut Context<Self>) {
        let target = path.clone();
        self.run(
            format!("forget:{path}"),
            commands::forget_folder(path),
            move |this, (), cx| {
                this.remove_folder(&target, cx);
                this.load_doctor(cx);
            },
            cx,
        );
    }

    /// One platform picker per folder, rebuilt when the connected platforms
    /// change.
    pub fn sync_root_selects(&mut self, cx: &mut Context<Self>) {
        let platforms = self.connected_platforms();
        let paths: Vec<String> = self.roots.iter().map(|root| root.path.clone()).collect();
        self.root_selects.retain(|path, _| paths.contains(path));
        for root in self.roots.clone() {
            let fresh = self
                .root_selects
                .get(&root.path)
                .is_some_and(|(options, _)| *options == platforms);
            if fresh {
                continue;
            }
            let selected = platforms
                .iter()
                .position(|&p| p == root.platform)
                .unwrap_or(0);
            let options = platforms
                .iter()
                .map(|p| SharedString::from(p.as_str()))
                .collect();
            let label = t(cx, "repos.sectionTitle");
            let select = cx.new(|_| Select::new(options, selected, true, label.into()));
            let (path, choices) = (root.path.clone(), platforms.clone());
            cx.subscribe(&select, move |this, _, event: &SelectChanged, cx| {
                for root in this.roots.iter_mut().filter(|root| root.path == path) {
                    root.platform = choices[event.0];
                }
                cx.notify();
            })
            .detach();
            self.root_selects
                .insert(root.path.clone(), (platforms.clone(), select));
        }
        let blocked = !self.busy.is_empty() || self.repos_loading;
        for (_, select) in self.root_selects.values() {
            select.update(cx, |select, _| select.disabled = blocked);
        }
    }

    fn folder(
        &self,
        root: &RepoRoot,
        platforms: &[Platform],
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let repos: Vec<_> = self
            .repos
            .iter()
            .filter(|repo| repo.root_path == root.path)
            .collect();
        let foreign = repos.iter().filter(|repo| repo.foreign_host).count();
        let open = self.open_roots.get(&root.path).copied().unwrap_or(false);
        let blocked = !self.busy.is_empty() || self.repos_loading;
        let path = root.path.clone();
        let toggle_path = root.path.clone();
        let mut summary = f(
            cx,
            "repos.folderSummary",
            &[("total", &repos.len().to_string())],
        );
        if foreign > 0 {
            summary.push_str(" · ");
            summary.push_str(&f(
                cx,
                "repos.foreignCount",
                &[("foreign", &foreign.to_string())],
            ));
        }
        let (toggle_hover, row_hover) = (p.fg2, p.raised_40);
        let tree = open.then(|| {
            if repos.is_empty() {
                return div()
                    .ts(X11)
                    .text_color(p.fg5)
                    .child(t(cx, "repos.folderEmpty"));
            }
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(repos.iter().map(|repo| {
                    let row = tree_row(repo);
                    div()
                        .id(SharedString::from(format!("repo-{}", repo.path)))
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .rounded(px(4.))
                        .pr(px(4.))
                        .pl(px(row.depth as f32 * 14. + 4.))
                        .py(px(2.))
                        .ts(X11)
                        .hover(move |s| s.bg(row_hover))
                        .child(div().flex_none().text_color(p.fg6).child(if row.depth > 0 {
                            "└"
                        } else {
                            "•"
                        }))
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .text_color(p.fg2)
                                .when(!row.prefix.is_empty(), |label| {
                                    label.child(
                                        div().text_color(p.fg6).child(format!("{}/", row.prefix)),
                                    )
                                })
                                .child(row.label.clone()),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .text_right()
                                .font_family(MONO_FONT)
                                .text_color(p.fg5)
                                .child(if repo.full_name.is_empty() {
                                    t(cx, "repos.noRemote")
                                } else {
                                    repo.full_name.clone()
                                }),
                        )
                        .when(repo.foreign_host, |row| {
                            row.child(
                                div()
                                    .flex_none()
                                    .text_color(p.warn)
                                    .child(t(cx, "repos.foreignHost")),
                            )
                        })
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(6.))
            .bg(p.raised)
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO_FONT)
                            .ts(XS)
                            .text_color(p.fg3)
                            .child(root.path.clone()),
                    )
                    .when(platforms.len() > 1, |row| {
                        row.children(
                            self.root_selects
                                .get(&root.path)
                                .map(|(_, select)| select.clone()),
                        )
                    })
                    .child(
                        btn_quiet_danger(
                            SharedString::from(format!("remove-{}", root.path)),
                            p,
                            blocked,
                        )
                        .rounded(px(6.))
                        .px(px(8.))
                        .py(px(4.))
                        .ts(XS)
                        .when(!blocked, |b| {
                            b.on_click(
                                cx.listener(move |this, _, _, cx| this.remove_folder(&path, cx)),
                            )
                        })
                        .child(t(cx, "repos.remove")),
                    ),
            )
            .child(
                control(SharedString::from(format!("toggle-{}", root.path)), p)
                    .aria_expanded(open)
                    .flex()
                    .w_full()
                    .items_center()
                    .gap(px(4.))
                    .border_t_1()
                    .border_color(p.border)
                    .pt(px(8.))
                    .ts(X11)
                    .text_color(p.fg4)
                    .hover(move |s| s.text_color(toggle_hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let open = this.open_roots.entry(toggle_path.clone()).or_insert(false);
                        *open = !*open;
                        cx.notify();
                    }))
                    .child(if open { "▾" } else { "▸" })
                    .child(summary),
            )
            .children(tree)
            .when(open && foreign > 0, |col| {
                col.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .ts(X11)
                        .text_color(p.warn)
                        .child(t(cx, "repos.foreignHostHint"))
                        .child(info_tip(
                            SharedString::from(format!("foreign-{}", root.path)),
                            &t(cx, "repos.foreignHostInfo"),
                            p,
                        )),
                )
            })
    }

    fn doctor(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let problems: Vec<_> = self
            .statuses
            .iter()
            .filter(|status| !status.ok && self.roots.iter().any(|root| root.path == status.path))
            .collect();
        if problems.is_empty() {
            return None;
        }
        let blocked = !self.busy.is_empty() || self.repos_loading;
        let label = |id: &str, cx: &App| {
            t(
                cx,
                match id {
                    "exists" => "repos.checkExists",
                    "rule" => "repos.checkRule",
                    "identity" => "repos.checkIdentity",
                    "local" => "repos.checkLocal",
                    _ => "repos.checkHooks",
                },
            )
        };
        let rows = problems.into_iter().map(|folder| {
            let missing = folder.checks.iter().any(|c| c.id == "exists" && !c.ok);
            let path = folder.path.clone();
            let mut actions = div()
                .mt(px(8.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.));
            if missing {
                if let Some(moved) = folder.moved_to.clone() {
                    let key = format!("relink:{path}");
                    let (from, to) = (path.clone(), moved.clone());
                    actions = actions.child(
                        btn_small_blue(SharedString::from(key.clone()), p, blocked)
                            .when(self.busy == key, |b| b.child(spinner(11., white())))
                            .when(!blocked, |b| {
                                b.on_click(cx.listener(move |this, _, _, cx| {
                                    this.relink_folder(from.clone(), to.clone(), cx)
                                }))
                            })
                            .child(f(cx, "repos.relinkTo", &[("path", &moved)])),
                    );
                }
                let key = format!("forget:{path}");
                let target = path.clone();
                let hover = p.subtle;
                actions = actions.child(
                    control(SharedString::from(key.clone()), p)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded(px(6.))
                        .bg(p.raised_40)
                        .px(px(12.))
                        .py(px(4.))
                        .ts(X11)
                        .text_color(p.fg3)
                        .when(blocked, |b| b.opacity(0.5))
                        .when(!blocked, |b| {
                            b.hover(move |s| s.bg(hover)).on_click(cx.listener(
                                move |this, _, _, cx| this.forget_folder(target.clone(), cx),
                            ))
                        })
                        .when(self.busy == key, |b| b.child(spinner(11., p.fg3)))
                        .child(t(cx, "repos.forgetFolder")),
                );
            } else {
                let key = format!("fix:{path}");
                let target = path.clone();
                actions = actions.child(
                    btn_small_blue(SharedString::from(key.clone()), p, blocked)
                        .when(self.busy == key, |b| b.child(spinner(11., white())))
                        .when(!blocked, |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                this.fix_folder(target.clone(), cx)
                            }))
                        })
                        .child(t(cx, "repos.fix")),
                );
            }
            let note = self
                .note
                .as_ref()
                .filter(|note| note.key.ends_with(&format!(":{path}")));
            div()
                .rounded(px(6.))
                .bg(p.raised)
                .p(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_baseline()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .font_family(MONO_FONT)
                                .ts(XS)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.fg2)
                                .child(folder.path.clone()),
                        )
                        .child(
                            div()
                                .ts(X11)
                                .text_color(p.fg5)
                                .child(folder.expected_email.clone()),
                        ),
                )
                .child(div().mt(px(8.)).flex().flex_col().gap(px(4.)).children(
                    folder.checks.iter().filter(|c| !c.ok).map(|check| {
                        div()
                            .flex()
                            .items_baseline()
                            .justify_between()
                            .gap(px(12.))
                            .ts(X11)
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(p.fail)
                                    .child(format!("✗ {}", label(&check.id, cx))),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_right()
                                    .text_color(p.fg4)
                                    .child(check.detail.clone()),
                            )
                    }),
                ))
                .when(!folder.overrides.is_empty(), |col| {
                    col.child(
                        div()
                            .mt(px(8.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .border_t_1()
                            .border_color(p.border)
                            .pt(px(8.))
                            .children(folder.overrides.iter().map(|repo| {
                                div()
                                    .flex()
                                    .items_baseline()
                                    .justify_between()
                                    .gap(px(12.))
                                    .ts(X11)
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_color(p.fg3)
                                            .child(repo.relative.clone()),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_right()
                                            .font_family(MONO_FONT)
                                            .text_color(p.fg5)
                                            .child(repo.detail.clone()),
                                    )
                            })),
                    )
                })
                .when(!folder.bypassed.is_empty(), |col| {
                    col.child(
                        div()
                            .mt(px(8.))
                            .flex()
                            .items_start()
                            .gap(px(6.))
                            .ts(X11)
                            .text_color(p.warn)
                            .child(f(
                                cx,
                                "repos.bypassedCount",
                                &[("count", &folder.bypassed.len().to_string())],
                            ))
                            .child(info_tip(
                                SharedString::from(format!("bypassed-{path}")),
                                &t(cx, "repos.bypassedInfo"),
                                p,
                            )),
                    )
                })
                .child(actions)
                .when(missing && folder.moved_to.is_none(), |col| {
                    col.child(
                        div()
                            .mt(px(8.))
                            .ts(X11)
                            .text_color(p.fg5)
                            .child(t(cx, "repos.movedUnknown")),
                    )
                })
                .children(note.map(|note| {
                    div()
                        .mt(px(8.))
                        .ts(X11)
                        .text_color(if note.ok { p.success_fg } else { p.danger_fg })
                        .child(note.text.clone())
                }))
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .border_t_1()
                .border_color(p.border)
                .pt(px(12.))
                .child(
                    div()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .ts(XS)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.fg2)
                                .child(t(cx, "repos.doctorTitle"))
                                .child(info_tip("doctor-info", &t(cx, "repos.doctorInfo"), p)),
                        )
                        .child(
                            div()
                                .ts(X11)
                                .text_color(p.fg5)
                                .child(t(cx, "repos.doctorHint")),
                        ),
                )
                .child(div().flex().flex_col().gap(px(8.)).children(rows)),
        )
    }

    pub fn repos_panel(&self, platforms: &[Platform], p: &Palette, cx: &mut Context<Self>) -> Div {
        let blocked = !self.busy.is_empty() || self.repos_loading;
        let scanning = self.busy == "scan";
        let add_platforms = platforms.to_vec();
        let body = if self.repos_loading {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .ts(XS)
                .text_color(p.fg5)
                .child(spinner(12., p.fg5))
                .child(t(cx, "repos.loading"))
        } else if self.roots.is_empty() {
            div().ts(XS).text_color(p.fg5).child(t(cx, "repos.noRoots"))
        } else {
            div().flex().flex_col().gap(px(12.)).children(
                self.roots
                    .iter()
                    .map(|root| self.folder(root, platforms, p, cx)),
            )
        };
        let general_note = self.note.as_ref().filter(|note| !note.key.contains(':'));
        panel(p)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.fg2)
                                    .child(t(cx, "repos.sectionTitle"))
                                    .child(info_tip("repos-info", &t(cx, "repos.sectionInfo"), p)),
                            )
                            .child(
                                div()
                                    .ts(XS)
                                    .text_color(p.fg5)
                                    .child(t(cx, "repos.sectionHint")),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .gap(px(8.))
                            .when(!self.roots.is_empty(), |row| {
                                row.child(
                                    btn_raised_sm("rescan", p, blocked)
                                        .when(scanning, |b| b.child(spinner(12., p.fg2)))
                                        .when(!blocked, |b| {
                                            b.on_click(cx.listener(|this, _, _, cx| {
                                                let roots = this.roots.clone();
                                                this.scan_with(roots, cx)
                                            }))
                                        })
                                        .child(t(
                                            cx,
                                            if scanning {
                                                "repos.scanning"
                                            } else {
                                                "repos.rescan"
                                            },
                                        )),
                                )
                            })
                            .child(
                                btn_raised_sm("add-folder", p, blocked)
                                    .when(!blocked, |b| {
                                        b.on_click(cx.listener(move |this, _, _, cx| {
                                            this.add_folder(add_platforms.clone(), cx)
                                        }))
                                    })
                                    .child(t(cx, "repos.addFolder")),
                            ),
                    ),
            )
            .child(body)
            .children(self.doctor(p, cx))
            .children(general_note.map(|note| {
                div()
                    .border_t_1()
                    .border_color(p.border)
                    .pt(px(8.))
                    .ts(X11)
                    .text_color(if note.ok { p.fg3 } else { p.danger_fg })
                    .child(note.text.clone())
            }))
    }
}
