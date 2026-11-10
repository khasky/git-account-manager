//! The system-tray menu: the one place a profile can be switched without
//! opening the window.

use futures::channel::mpsc::UnboundedSender;

use crate::models::Profile;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Show,
    Quit,
    Activate(String),
}

/// Localized labels for the menu, in the window's current language.
#[derive(Clone)]
pub struct Labels {
    pub show: String,
    pub quit: String,
    pub active_prefix: String,
    pub no_active: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            show: "Show Window".to_string(),
            quit: "Close Git Account Manager".to_string(),
            active_prefix: "Active:".to_string(),
            no_active: "No active profile".to_string(),
        }
    }
}

/// One menu row; `None` is a separator.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    label: String,
    enabled: bool,
    command: Option<Command>,
}

/// A disabled header naming the active identity, one entry per profile (a
/// check mark marks the active one), then Show and Quit.
fn rows(profiles: &[Profile], labels: &Labels) -> Vec<Option<Row>> {
    let header = match profiles.iter().find(|p| p.is_active) {
        Some(p) => match p.active_identity() {
            Some((name, email)) => format!("{} {} <{}>", labels.active_prefix, name, email),
            None => format!("{} {}", labels.active_prefix, p.name),
        },
        None => labels.no_active.clone(),
    };
    let mut rows = vec![
        Some(Row {
            label: header,
            enabled: false,
            command: None,
        }),
        None,
    ];
    for p in profiles {
        let mark = if p.is_active { "\u{2713} " } else { "   " };
        rows.push(Some(Row {
            label: format!("{mark}{}", p.name),
            enabled: true,
            command: Some(Command::Activate(p.id.clone())),
        }));
    }
    if !profiles.is_empty() {
        rows.push(None);
    }
    rows.push(Some(Row {
        label: labels.show.clone(),
        enabled: true,
        command: Some(Command::Show),
    }));
    rows.push(None);
    rows.push(Some(Row {
        label: labels.quit.clone(),
        enabled: true,
        command: Some(Command::Quit),
    }));
    rows
}

pub fn tooltip(profiles: &[Profile]) -> String {
    match profiles.iter().find(|p| p.is_active) {
        Some(p) => format!("Git Account Manager \u{2014} {}", p.name),
        None => "Git Account Manager".to_string(),
    }
}

