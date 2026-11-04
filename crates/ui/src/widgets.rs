// Small pieces several views share: buttons, chips, popup menus and the status dot. Sizes and
// colors follow the Tauri app's controls.css, composer.css and menus.css, through the theme's
// tokens only, so every view keeps the same neutral, low-contrast look on every theme.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Anchor, Animation, AnimationExt, AnyElement, App, Bounds, BoxShadow, Div, ElementId,
    FontWeight, IntoElement, MouseButton, Pixels, Point, SharedString, Stateful, Window, anchored,
    deferred, div, point, prelude::*,
};

use crate::assets::icon;
use crate::colors;
use crate::transcript::Status;
use crate::zoom::px;

/// A quiet button: text on a faint fill that deepens on hover, like the segmented controls.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_frame(id).child(label.into())
}

/// [`button`] without its label, for content of the caller's own (an icon before the text).
pub fn button_frame(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(28.))
        .px(px(12.))
        .rounded(px(8.))
        .bg(colors::ink(0.06))
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text1())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.1)))
}

/// The one button that moves things forward, in the theme's accent.
pub fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    primary_frame(id).child(label.into())
}

/// [`primary`] without its label, like [`button_frame`].
pub fn primary_frame(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .h(px(26.))
        .px(px(10.))
        .rounded(px(6.))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .bg(colors::accent())
        .text_color(colors::on_accent())
        .cursor_pointer()
        .hover(|s| s.bg(colors::accent_hover()))
}

/// The round send button in a prompt box: the accent once there is something to send, muted
/// until then.
pub fn send(id: impl Into<ElementId>, icon_name: &str, ready: bool) -> Stateful<Div> {
    let b = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded_full()
        .cursor_pointer();
    if ready {
        b.bg(colors::accent())
            .hover(|s| s.bg(colors::accent_hover()))
            .child(icon(icon_name, 15., colors::on_accent()))
    } else {
        b.bg(colors::ink(0.1))
            .hover(|s| s.bg(colors::ink(0.16)))
            .child(icon(icon_name, 15., colors::text3()))
    }
}

pub const BOX_RADIUS: f32 = 20.;

pub fn composer_box(focused: bool) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(BOX_RADIUS))
        .border_1()
        .border_color(colors::ink(if focused { 0.13 } else { 0.05 }))
        .bg(colors::surface2())
        .shadow(raised(focused))
}

/// A box standing off the page: a deep shadow under it and, on a dark side, light along its top
/// edge.
fn raised(focused: bool) -> Vec<BoxShadow> {
    let dark = colors::theme().dark;
    let mut s = colors::shadow();
    s.push(BoxShadow {
        color: colors::ink(match (dark, focused) {
            (false, _) => 0.,
            (true, false) => 0.07,
            (true, true) => 0.1,
        }),
        offset: point(px(0.), px(1.)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: true,
    });
    s
}

/// Send's neutral twin while a run is live: a round button with a filled square.
pub fn stop(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(32.))
        .rounded_full()
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface3())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.14)))
        .child(div().size(px(10.)).rounded(px(2.)).bg(colors::text1()))
}

pub const SMALL: f32 = 22.;
pub const LARGE: f32 = 28.;

/// A square icon button with no frame until hovered.
pub fn icon_button(id: impl Into<ElementId>, name: &str, size: f32) -> Stateful<Div> {
    icon_frame(id, size).child(
        icon(name, (size * 0.5).round(), colors::text3())
            .group_hover("icon-button", |s| s.text_color(colors::text1())),
    )
}

pub fn icon_frame(id: impl Into<ElementId>, size: f32) -> Stateful<Div> {
    bare_icon_frame(id, size).hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
}

/// `icon_frame` without its hover, for a button that hovers its own way.
pub fn bare_icon_frame(id: impl Into<ElementId>, size: f32) -> Stateful<Div> {
    div()
        .id(id)
        .group("icon-button")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(6.))
        .text_color(colors::text3())
        .cursor_pointer()
}

/// The chevron at the end of a chip.
pub fn caret() -> AnyElement {
    icon("chevron-down", 12., colors::text3()).into_any_element()
}

/// Where a popup opens: below the click (menus) or above it (pickers under a prompt box, which
/// must never cover the text being typed).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Open {
    Down,
    Up,
}

/// Floats `content` at `at` (window coordinates, usually where the click was). A clear layer
/// over the whole window catches the next click outside it and calls `close`, so that click
/// does not also land on whatever is underneath. `content` brings its own frame.
pub fn layer(
    at: Point<Pixels>,
    open: Open,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let (corner, at, from) = match open {
        Open::Down => (Anchor::TopLeft, at, -4.),
        Open::Up => (
            Anchor::BottomLeft,
            point(at.x - px(12.), at.y - px(16.)),
            4.,
        ),
    };
    // fades in while travelling a few pixels away from where it opened
    let content = crate::slide::ease_in(
        div().relative().child(content),
        "popup-in",
        130,
        move |d, t| d.opacity(t).top(px(from * (1. - t))),
    );
    float(corner, at, window, close, content)
}

