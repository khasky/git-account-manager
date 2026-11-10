//! The stylesheet's recurring shapes: buttons, fields, switches, dialogs.

use std::borrow::Cow;
use std::time::Duration;

use gpui_kit::base::input::{Input, InputEditorStyle, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use super::theme::{hex, Palette, BLUE_500, BLUE_600, EMERALD_600, RED_500, RED_600};

// -- assets -------------------------------------------------------------------

pub struct Assets;

fn stroke(path: &str, width: f32) -> Cow<'static, [u8]> {
    Cow::Owned(
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"><path d="{path}"/></svg>"##
        )
        .into_bytes(),
    )
}

fn filled(view_box: &str, path: &str) -> Cow<'static, [u8]> {
    Cow::Owned(
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}" fill="#000"><path d="{path}"/></svg>"##
        )
        .into_bytes(),
    )
}

pub const SUN: &str = "sun.svg";
pub const MOON: &str = "moon.svg";
pub const MONITOR: &str = "monitor.svg";
pub const COPY: &str = "copy.svg";
pub const BACK: &str = "back.svg";
pub const CLOSE: &str = "close.svg";
pub const PLUS: &str = "plus.svg";
pub const CHECK: &str = "check.svg";
pub const GEAR: &str = "gear.svg";
pub const PEOPLE: &str = "people.svg";
pub const GITHUB: &str = "github.svg";
pub const GITLAB: &str = "gitlab.svg";
pub const AZURE: &str = "azure.svg";
pub const BITBUCKET: &str = "bitbucket.svg";
pub const CODEBERG: &str = "codeberg.svg";
pub const HUGGINGFACE: &str = "huggingface.svg";
pub const SPINNER: &str = "spinner.svg";
pub const CHEVRON: &str = "chevron.svg";
pub const BOX_CHECK: &str = "box-check.svg";

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Some(match path {
            SUN => stroke("M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z", 2.),
            MOON => stroke("M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z", 2.),
            MONITOR => stroke("M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z", 2.),
            COPY => stroke("M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z", 2.),
            BACK => stroke("M15 19l-7-7 7-7", 2.),
            CLOSE => stroke("M6 18L18 6M6 6l12 12", 2.),
            PLUS => stroke("M12 4v16m8-8H4", 2.),
            CHECK => stroke("M5 13l4 4L19 7", 2.),
            PEOPLE => stroke("M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z", 1.),
            CHEVRON => stroke("M6 9l6 6 6-6", 3.),
            GEAR => Cow::Borrowed(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"/><path d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"/></svg>"##),
            GITHUB => filled("-2 -2 20 20", "M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.650-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"),
            GITLAB => filled("80 85 220 200", "M282.83 170.73l-.27-.69-26.14-68.22a6.81 6.81 0 00-2.69-3.24 7 7 0 00-8 .43 7 7 0 00-2.32 3.52l-17.65 54h-71.47l-17.65-54a6.86 6.86 0 00-2.32-3.53 7 7 0 00-8-.43 6.87 6.87 0 00-2.690 3.24L97.44 170l-.26.69a48.54 48.54 0 0016.1 56.07l.09.07.24.17 39.82 29.82 19.7 14.91 12 9.06a8.07 8.07 0 009.76 0l12-9.06 19.7-14.91 40.06-30 .1-.08a48.56 48.56 0 0016.08-56.04z"),
            AZURE => filled("0 0 24 24", "M0 8.877L2.247 5.91l8.405-3.416V.022l7.37 5.393L2.966 8.338v8.225L0 15.707zm24-4.45v14.651l-5.753 4.9-9.303-3.057v3.056l-5.978-7.416 15.057 1.798V5.415z"),
            BITBUCKET => filled("0 0 24 24", "M.778 1.213a.768.768 0 00-.768.892l3.263 19.81c.084.5.515.868 1.022.873H19.95a.772.772 0 00.77-.646l3.27-20.03a.768.768 0 00-.768-.891zM14.52 15.53H9.522L8.17 8.466h7.561z"),
            CODEBERG => filled("0 0 24 24", "M11.955.49A12 12 0 0 0 0 12.49a12 12 0 0 0 1.832 6.373L11.838 5.928a.187.14 0 0 1 .324 0l10.006 12.935A12 12 0 0 0 24 12.49a12 12 0 0 0-12-12 12 12 0 0 0-.045 0zm.375 6.467l4.416 16.553a12 12 0 0 0 5.137-4.213z"),
            HUGGINGFACE => filled("0 0 24 24", include_str!("huggingface.path")),
            SPINNER => Cow::Borrowed(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"><circle opacity="0.25" cx="12" cy="12" r="10" stroke="#000" stroke-width="4"/><path opacity="0.9" fill="#000" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z"/></svg>"##),
            BOX_CHECK => Cow::Borrowed(br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M3.2 8.4 6.4 11.4 12.8 4.8" fill="none" stroke="#000" stroke-width="2.2"/></svg>"##),
            _ => return Ok(None),
        }))
    }

    fn list(&self, _: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

// -- type ---------------------------------------------------------------------

pub const UI_FONT: &str = ".SystemUIFont";

#[cfg(target_os = "windows")]
pub const MONO_FONT: &str = "Consolas";
#[cfg(target_os = "macos")]
pub const MONO_FONT: &str = "Menlo";
#[cfg(target_os = "linux")]
pub const MONO_FONT: &str = "Liberation Mono";

/// A Tailwind text size: font size and line height in pixels.
#[derive(Clone, Copy)]
pub struct Size(pub f32, pub f32);

pub const XS: Size = Size(12., 16.);
pub const SM: Size = Size(14., 20.);
pub const BASE: Size = Size(16., 24.);
pub const LG: Size = Size(18., 28.);
pub const XL: Size = Size(20., 28.);
pub const XXL: Size = Size(24., 32.);
/// `text-[11px]` under the document's 1.5 line height.
pub const X11: Size = Size(11., 16.5);
/// `text-[10px]` inside a `text-sm` row.
pub const X10: Size = Size(10., 14.29);

pub trait TextSize: Styled + Sized {
    fn ts(self, size: Size) -> Self {
        self.text_size(px(size.0)).line_height(px(size.1))
    }
}

impl<T: Styled> TextSize for T {}

pub fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// The running half of a button's label, sized like the text beside it.
pub fn spinner(size: f32, color: Hsla) -> impl IntoElement {
    icon(SPINNER, size, color).with_animation(
        ElementId::from(SharedString::from("spinner")),
        Animation::new(Duration::from_secs(1)).repeat(),
        |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
    )
}

fn shadow(layers: &[(f32, f32, f32, f32)]) -> Vec<BoxShadow> {
    layers
        .iter()
        .map(|&(y, blur, spread, alpha)| BoxShadow {
            color: hsla(0., 0., 0., alpha),
            offset: point(px(0.), px(y)),
            blur_radius: px(blur),
            spread_radius: px(spread),
            inset: false,
        })
        .collect()
}

pub fn shadow_lg() -> Vec<BoxShadow> {
    shadow(&[(10., 15., -3., 0.1), (4., 6., -4., 0.1)])
}

pub fn shadow_xl() -> Vec<BoxShadow> {
    shadow(&[(20., 25., -5., 0.1), (8., 10., -6., 0.1)])
}

pub fn shadow_2xl() -> Vec<BoxShadow> {
    shadow(&[(25., 50., -12., 0.25)])
}

fn shadow_sm() -> Vec<BoxShadow> {
    shadow(&[(1., 3., 0., 0.1), (1., 2., -1., 0.1)])
}

// -- buttons ------------------------------------------------------------------

/// A focusable control with the browser's keyboard focus ring.
pub fn control(id: impl Into<ElementId>, palette: &Palette) -> Stateful<Div> {
    let ring = palette.link;
    div()
        .id(id)
        .tab_index(0)
        .role(Role::Button)
        .cursor_pointer()
        .focus_visible(move |style| {
            style.shadow(vec![BoxShadow {
                color: ring,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(2.),
                inset: false,
            }])
        })
}

/// `btn-primary`.
pub fn btn_primary(id: impl Into<ElementId>, palette: &Palette, disabled: bool) -> Stateful<Div> {
    control(id, palette)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .bg(hex(BLUE_600))
        .px(px(16.))
        .py(px(8.))
        .ts(SM)
        .font_weight(FontWeight::MEDIUM)
        .text_color(white())
        .when(disabled, |b| b.opacity(0.5).cursor_default())
        .when(!disabled, |b| b.hover(|s| s.bg(hex(BLUE_500))))
}

/// `btn-subtle`.
pub fn btn_subtle(id: impl Into<ElementId>, palette: &Palette) -> Stateful<Div> {
    let hover = palette.hover;
    control(id, palette)
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .bg(palette.subtle)
        .px(px(16.))
        .py(px(8.))
        .ts(SM)
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.fg2)
        .hover(move |s| s.bg(hover))
}

/// `btn-raised-sm`.
pub fn btn_raised_sm(id: impl Into<ElementId>, palette: &Palette, disabled: bool) -> Stateful<Div> {
    let hover = palette.subtle;
    control(id, palette)
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(6.))
        .bg(palette.raised)
        .px(px(12.))
        .py(px(6.))
        .ts(XS)
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.fg2)
        .when(disabled, |b| b.opacity(0.5).cursor_default())
        .when(!disabled, |b| b.hover(move |s| s.bg(hover)))
}