struct Image {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn icon_image() -> Result<Image, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!("../icons/32x32.png")));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or("Tray icon is too large")?
    ];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("Tray icon must be an 8-bit RGBA PNG".into());
    }
    pixels.truncate(info.buffer_size());
    Ok(Image {
        rgba: pixels,
        width: info.width,
        height: info.height,
    })
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;
    use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    pub struct Tray {
        tray: TrayIcon,
    }

    fn id(command: &Command) -> String {
        match command {
            Command::Show => "show".into(),
            Command::Quit => "quit".into(),
            Command::Activate(profile) => format!("activate:{profile}"),
        }
    }

    fn command(id: &MenuId) -> Option<Command> {
        match id.as_ref() {
            "show" => Some(Command::Show),
            "quit" => Some(Command::Quit),
            other => other
                .strip_prefix("activate:")
                .map(|profile| Command::Activate(profile.to_string())),
        }
    }

    fn menu(profiles: &[Profile], labels: &Labels) -> Result<Menu, String> {
        let menu = Menu::new();
        for (index, row) in rows(profiles, labels).into_iter().enumerate() {
            let result = match row {
                None => menu.append(&PredefinedMenuItem::separator()),
                Some(row) => {
                    let item_id = row
                        .command
                        .as_ref()
                        .map(id)
                        .unwrap_or_else(|| format!("label:{index}"));
                    menu.append(&MenuItem::with_id(item_id, row.label, row.enabled, None))
                }
            };
            result.map_err(|e| e.to_string())?;
        }
        Ok(menu)
    }

    impl Tray {
        pub fn new(commands: UnboundedSender<Command>) -> Result<Self, String> {
            let image = icon_image()?;
            let icon = Icon::from_rgba(image.rgba, image.width, image.height)
                .map_err(|e| e.to_string())?;
            let menu_commands = commands.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if let Some(command) = command(&event.id) {
                    let _ = menu_commands.unbounded_send(command);
                }
            }));
            // The menu opens on a right click; a left click brings the window up.
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = commands.unbounded_send(Command::Show);
                }
            }));
            let tray = TrayIconBuilder::new()
                .with_icon(icon)
                .with_tooltip(tooltip(&[]))
                .with_menu(Box::new(menu(&[], &Labels::default())?))
                .with_menu_on_left_click(false)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(Self { tray })
        }

        pub fn refresh(&mut self, profiles: &[Profile], labels: &Labels) -> Result<(), String> {
            self.tray.set_menu(Some(Box::new(menu(profiles, labels)?)));
            self.tray
                .set_tooltip(Some(tooltip(profiles)))
                .map_err(|e| e.to_string())
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use ksni::blocking::{Handle, TrayMethods};

    struct Item {
        icon: ksni::Icon,
        tooltip: String,
        rows: Vec<Option<Row>>,
        commands: UnboundedSender<Command>,
    }

    impl ksni::Tray for Item {
        fn id(&self) -> String {
            "git-account-manager".into()
        }

        fn title(&self) -> String {
            self.tooltip.clone()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![self.icon.clone()]
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                title: self.tooltip.clone(),
                ..Default::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.commands.unbounded_send(Command::Show);
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            self.rows
                .iter()
                .map(|row| match row {
                    None => ksni::MenuItem::Separator,
                    Some(row) => {
                        let command = row.command.clone();
                        ksni::menu::StandardItem {
                            label: row.label.clone(),
                            enabled: row.enabled,
                            activate: Box::new(move |this: &mut Self| {
                                if let Some(command) = command.clone() {
                                    let _ = this.commands.unbounded_send(command);
                                }
                            }),
                            ..Default::default()
                        }
                        .into()
                    }
                })
                .collect()
        }
    }

    pub struct Tray {
        handle: Handle<Item>,
    }

    impl Tray {
        pub fn new(commands: UnboundedSender<Command>) -> Result<Self, String> {
            let image = icon_image()?;
            let mut data = image.rgba;
            // StatusNotifierItem pixmaps are ARGB.
            for pixel in data.as_chunks_mut::<4>().0 {
                pixel.rotate_right(1);
            }
            let handle = Item {
                icon: ksni::Icon {
                    width: image.width as i32,
                    height: image.height as i32,
                    data,
                },
                tooltip: tooltip(&[]),
                rows: rows(&[], &Labels::default()),
                commands,
            }
            .spawn()
            .map_err(|e| e.to_string())?;
            Ok(Self { handle })
        }

        pub fn refresh(&mut self, profiles: &[Profile], labels: &Labels) -> Result<(), String> {
            let rows = rows(profiles, labels);
            let tooltip = tooltip(profiles);
            self.handle
                .update(|item| {
                    item.rows = rows;
                    item.tooltip = tooltip;
                })
                .ok_or_else(|| "The tray service has stopped".to_string())
        }
    }
}

pub use platform::Tray;

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, name: &str, active: bool) -> Profile {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": name, "is_active": active
        }))
        .unwrap()
    }

    #[test]
    fn menu_lists_profiles_between_the_header_and_the_window_controls() {
        let profiles = [profile("a", "Work", true), profile("b", "Home", false)];
        let menu = rows(&profiles, &Labels::default());
        let labels: Vec<_> = menu
            .iter()
            .map(|row| row.as_ref().map(|r| r.label.as_str()).unwrap_or("-"))
            .collect();
        assert_eq!(
            labels,
            [
                "Active: Work",
                "-",
                "\u{2713} Work",
                "   Home",
                "-",
                "Show Window",
                "-",
                "Close Git Account Manager"
            ]
        );
        assert!(!menu[0].as_ref().unwrap().enabled);
        assert_eq!(
            menu[3].as_ref().unwrap().command,
            Some(Command::Activate("b".into()))
        );
        assert_eq!(tooltip(&profiles), "Git Account Manager \u{2014} Work");
        assert_eq!(rows(&[], &Labels::default()).len(), 5);
    }
}