/// Like [`layer`], but by a picker's chip, wherever on the chip the click landed: under it when
/// `below`, else over it, lined up with the chip's edge nearer the window's middle, so it stays
/// over the box it came from.
pub fn by_chip(
    chip: Bounds<Pixels>,
    below: bool,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let left = from_left(chip, window);
    let x = if left { chip.left() } else { chip.right() };
    let (corner, at) = match (below, left) {
        (true, true) => (Anchor::TopLeft, point(x, chip.bottom() + px(6.))),
        (true, false) => (Anchor::TopRight, point(x, chip.bottom() + px(6.))),
        (false, true) => (Anchor::BottomLeft, point(x, chip.top() - px(6.))),
        (false, false) => (Anchor::BottomRight, point(x, chip.top() - px(6.))),
    };
    float(corner, at, window, close, content)
}

pub fn from_left(chip: Bounds<Pixels>, window: &Window) -> bool {
    chip.center().x < window.viewport_size().width / 2.
}

fn float(
    corner: Anchor,
    at: Point<Pixels>,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let close = Rc::new(close);
    let (a, b) = (close.clone(), close);
    let size = window.viewport_size();
    deferred(
        anchored().position(point(px(0.), px(0.))).child(
            div()
                .id("popup-layer")
                .w(size.width)
                .h(size.height)
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, w, cx| a(w, cx))
                .on_mouse_down(MouseButton::Right, move |_, w, cx| b(w, cx))
                .child(
                    anchored()
                        .anchor(corner)
                        .position(at)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("popup")
                                .occlude()
                                // clicks inside the popup must not reach the layer behind it
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                                .child(content),
                        ),
                ),
        ),
    )
    .with_priority(1)
    .into_any_element()
}

/// The context-menu frame (menus.css).
fn menu_frame() -> Stateful<Div> {
    div()
        .id("menu")
        .min_w(px(176.))
        .max_w(px(400.))
        .max_h(px(520.))
        .overflow_y_scroll()
        .p(px(5.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.5))
        .text_color(colors::text2())
}

pub fn popup_with_note(
    at: Point<Pixels>,
    open: Open,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
    note: &'static str,
) -> AnyElement {
    let note = div()
        .mt(px(4.))
        .mx(px(-5.))
        .mb(px(-5.))
        .px(px(12.))
        .py(px(8.))
        .border_t_1()
        .border_color(colors::border1())
        .text_size(px(11.))
        .text_color(colors::text3())
        .child(note);
    layer(
        at,
        open,
        window,
        close,
        menu_frame().child(content).child(note),
    )
}

/// A menu in the context-menu frame, floated by `layer`.
pub fn popup(
    at: Point<Pixels>,
    open: Open,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    layer(at, open, window, close, menu_frame().child(content))
}

/// A `select`'s menu: dropped just under the field and exactly as wide, like a native select.
pub fn dropdown(
    field: Bounds<Pixels>,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let w = field.size.width;
    layer(
        field.bottom_left() + point(px(0.), px(4.)),
        Open::Down,
        window,
        close,
        menu_frame().w(w).min_w(w).max_w(w).child(content),
    )
}

/// A picker row: a name, a note under it, and a check on the current one.
pub fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    note: Option<SharedString>,
    checked: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.06)))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(colors::text1())
                        .when(checked, |d| d.font_weight(FontWeight::SEMIBOLD))
                        .child(label.into()),
                )
                .when_some(note, |d, n| {
                    d.child(
                        div()
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .truncate()
                            .child(n),
                    )
                }),
        )
        .when(checked, |d| d.child(icon("check", 13., colors::text1())))
}

pub fn menu_rule() -> Div {
    div().h(px(1.)).mx(px(2.)).my(px(4.)).bg(colors::border1())
}

pub fn toast(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .occlude()
        .flex()
        .items_center()
        .gap(px(10.))
        .max_w(px(440.))
        .h(px(38.))
        .pl(px(14.))
        .pr(px(6.))
        .rounded(px(12.))
        .bg(colors::surface3())
        .border_1()
        .border_color(colors::border1())
        .shadow(colors::shadow())
        .text_size(px(13.))
}

pub fn status_dot(status: Status) -> Div {
    let d = div().flex_none().size(px(7.)).rounded_full();
    match status {
        Status::Idle => d.border_1().border_color(colors::text3()),
        Status::Working => d.bg(colors::busy()),
        Status::Waiting => d.bg(colors::waiting()),
        Status::Done => d.bg(colors::ok()),
        Status::Failed => d.bg(colors::error()),
    }
}

/// A segmented control's frame. Its options are `segment`s, flush against each other.
pub fn segments() -> Div {
    div()
        .flex()
        .flex_none()
        .p(px(2.))
        .rounded(px(8.))
        .bg(colors::ink(0.045))
}