/// `btn-icon`: a 20px glyph in a padded square.
pub fn btn_icon(
    id: impl Into<ElementId>,
    glyph: &'static str,
    label: impl Into<SharedString>,
    palette: &Palette,
) -> Stateful<Div> {
    let (hover_bg, hover_fg) = (palette.subtle, palette.fg2);
    control(id, palette)
        .aria_label(label.into())
        .group("btn-icon")
        .rounded(px(6.))
        .bg(palette.raised)
        .p(px(8.))
        .hover(move |s| s.bg(hover_bg))
        .child(
            icon(glyph, 20., palette.fg4).group_hover("btn-icon", move |s| s.text_color(hover_fg)),
        )
}

/// A small coloured action: `rounded-md px-3 py-1 text-[11px] font-medium`.
pub fn btn_small_blue(
    id: impl Into<ElementId>,
    palette: &Palette,
    disabled: bool,
) -> Stateful<Div> {
    control(id, palette)
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(6.))
        .bg(hex(BLUE_600))
        .px(px(12.))
        .py(px(4.))
        .ts(X11)
        .font_weight(FontWeight::MEDIUM)
        .text_color(white())
        .when(disabled, |b| b.opacity(0.5).cursor_default())
        .when(!disabled, |b| b.hover(|s| s.bg(hex(BLUE_500))))
}

