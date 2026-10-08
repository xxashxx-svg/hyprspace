// The pieces every Settings page is built from, after zeron's settings widgets: a page title,
// sections with a plain label over a filled block, and rows inside the block with the name and a
// line on what it does on the left and the control on the right, split by hairlines.

use gpui::{
    AnyElement, Div, ElementId, FontWeight, IntoElement, SharedString, Stateful, div, prelude::*,
    px,
};

use crate::assets::icon;
use crate::colors;

/// The page column: centered, at most this wide, with air on both sides.
const PAGE_WIDTH: f32 = 760.;
const PAGE_PAD: f32 = 40.;

pub(super) fn page(title: &str, desc: &str, body: impl IntoElement) -> Div {
    div().w_full().flex().justify_center().child(
        div()
            .w_full()
            .max_w(px(PAGE_WIDTH))
            .px(px(PAGE_PAD))
            .pt(px(34.))
            .pb(px(56.))
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(4.))
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(title.to_string()),
            )
            .child(
                div()
                    .mt(px(4.))
                    .px(px(4.))
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .child(desc.to_string()),
            )
            .child(div().mt(px(26.)).flex().flex_col().gap(px(28.)).child(body)),
    )
}

/// A plain label over its block. Sentence case, never an upper-case eyebrow.
pub(super) fn section(title: &str, block: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .px(px(4.))
                .text_size(px(13.))
                .text_color(colors::text3())
                .child(title.to_string()),
        )
        .child(block)
}

/// A filled, rounded block with hairlines between its rows.
pub(super) fn block(rows: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .bg(colors::ink(0.035))
        .children(rows.into_iter().enumerate().map(|(i, r)| {
            div()
                .mx(px(16.))
                .when(i > 0, |d| d.border_t_1().border_color(colors::border1()))
                .child(r)
        }))
}

/// [`section`] around a [`block`].
pub(super) fn group(title: &str, rows: Vec<AnyElement>) -> Div {
    section(title, block(rows))
}

/// One setting: its name and what it does, with the control on the right.
pub(super) fn row(
    name: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    control: impl IntoElement,
) -> AnyElement {
    let desc = desc.into();
    if desc.is_empty() {
        return div()
            .flex()
            .items_center()
            .gap(px(16.))
            .min_h(px(48.))
            .py(px(9.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(name.into()),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(control),
            )
            .into_any_element();
    }
    row_with(None, name, text(desc), control)
}

/// [`row`] with a mark before the name and a line of the caller's own under it.
pub(super) fn row_with(
    lead: Option<AnyElement>,
    name: impl Into<SharedString>,
    under: impl IntoElement,
    control: impl IntoElement,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .min_h(px(58.))
        .py(px(11.))
        .children(lead)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(name.into()),
                )
                .child(under),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(control),
        )
        .into_any_element()
}

/// The quiet line under a row's name.
pub(super) fn text(desc: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(colors::text3())
        .child(desc.into())
}

/// A small square button with one icon, for steppers.
pub(super) fn step(id: impl Into<ElementId>, glyph: &str, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(26.))
        .rounded(px(6.))
        .child(icon(
            glyph,
            13.,
            if enabled {
                colors::text2()
            } else {
                colors::text3()
            },
        ))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|s| s.bg(colors::ink(0.08)))
        })
        .when(!enabled, |d| d.opacity(0.5))
}

/// A key or a key chord as caps: "Ctrl+Shift+G" as three.
pub(super) fn keys(chord: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(chord.split('+').map(|k| {
            div()
                .flex_none()
                .min_w(px(22.))
                .h(px(22.))
                .px(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(0.04))
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text2())
                .child(k.to_string())
        }))
}