/// An on/off switch. The caller handles the click.
pub fn switch(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    switch_turned(id, on, None)
}

pub fn switch_turned(id: impl Into<ElementId>, on: bool, turn: Option<usize>) -> Stateful<Div> {
    let side = move |t: f32| if on { t } else { 1. - t };
    let part = move |el: Div, name: &'static str, f: fn(Div, f32) -> Div| -> AnyElement {
        match turn {
            Some(n) => el
                .with_animation(
                    (name, n),
                    Animation::new(Duration::from_millis(240)).with_easing(crate::slide::ease_out),
                    move |d, t| f(d, side(t)),
                )
                .into_any_element(),
            None => f(el, side(1.)).into_any_element(),
        }
    };
    let knob = || div().absolute().top(px(2.)).size(px(14.)).rounded_full();
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(32.))
        .h(px(18.))
        .rounded_full()
        .cursor_pointer()
        .bg(colors::ink(0.14))
        .hover(move |s| {
            if on {
                s.opacity(0.9)
            } else {
                s.bg(colors::ink(0.2))
            }
        })
        .child(part(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .bg(colors::text1()),
            "switch-fill",
            |d, s| d.opacity(s),
        ))
        .child(part(knob().bg(colors::text2()), "switch-off", |d, s| {
            d.left(px(2. + 14. * s)).opacity(1. - s)
        }))
        .child(part(knob().bg(colors::bg()), "switch-on", |d, s| {
            d.left(px(2. + 14. * s)).opacity(s)
        }))
}

/// One option of a segmented control. The picked one sits on a raised neutral surface, never the
/// accent. A `risky` option turns red only while it is picked, so the warning shows when it
/// applies and not before.
pub fn segment(
    id: impl Into<ElementId>,
    icon_name: Option<&str>,
    label: impl Into<SharedString>,
    on: bool,
    risky: bool,
) -> Stateful<Div> {
    let (fg, glyph) = match (on, risky) {
        (true, true) => (colors::error(), colors::error()),
        (true, false) => (colors::text1(), colors::text2()),
        _ => (colors::text2(), colors::text3()),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(11.))
        .rounded(px(6.))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg)
        .cursor_pointer()
        .when(on && !risky, |d| {
            d.bg(colors::ink(0.1)).border_color(colors::ink(0.06))
        })
        .when(on && risky, |d| {
            d.bg(colors::error().opacity(0.14))
                .border_color(colors::error().opacity(0.4))
        })
        .when(!on, |d| d.hover(|s| s.text_color(colors::text1())))
        .children(icon_name.map(|n| icon(n, 14., glyph)))
        .child(label.into())
}

/// A dropdown's face (defaults.css `.set-select`): the current value and a chevron. The caller
/// opens the menu on click.
pub fn select(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(30.))
        .pl(px(11.))
        .pr(px(9.))
        .rounded(px(8.))
        .bg(colors::ink(0.045))
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text1())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.08)))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .child(icon("chevron-down", 12., colors::text3()))
}

/// A key's name in a small frame, like the Esc beside Settings' way back.
pub fn keycap(label: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(18.))
        .h(px(18.))
        .px(px(5.))
        .rounded(px(4.))
        .bg(colors::ink(0.06))
        .border_1()
        .border_b_2()
        .border_color(colors::ink(0.08))
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(label.into())
}

/// A short label beside a button while the pointer rests on it, in the app's own font.
pub fn tip(text: &'static str) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    move |_, cx| cx.new(|_| Tip(text.into())).into()
}

/// [`tip`] for words made at run time.
pub fn tip_text(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

struct Tip(SharedString);

impl gpui::Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .font_family(crate::settings::ui_font())
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface3())
            .shadow(colors::shadow())
            .text_size(px(12.))
            .text_color(colors::text1())
            .child(self.0.clone())
    }
}

#[derive(IntoElement)]
pub struct CopyButton {
    id: SharedString,
    text: SharedString,
}

impl CopyButton {
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl gpui::RenderOnce for CopyButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let copied = window.use_keyed_state(
            SharedString::from(format!("{}-copied", self.id)),
            cx,
            |_, _| None::<std::time::Instant>,
        );
        let fresh = copied
            .read(cx)
            .is_some_and(|at| at.elapsed() < std::time::Duration::from_millis(1500));
        if fresh {
            window.request_animation_frame();
        }
        let text = self.text;
        div()
            .id(ElementId::Name(self.id))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.08)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| {
                crate::clipboard::write(cx, gpui::ClipboardItem::new_string(text.to_string()));
                copied.update(cx, |c, _| *c = Some(std::time::Instant::now()));
            })
            .tooltip(tip(if fresh { "Copied" } else { "Copy" }))
            .child(if fresh {
                icon("check", 13., colors::ok())
            } else {
                icon("copy", 13., colors::text3())
            })
    }
}