/// A text-only control that turns red on hover, such as Remove.
pub fn btn_quiet_danger(
    id: impl Into<ElementId>,
    palette: &Palette,
    disabled: bool,
) -> Stateful<Div> {
    control(id, palette)
        .text_color(palette.fg4)
        .when(disabled, |b| b.opacity(0.5).cursor_default())
        .when(!disabled, |b| b.hover(|s| s.text_color(hex(RED_500))))
}

// -- switches -----------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum ToggleSize {
    Md,
}

/// `Toggle`: a pill switch, emerald when on.
pub fn toggle(
    id: impl Into<ElementId>,
    on: bool,
    palette: &Palette,
    _size: ToggleSize,
) -> Stateful<Div> {
    control(id, palette)
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .flex_none()
        .flex()
        .w(px(44.))
        .h(px(24.))
        .p(px(2.))
        .rounded_full()
        .bg(if on {
            hex(EMERALD_600)
        } else {
            palette.toggle_off
        })
        .child(
            div()
                .size(px(20.))
                .rounded_full()
                .bg(white())
                .shadow(shadow_sm())
                .when(on, |knob| knob.ml(px(20.))),
        )
}

/// A native 16px checkbox with `accent-emerald-600`.
pub fn checkbox(id: impl Into<ElementId>, checked: bool, palette: &Palette) -> Stateful<Div> {
    let accent = hex(EMERALD_600);
    control(id, palette)
        .role(Role::CheckBox)
        .aria_toggled(if checked {
            Toggled::True
        } else {
            Toggled::False
        })
        .flex_none()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .when(checked, |b| {
            b.bg(accent)
                .border_color(accent)
                .child(icon(BOX_CHECK, 14., white()))
        })
        .when(!checked, |b| {
            b.bg(palette.check_bg).border_color(palette.check_border)
        })
}

// -- fields -------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Field {
    /// `field`: `px-3 py-2 text-sm`.
    Md,
    /// `field-sm`: `px-2.5 py-1.5 text-sm`.
    Sm,
    /// The custom-address field: `px-2.5 py-1.5 text-xs`.
    Xs,
}

pub fn input_state(
    placeholder: impl Into<SharedString>,
    masked: bool,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let placeholder = placeholder.into();
    cx.new(|cx| {
        InputState::new(window, cx)
            .masked(masked)
            .placeholder(placeholder)
    })
}

