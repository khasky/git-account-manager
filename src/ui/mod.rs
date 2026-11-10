//! The window: the profile list, the profile form and the settings page.

mod app;
mod form;
mod helpers;
mod i18n;
mod platform;
mod repos;
#[cfg(test)]
mod screenshots;
mod settings;
mod theme;
mod widgets;

use std::future::Future;

use futures::channel::mpsc::{unbounded, UnboundedReceiver};
use futures::StreamExt;
use gpui_kit::*;

use crate::events::Event;
use crate::tray::{Command, Labels, Tray};

pub use app::AppView;
use i18n::I18n;
use theme::{is_dark, Palette, Prefs};

pub const REPOSITORY: &str = "https://github.com/khasky/git-account-manager";

/// Runs blocking backend work on the runtime's thread pool.
pub fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> impl Future<Output = T> {
    let handle = crate::runtime().spawn_blocking(work);
    async move { handle.await.expect("backend task panicked") }
}

/// Runs a network command on the runtime `reqwest` needs.
pub fn tokio<T: Send + 'static>(
    work: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = T> {
    let handle = crate::runtime().spawn(work);
    async move { handle.await.expect("backend task panicked") }
}

/// The colours for the theme the window is showing now.
pub fn palette(window: &Window, cx: &App) -> Palette {
    Palette::new(is_dark(cx.global::<Prefs>().theme, window))
}

pub fn t(cx: &App, key: &str) -> String {
    cx.global::<I18n>().t(key).to_string()
}

pub fn f(cx: &App, key: &str, vars: &[(&str, &str)]) -> String {
    cx.global::<I18n>().f(key, vars)
}

struct Main {
    window: Option<AnyWindowHandle>,
    view: Option<Entity<AppView>>,
    tray: Option<Tray>,
}

impl Global for Main {}

pub fn run(mut events: UnboundedReceiver<Event>) {
    application().with_assets(widgets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        let prefs = Prefs::load();
        let lang = prefs
            .language
            .clone()
            .unwrap_or_else(|| i18n::detect(&i18n::system_language()).to_string());
        cx.set_global(I18n::new(&lang));
        cx.set_global(prefs);

        let (commands, mut tray_commands) = unbounded();
        let tray = Tray::new(commands)
            .inspect_err(|error| eprintln!("Cannot create the tray icon: {error}"))
            .ok();
        // Without a tray icon, closing the window is the only way to quit.
        cx.set_quit_mode(if tray.is_some() {
            QuitMode::Explicit
        } else {
            QuitMode::LastWindowClosed
        });
        cx.set_global(Main {
            window: None,
            view: None,
            tray,
        });
        refresh_tray(cx);
        show_main(cx);

        cx.spawn(async move |cx| {
            while let Some(event) = events.next().await {
                cx.update(|cx| handle_event(event, cx));
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            while let Some(command) = tray_commands.next().await {
                cx.update(|cx| handle_command(command, cx));
            }
        })
        .detach();
    });
}

fn handle_event(event: Event, cx: &mut App) {
    match event {
        Event::Show => show_main(cx),
        Event::Clipboard(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        Event::ProfilesChanged => {
            refresh_tray(cx);
            if let Some(view) = cx.global::<Main>().view.clone() {
                view.update(cx, |view, cx| view.load_profiles(cx));
            }
        }
        Event::SaveProgress(progress) => {
            if let Some(view) = cx.global::<Main>().view.clone() {
                view.update(cx, |view, cx| view.save_progress(progress, cx));
            }
        }
    }
}

fn handle_command(command: Command, cx: &mut App) {
    match command {
        Command::Show => show_main(cx),
        Command::Quit => cx.quit(),
        Command::Activate(id) => {
            cx.spawn(async move |cx| {
                if let Err(error) = blocking(move || crate::commands::activate_profile(&id)).await {
                    eprintln!("Cannot activate the profile: {error}");
                    cx.update(refresh_tray);
                }
            })
            .detach();
        }
    }
}

/// The tray menu follows the profiles and the window's language.
pub fn refresh_tray(cx: &mut App) {
    let i18n = cx.global::<I18n>();
    let labels = Labels {
        show: i18n.t("tray.show").into(),
        quit: i18n.t("tray.quit").into(),
        active_prefix: i18n.t("tray.activePrefix").into(),
        no_active: i18n.t("tray.noActiveProfile").into(),
    };
    cx.spawn(async move |cx| {
        let Ok(profiles) = blocking(crate::commands::get_profiles).await else {
            return;
        };
        cx.update(|cx| {
            if let Some(Err(error)) = cx
                .global_mut::<Main>()
                .tray
                .as_mut()
                .map(|tray| tray.refresh(&profiles, &labels))
            {
                eprintln!("Cannot update the tray menu: {error}");
            }
        });
    })
    .detach();
}

/// Switches the interface language and remembers the choice.
pub fn set_language(lang: &str, cx: &mut App) {
    cx.set_global(I18n::new(lang));
    let prefs = cx.global_mut::<Prefs>();
    prefs.language = Some(lang.to_string());
    prefs.save();
    refresh_tray(cx);
    cx.refresh_windows();
}

pub fn set_theme(theme: theme::Theme, cx: &mut App) {
    let prefs = cx.global_mut::<Prefs>();
    prefs.theme = theme;
    prefs.save();
    cx.refresh_windows();
}

/// Opens the main window, or brings it forward when it is already open.
pub fn show_main(cx: &mut App) {
    if let Some(handle) = cx.global::<Main>().window {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
    }
    let existing = cx.global::<Main>().view.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(920.), px(660.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Git Account Manager".into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(560.), px(420.))),
        app_id: Some("com.khasky.git-account-manager".into()),
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        let view = existing.unwrap_or_else(|| {
            cx.new(|cx| {
                let mut view = AppView::new(window, cx);
                view.start(window, cx);
                view
            })
        });
        window.on_window_should_close(cx, |_, cx| {
            if cx.global::<Main>().tray.is_none() {
                return true;
            }
            cx.global_mut::<Main>().window = None;
            true
        });
        view.update(cx, |_, cx| {
            cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                .detach();
        });
        // Minimizing hides the window to the tray, as closing does.
        #[cfg(target_os = "windows")]
        window
            .observe_window_visibility(|visibility, _, cx| {
                if visibility == WindowVisibility::Hidden {
                    cx.defer(hide_main);
                }
            })
            .detach();
        view
    });
    match opened {
        Ok((handle, view)) => {
            let main = cx.global_mut::<Main>();
            main.window = Some(handle);
            main.view = Some(view);
        }
        Err(error) => eprintln!("Cannot open the window: {error}"),
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn hide_main(cx: &mut App) {
    if cx.global::<Main>().tray.is_none() {
        return;
    }
    if let Some(handle) = cx.global_mut::<Main>().window.take() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

/// Brings the window up for a finding nobody went looking for.
pub fn focus_window(cx: &mut App) {
    show_main(cx);
}

/// Copies text to the clipboard.
pub fn copy(text: String, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text));
}
