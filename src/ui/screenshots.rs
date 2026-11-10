//! Regenerates `screenshots/` with demonstration data: `pnpm screenshots`.
//! Nothing here reads the machine's profiles, keys or Git config.

use std::path::Path;
use std::time::Duration;

use gpui_kit::*;
use serde_json::json;

use super::app::{AppView, Page};
use super::form::ProfileForm;
use super::i18n::I18n;
use super::settings::SettingsPage;
use super::theme::{Prefs, Theme};
use super::widgets::Assets;
use crate::doctor::FolderStatus;
use crate::gh::GhProbe;
use crate::guard::GuardStatus;
use crate::models::{
    DeviceCodeResponse, GuardSettings, OAuthSettings, Platform, Profile, RepoRoot, SshKeyInfo,
};
use crate::openssh_integration::OpenSshIntegrationProbe;
use crate::repos::FolderRepo;

const HOME: &str = "C:/Users/nadia";
const WORK: &str = "8f2b1c40-3d5e-4a91-b8c2-71ea9d4f6a30";
const NOREPLY: &str = "48213574+nadia-acme@users.noreply.github.com";

fn account(username: &str, name: &str, email: &str, key: &str) -> serde_json::Value {
    json!({
        "username": username, "git_name": name, "git_email": email,
        "ssh_private_key_path": format!("{HOME}/.ssh/{key}"),
        "ssh_public_key_path": format!("{HOME}/.ssh/{key}.pub"),
        "sign_commits": true,
    })
}

fn profiles() -> Vec<Profile> {
    serde_json::from_value(json!([
        { "id": WORK, "name": "Work", "default_platform": "github", "is_active": true,
          "github": account("nadia-acme", "Nadia Petrenko", NOREPLY, "id_ed25519_github_work"),
          "gitlab": account("n.petrenko", "Nadia Petrenko", "n.petrenko@acme.example", "id_ed25519_gitlab_work"),
          "bitbucket": account("nadia_acme", "Nadia Petrenko", "nadia@acme.example", "id_ed25519_bitbucket_work") },
        { "id": "1c7d94a2-5b60-4e83-9f11-2ad6c803be74", "name": "Open Source",
          "default_platform": "github", "is_active": false,
          "github": account("nadiacodes", "Nadia P.", "9128374+nadiacodes@users.noreply.github.com", "id_ed25519_github_oss") },
        { "id": "b53e08f7-9c42-4d16-a70b-5e81f2947cd3", "name": "Client \u{b7} Northwind",
          "default_platform": "gitlab", "is_active": false,
          "gitlab": account("nadia-nw", "Nadia Petrenko", "n.petrenko@northwind.example", "id_ed25519_gitlab_nw"),
          "bitbucket": account("nadia-northwind", "Nadia Petrenko", "nadia@northwind.example", "id_ed25519_bitbucket_nw") },
    ]))
    .unwrap()
}

fn ssh_keys() -> Vec<SshKeyInfo> {
    [
        "id_ed25519_github_work",
        "id_ed25519_gitlab_work",
        "id_ed25519_bitbucket_work",
        "id_ed25519_github_oss",
        "id_ed25519_gitlab_nw",
        "id_ed25519_bitbucket_nw",
        "id_rsa",
    ]
    .map(|name| SshKeyInfo {
        name: name.into(),
        private_key_path: format!("{HOME}/.ssh/{name}"),
        public_key_path: format!("{HOME}/.ssh/{name}.pub"),
    })
    .into()
}

fn roots() -> Vec<RepoRoot> {
    [
        ("D:/work/acme", Platform::Github),
        ("D:/work/acme-labs", Platform::Gitlab),
        ("D:/work/old-clients", Platform::Bitbucket),
    ]
    .map(|(path, platform)| RepoRoot {
        path: path.into(),
        profile_id: WORK.into(),
        platform,
        fingerprint: Vec::new(),
    })
    .into()
}