/// A bordered text field around an input's state.
pub fn field(
    state: &Entity<InputState>,
    kind: Field,
    palette: &Palette,
    window: &Window,
    cx: &mut App,
) -> Div {
    let focused = state.read(cx).focus_handle(cx).is_focused(window);
    state.update(cx, |state, _| {
        state.set_editor_style(InputEditorStyle {
            foreground: palette.fg,
            muted_foreground: palette.fg5,
            background: palette.input,
            border: palette.border_s,
            selection: hex(BLUE_500).opacity(0.3),
            caret: palette.fg,
            ..Default::default()
        })
    });
    let (px_, py_, size) = match kind {
        Field::Md => (12., 8., SM),
        Field::Sm => (10., 6., SM),
        Field::Xs => (10., 6., XS),
    };
    div()
        .w_full()
        .rounded(px(6.))
        .border_1()
        .border_color(if focused {
            hex(BLUE_500)
        } else {
            palette.border_s
        })
        .bg(palette.input)
        .px(px(px_))
        .py(px(py_))
        .ts(size)
        .text_color(palette.fg)
        .child(Input::new(state))
}

pub fn value(state: &Entity<InputState>, cx: &App) -> String {
    state.read(cx).value().to_string()
}

// -- rich text ----------------------------------------------------------------

pub type Action = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// What a translated string's `<a>` does.
pub enum Link {
    None,
    /// An external page, styled as a plain link.
    Href(String),
    /// An action, styled as an underlined button.
    Action(Action),
}

/// Renders a translated string carrying `<code>`, `<b>` and one `<a>`.
pub fn rich(
    id: impl Into<ElementId>,
    text: impl AsRef<str>,
    color: Hsla,
    code_color: Hsla,
    palette: &Palette,
    link: Link,
) -> AnyElement {
    let base = font(UI_FONT);
    let mut plain = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut link_range = None;
    let (mut code, mut bold, mut anchor) = (false, false, false);
    let mut rest = text.as_ref();
    let push = |segment: &str,
                plain: &mut String,
                runs: &mut Vec<TextRun>,
                code: bool,
                bold: bool,
                anchor: bool| {
        if segment.is_empty() {
            return;
        }
        let mut run_font = if code { font(MONO_FONT) } else { base.clone() };
        if bold {
            run_font.weight = FontWeight::BOLD;
        } else if anchor && !matches!(link, Link::Href(_)) {
            run_font.weight = FontWeight::MEDIUM;
        }
        plain.push_str(segment);
        runs.push(TextRun {
            len: segment.len(),
            font: run_font,
            color: if anchor {
                palette.link
            } else if code {
                code_color
            } else {
                color
            },
            background_color: None,
            underline: (anchor && !matches!(link, Link::Href(_))).then(|| UnderlineStyle {
                thickness: px(1.),
                color: Some(palette.link),
                wavy: false,
            }),
            strikethrough: None,
        });
    };
    while !rest.is_empty() {
        let next = ["<code>", "</code>", "<b>", "</b>", "<a>", "</a>"]
            .iter()
            .filter_map(|tag| rest.find(tag).map(|at| (at, *tag)))
            .min_by_key(|(at, _)| *at);
        let Some((at, tag)) = next else {
            push(rest, &mut plain, &mut runs, code, bold, anchor);
            break;
        };
        push(&rest[..at], &mut plain, &mut runs, code, bold, anchor);
        match tag {
            "<code>" => code = true,
            "</code>" => code = false,
            "<b>" => bold = true,
            "</b>" => bold = false,
            "<a>" => {
                anchor = true;
                link_range = Some(plain.len()..plain.len());
            }
            _ => {
                anchor = false;
                if let Some(range) = link_range.as_mut() {
                    range.end = plain.len();
                }
            }
        }
        rest = &rest[at + tag.len()..];
    }
    let styled = StyledText::new(plain).with_runs(runs);
    match (link, link_range) {
        (Link::None, _) | (_, None) => styled.into_any_element(),
        (Link::Href(url), Some(range)) => InteractiveText::new(id, styled)
            .on_click(vec![range], move |_, _, cx| cx.open_url(&url))
            .into_any_element(),
        (Link::Action(action), Some(range)) => InteractiveText::new(id, styled)
            .on_click(vec![range], move |_, window, cx| action(window, cx))
            .into_any_element(),
    }
}

