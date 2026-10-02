// Small pieces several views share: buttons, chips, popup menus and the status dot. Sizes and
// colors follow the Tauri app's controls.css, composer.css and menus.css, through the theme's
// tokens only, so every view keeps the same neutral, low-contrast look on every theme.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, Bounds, Div, ElementId, FontWeight, IntoElement, MouseButton, Pixels,
    Point, SharedString, Stateful, Window, anchored, deferred, div, point, prelude::*, px,
};

use crate::assets::icon;
use crate::colors;
use crate::transcript::Status;

/// A quiet button: text on a hairline border that lifts on hover.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_frame(id).child(label.into())
}

/// [`button`] without its label, for content of the caller's own (an icon before the text).
pub fn button_frame(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .h(px(26.))
        .px(px(10.))
        .rounded(px(6.))
        .border_1()
        .border_color(colors::border1())
        .text_size(px(12.))
        .text_color(colors::text1())
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface3()).border_color(colors::border2()))
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

/// The round send button in a prompt box, zeron's: the text color filled in, its arrow cut out
/// in the background color.
pub fn send(id: impl Into<ElementId>, icon_name: &str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded_full()
        .bg(colors::text1())
        .cursor_pointer()
        .hover(|s| s.bg(colors::text1().opacity(0.85)))
        .child(icon(icon_name, 15., colors::bg()))
}

/// Send's neutral twin while a run is live: a round button with a filled square.
pub fn stop(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(30.))
        .rounded_full()
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface3())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.14)))
        .child(div().size(px(10.)).rounded(px(2.)).bg(colors::text1()))
}

/// A square icon button with no frame until hovered.
pub fn icon_button(id: impl Into<ElementId>, name: &str, size: f32) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(6.))
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface3()).text_color(colors::text1()))
        .child(icon(name, (size * 0.55).round(), colors::text3()))
}

/// A composer chip: a small framed pill that opens a picker. The caller adds its contents.
pub fn chip(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(8.))
        .max_w(px(260.))
        .rounded(px(6.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::surface1())
        .text_size(px(12.))
        .text_color(colors::text2())
        .cursor_pointer()
        .hover(|s| {
            s.bg(colors::surface3())
                .border_color(colors::border2())
                .text_color(colors::text1())
        })
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
    let (corner, at) = match open {
        Open::Down => (Anchor::TopLeft, at),
        Open::Up => (Anchor::BottomLeft, point(at.x - px(12.), at.y - px(16.))),
    };
    float(corner, at, window, close, content)
}

/// Like [`layer`], but sitting just above `chip` with its left edge on the chip's, so a picker
/// opens from its chip wherever on the chip the click landed.
pub fn above(
    chip: Bounds<Pixels>,
    window: &Window,
    close: impl Fn(&mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let at = point(chip.origin.x, chip.origin.y - px(6.));
    float(Anchor::BottomLeft, at, window, close, content)
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
        .p(px(4.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .rounded(px(9.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.))
        .text_color(colors::text2())
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
        menu_frame().w(w).max_w(w).child(content),
    )
}

/// One row of a context menu. A `danger` row turns red on hover.
pub fn menu_row(
    id: impl Into<ElementId>,
    icon_name: Option<&str>,
    label: impl Into<SharedString>,
    danger: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(26.))
        .px(px(8.))
        .rounded(px(6.))
        .cursor_pointer()
        .when(!danger, |d| {
            d.hover(|s| s.bg(colors::surface3()).text_color(colors::text1()))
        })
        .when(danger, |d| {
            d.hover(|s| {
                s.bg(colors::error().opacity(0.14))
                    .text_color(colors::error())
            })
        })
        .children(icon_name.map(|n| icon(n, 13., colors::text3())))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
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
        .px(px(8.))
        .py(px(5.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface3()))
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

/// One line of a compact picker: a mark or icon, the name, a short muted note pushed right, and
/// a slot at the end for a check or a chevron. `hi` is the row the mouse or the arrows are on;
/// it gets a quiet wash, never the accent.
pub fn pick_row(
    id: impl Into<ElementId>,
    lead: Option<AnyElement>,
    label: impl Into<SharedString>,
    note: Option<SharedString>,
    trail: Option<AnyElement>,
    hi: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(30.))
        .px(px(8.))
        .rounded(px(7.))
        .cursor_pointer()
        .when(hi, |d| d.bg(colors::ink(0.07)))
        .children(lead)
        .child(
            div()
                .flex_shrink_0()
                .max_w(px(180.))
                .truncate()
                .text_color(colors::text1())
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .justify_end()
                .text_size(px(11.5))
                .text_color(colors::text3())
                .children(note.map(|n| div().truncate().child(n))),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .justify_center()
                .w(px(14.))
                .children(trail),
        )
}

/// What a menu is about: small, dim and upper case.
pub fn menu_heading(label: impl Into<SharedString>) -> Div {
    let label: SharedString = label.into();
    div()
        .px(px(8.))
        .pt(px(3.))
        .pb(px(5.))
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(label.to_uppercase())
}

pub fn menu_rule() -> Div {
    div().h(px(1.)).mx(px(2.)).my(px(4.)).bg(colors::border1())
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

/// A segmented control's frame (defaults.css `.df-seg`). Its options are `segment`s.
pub fn segments() -> Div {
    div()
        .flex()
        .gap(px(3.))
        .p(px(3.))
        .rounded(px(9.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.04))
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
        .gap(px(7.))
        .h(px(28.))
        .px(px(12.))
        .rounded(px(6.))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg)
        .cursor_pointer()
        .when(on && !risky, |d| {
            d.bg(colors::surface3()).border_color(colors::border1())
        })
        .when(on && risky, |d| {
            d.bg(colors::error().opacity(0.14))
                .border_color(colors::error().opacity(0.4))
        })
        .when(!on, |d| {
            d.hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
        })
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
        .h(px(34.))
        .pl(px(12.))
        .pr(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.04))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text1())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.07)))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .child(icon("chevron-down", 12., colors::text3()))
}

/// A key's name in a small frame, like the Esc beside Settings' way back.
pub fn keycap(label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .border_1()
        .border_color(colors::border1())
        .text_size(px(10.))
        .text_color(colors::text3())
        .child(label.into())
}