fn repos() -> Vec<FolderRepo> {
    let repo = |root: &str, relative: &str, full: &str, foreign: bool| FolderRepo {
        path: format!("{root}/{relative}"),
        relative: relative.into(),
        name: relative.rsplit('/').next().unwrap().into(),
        root_path: root.into(),
        remote_url: if full.is_empty() {
            String::new()
        } else {
            format!("git@host:{full}.git")
        },
        full_name: full.into(),
        foreign_host: foreign,
    };
    vec![
        repo(
            "D:/work/acme",
            "billing-api",
            "acme-corp/billing-api",
            false,
        ),
        repo(
            "D:/work/acme",
            "design-system",
            "nadia-acme/design-system",
            false,
        ),
        repo(
            "D:/work/acme",
            "legacy/etl-jobs",
            "acme-corp/etl-jobs",
            false,
        ),
        repo("D:/work/acme", "legacy/reports", "", false),
        repo(
            "D:/work/acme",
            "vendor/infra-terraform",
            "shared-tools/infra-terraform",
            true,
        ),
        repo(
            "D:/work/acme-labs",
            "runner-images",
            "acme-labs/runner-images",
            false,
        ),
        repo(
            "D:/work/acme-labs",
            "metrics-agent",
            "acme-labs/metrics-agent",
            false,
        ),
    ]
}

fn statuses() -> Vec<FolderStatus> {
    let check = |id: &str, ok: bool, detail: &str| json!({ "id": id, "ok": ok, "detail": detail });
    serde_json::from_value(json!([
        { "path": "D:/work/acme", "profile_id": WORK, "profile_name": "Work", "platform": "github",
          "expected_email": NOREPLY, "repos": 5,
          "checks": [check("exists", true, "D:/work/acme"), check("rule", true, "includeIf gitdir:D:/work/acme/"),
                     check("identity", true, NOREPLY), check("local", false, "2 repositories set their own user.email"),
                     check("guard", true, "global hook")],
          "overrides": [
            { "path": "D:/work/acme/legacy/etl-jobs", "relative": "legacy/etl-jobs", "detail": "user.email = nadia.p@mail.example" },
            { "path": "D:/work/acme/billing-api", "relative": "billing-api", "detail": "user.name = Nadia" }],
          "bypassed": [{ "path": "D:/work/acme/design-system", "relative": "design-system", "detail": ".husky/_" }],
          "moved_to": null, "ok": false },
        { "path": "D:/work/acme-labs", "profile_id": WORK, "profile_name": "Work", "platform": "gitlab",
          "expected_email": "n.petrenko@acme.example", "repos": 2,
          "checks": [check("exists", true, "D:/work/acme-labs"), check("rule", false, "not in ~/.gitconfig"),
                     check("identity", false, "nadia.p@mail.example"), check("local", true, ""), check("guard", true, "global hook")],
          "overrides": [], "bypassed": [], "moved_to": null, "ok": false },
        { "path": "D:/work/old-clients", "profile_id": WORK, "profile_name": "Work", "platform": "bitbucket",
          "expected_email": "nadia@acme.example", "repos": 0,
          "checks": [check("exists", false, "D:/work/old-clients"), check("rule", true, ""), check("identity", true, ""),
                     check("local", true, ""), check("guard", true, "")],
          "overrides": [], "bypassed": [], "moved_to": "D:/archive/old-clients", "ok": false },
    ]))
    .unwrap()
}

fn settings() -> OAuthSettings {
    OAuthSettings {
        github_client_id: "Ov23liK7pQx4WmZ0aT2u".into(),
        gitlab_client_id: "a1c9f0e6b74d2385c1fa9e70d4b62c85f3197ad0e6428bc51d93f70ae82c46b9".into(),
        use_openssh_for_git_tools: true,
        switch_gh_account: true,
        use_https_credential_helper: false,
    }
}

fn guard_status() -> GuardStatus {
    serde_json::from_value(json!({
        "global_name": "", "global_email": "", "gitconfig_path": format!("{HOME}/.gitconfig"),
        "rules_written": true,
        "ssh_hosts": [{ "host": "github.com", "profile": "Work" }, { "host": "gitlab.com", "profile": "Work" },
                      { "host": "bitbucket.org", "profile": null }],
        "hooks_path": format!("{HOME}/AppData/Roaming/git-account-manager/hooks"), "hooks": "global",
        "use_config_only": true, "ok": true,
    }))
    .unwrap()
}