// -- tips -----------------------------------------------------------------------

struct Tip {
    text: SharedString,
    palette: Palette,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        div()
            .w(px(256.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .bg(p.dialog)
            .px(px(10.))
            .py(px(8.))
            .text_size(px(11.))
            .line_height(px(17.875))
            .font_weight(FontWeight::NORMAL)
            .text_color(p.fg3)
            .shadow(shadow_lg())
            .child(self.text.clone())
    }
}

/// `InfoTip`: an `i` that explains the control next to it.
pub fn info_tip(id: impl Into<ElementId>, text: &str, palette: &Palette) -> Stateful<Div> {
    let text: SharedString = text.to_string().into();
    let p = *palette;
    let (hover_border, hover_fg) = (palette.border, palette.fg3);
    control(id, palette)
        .aria_label(text.clone())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded_full()
        .border_1()
        .border_color(palette.border_s)
        .text_size(px(10.))
        .line_height(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.fg5)
        .hover(move |s| s.border_color(hover_border).text_color(hover_fg))
        .child("i")
        .tooltip(move |_, cx| {
            let text = text.clone();
            cx.new(|_| Tip { text, palette: p }).into()
        })
}

/// What the webview's `title` attribute showed: a small native-looking tip.
pub fn tip(text: &str, palette: &Palette) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text: SharedString = text.to_string().into();
    let p = *palette;
    move |_, cx| {
        let text = text.clone();
        cx.new(|_| TitleTip { text, palette: p }).into()
    }
}

struct TitleTip {
    text: SharedString,
    palette: Palette,
}

impl Render for TitleTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        div()
            .rounded(px(4.))
            .border_1()
            .border_color(p.border)
            .bg(p.dialog)
            .px(px(8.))
            .py(px(4.))
            .font_family(UI_FONT)
            .ts(XS)
            .text_color(p.fg2)
            .shadow(shadow_sm())
            .child(self.text.clone())
    }
}

// -- scrolling ------------------------------------------------------------------

/// A scrolling pane with the stylesheet's 6px scrollbar thumb. `content` is
/// the pane itself, so its children are the items the handle can locate.
pub fn scroll_area(
    id: &'static str,
    handle: &ScrollHandle,
    palette: &Palette,
    content: Div,
) -> Div {
    let viewport = handle.bounds().size.height;
    let max = handle.max_offset().y;
    let thumb = (max > px(0.) && viewport > px(0.)).then(|| {
        let height = (viewport * (viewport / (viewport + max))).max(px(24.));
        let top = (-handle.offset().y).clamp(px(0.), max) / max * (viewport - height);
        div()
            .absolute()
            .right_0()
            .top(top)
            .w(px(6.))
            .h(height)
            .rounded(px(3.))
            .bg(palette.scrollbar)
    });
    div()
        .flex_1()
        .min_h_0()
        .relative()
        .child(
            content
                .id(id)
                .size_full()
                .overflow_y_scroll()
                .track_scroll(handle),
        )
        .children(thumb)
}

// -- select ---------------------------------------------------------------------

pub struct SelectChanged(pub usize);

/// A native `<select>`: the current option and a chevron, with a list below.
pub struct Select {
    options: Vec<SharedString>,
    pub selected: usize,
    open: bool,
    small: bool,
    label: SharedString,
    pub disabled: bool,
}

impl EventEmitter<SelectChanged> for Select {}

impl Select {
    pub fn new(
        options: Vec<SharedString>,
        selected: usize,
        small: bool,
        label: SharedString,
    ) -> Self {
        Self {
            options,
            selected,
            open: false,
            small,
            label,
            disabled: false,
        }
    }
}