fn form(
    profile: Option<Profile>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<ProfileForm> {
    let edit = profile.is_some();
    cx.new(|cx| {
        let mut form = ProfileForm::new(profile, None, window, cx);
        form.ssh_keys = ssh_keys();
        form.settings = Some(settings());
        form.repos_loading = false;
        if edit {
            form.section_mut(Platform::Github).https_stored = true;
            form.roots = roots();
            form.repos = repos();
            form.statuses = statuses();
            form.open_roots = form.roots.iter().map(|r| (r.path.clone(), true)).collect();
        } else {
            let github = form.section_mut(Platform::Github);
            github.connecting = true;
            github.countdown = 899;
            github.device = Some(DeviceCodeResponse {
                device_code: "b8c0".into(),
                user_code: "WDJB-MJHT".into(),
                verification_uri: "https://github.com/login/device".into(),
                expires_in: 899,
                interval: 5,
            });
        }
        form
    })
}

fn open(scene: u32, cx: &mut App) -> WindowHandle<AppView> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(920.), px(660.)),
        ))),
        // Hidden windows do not render a frame to capture.
        focus: false,
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            let mut view = AppView::new(window, cx);
            view.profiles = profiles();
            view.loading = false;
            view.problems.insert(WORK.into(), 3);
            let work = view.profiles[0].clone();
            view.page = match scene {
                2 => Page::Settings(cx.new(|cx| {
                    let mut page = SettingsPage::new(window, cx);
                    page.set_settings(settings(), window, cx);
                    page.gh = Some(GhProbe {
                        available: true,
                        logins: vec!["nadia-acme".into(), "nadiacodes".into()],
                        active: Some("nadia-acme".into()),
                    });
                    page.openssh = Some(OpenSshIntegrationProbe {
                        available: true,
                        ssh_exe: Some("C:\\Windows\\System32\\OpenSSH\\ssh.exe".into()),
                    });
                    page.autostart = true;
                    page.guard = Some(GuardSettings {
                        unset_global_identity: true,
                        guard_commits: true,
                    });
                    page.guard_status = Some(guard_status());
                    page
                })),
                3..=5 => Page::Form(form(Some(work), window, cx)),
                6 => Page::Form(form(None, window, cx)),
                _ => Page::List,
            };
            view
        })
    })
    .unwrap()
}

/// Scene 4 puts the default-identity panel 20px under the top of the pane;
/// scene 5 shows the doctor, at the bottom of the form.
fn scroll(scene: u32, view: &mut AppView, cx: &mut Context<AppView>) {
    let Page::Form(form) = &view.page else {
        return;
    };
    form.update(cx, |form, cx| {
        let handle = &form.scroll;
        let max = handle.max_offset().y;
        let target = match scene {
            4 => handle
                .bounds_for_item(7)
                .map(|item| item.top() - handle.bounds().top() - px(20.))
                .unwrap_or(max),
            _ => max,
        };
        handle.set_offset(point(px(0.), -target.clamp(px(0.), max)));
        cx.notify();
    });
}

#[std::prelude::rust_2021::test]
#[ignore = "writes screenshots/; run with `pnpm screenshots`"]
fn capture() {
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("screenshots");
    application().with_assets(Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.set_quit_mode(QuitMode::Explicit);
        cx.set_global(I18n::new("en"));
        cx.set_global(Prefs::default());
        cx.spawn(async move |cx| {
            for (theme, name) in [(Theme::Light, "light"), (Theme::Dark, "dark")] {
                cx.update(|cx| cx.global_mut::<Prefs>().theme = theme);
                for scene in 1..=6 {
                    let window = cx.update(|cx| open(scene, cx));
                    cx.background_executor()
                        .timer(Duration::from_millis(700))
                        .await;
                    if matches!(scene, 4 | 5) {
                        window
                            .update(cx, |view, _, cx| scroll(scene, view, cx))
                            .unwrap();
                        cx.background_executor()
                            .timer(Duration::from_millis(300))
                            .await;
                    }
                    let image = window
                        .update(cx, |_, window, _| {
                            let image = window.render_to_image();
                            window.remove_window();
                            image
                        })
                        .unwrap()
                        .unwrap();
                    let path = output.join(format!("{name}{scene}.webp"));
                    image.save(&path).unwrap();
                    println!("{} {}x{}", path.display(), image.width(), image.height());
                }
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}