impl Render for Select {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = super::palette(window, cx);
        let size = if self.small { XS } else { SM };
        let (pl, pr, py) = if self.small {
            (12., 6., 4.)
        } else {
            (16., 8., 6.)
        };
        let current = self.options.get(self.selected).cloned().unwrap_or_default();
        // Sized to the widest option, as a native select is.
        let sizer = self.options.iter().map(|option| {
            div()
                .h(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(option.clone())
        });
        let list = self.open.then(|| {
            deferred(
                anchored().snap_to_window().child(
                    div()
                        .id("select-list")
                        .occlude()
                        .mt(px(2.))
                        .min_w(px(80.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.dialog)
                        .py(px(4.))
                        .shadow(shadow_lg())
                        .ts(size)
                        .text_color(p.fg)
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open = false;
                            cx.notify();
                        }))
                        .children(self.options.iter().enumerate().map(|(index, option)| {
                            let selected = index == self.selected;
                            let hover = p.raised;
                            div()
                                .id(index)
                                .role(Role::ListBoxOption)
                                .px(px(12.))
                                .py(px(4.))
                                .cursor_pointer()
                                .whitespace_nowrap()
                                .when(selected, |row| {
                                    row.bg(p.selected_bg).text_color(p.selected_fg)
                                })
                                .when(!selected, |row| row.hover(move |s| s.bg(hover)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open = false;
                                    if this.selected != index {
                                        this.selected = index;
                                        cx.emit(SelectChanged(index));
                                    }
                                    cx.notify();
                                }))
                                .child(option.clone())
                        })),
                ),
            )
            .with_priority(1)
        });
        let disabled = self.disabled;
        div()
            .flex_none()
            .flex()
            .flex_col()
            .child(
                control("select", &p)
                    .role(Role::ComboBox)
                    .aria_label(self.label.clone())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if self.open { hex(BLUE_500) } else { p.border_s })
                    .bg(p.input)
                    .pl(px(pl))
                    .pr(px(pr))
                    .py(px(py))
                    .ts(size)
                    .text_color(p.fg)
                    .when(disabled, |b| b.opacity(0.5).cursor_default())
                    .when(!disabled, |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            this.open = !this.open;
                            cx.notify();
                        }))
                    })
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(div().whitespace_nowrap().child(current))
                            .children(sizer),
                    )
                    .child(icon(CHEVRON, if self.small { 10. } else { 12. }, p.fg)),
            )
            .children(list)
    }
}

// -- dialogs ----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Variant {
    Danger,
    Default,
    Cancel,
}

/// `ConfirmDialog`'s full-width action button.
pub fn dialog_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    palette: &Palette,
) -> Stateful<Div> {
    let (bg, fg, hover) = match variant {
        Variant::Danger => (hex(RED_600), white(), hex(RED_500)),
        Variant::Cancel => (palette.subtle, palette.fg3, palette.hover),
        Variant::Default => (hex(BLUE_600), white(), hex(BLUE_500)),
    };
    control(id, palette)
        .w_full()
        .flex()
        .justify_center()
        .rounded(px(6.))
        .px(px(16.))
        .py(px(8.))
        .ts(SM)
        .font_weight(FontWeight::MEDIUM)
        .bg(bg)
        .text_color(fg)
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// The backdrop every modal sits on, catching the pointer.
pub fn backdrop(color: Hsla) -> Stateful<Div> {
    div()
        .id("backdrop")
        .absolute()
        .inset_0()
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(color)
}

/// `ConfirmDialog`: title, body and a column of actions.
pub fn confirm_dialog(
    title: impl Into<SharedString>,
    body: impl IntoElement,
    actions: Vec<Stateful<Div>>,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
    palette: &Palette,
) -> Stateful<Div> {
    backdrop(palette.overlay)
        .on_click(move |_, window, cx| on_cancel(window, cx))
        .child(
            div()
                .id("dialog")
                .role(Role::Dialog)
                .occlude()
                .on_click(|_, _, cx| cx.stop_propagation())
                .mx(px(16.))
                .w_full()
                .max_w(px(448.))
                .rounded(px(8.))
                .border_1()
                .border_color(palette.border)
                .bg(palette.dialog)
                .shadow(shadow_2xl())
                .child(
                    div()
                        .border_b_1()
                        .border_color(palette.border)
                        .px(px(20.))
                        .py(px(16.))
                        .ts(BASE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.fg)
                        .child(title.into()),
                )
                .child(div().px(px(20.)).py(px(16.)).child(body))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .border_t_1()
                        .border_color(palette.border)
                        .px(px(20.))
                        .py(px(16.))
                        .children(actions),
                ),
        )
}

/// The file name at the end of a path.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    #[std::prelude::rust_2021::test]
    fn file_names_split_on_either_separator() {
        assert_eq!(super::file_name("C:\\Users\\a\\.ssh\\id_x"), "id_x");
        assert_eq!(super::file_name("/home/a/.ssh/id_y"), "id_y");
        assert_eq!(super::file_name("id_z"), "id_z");
    }
}
