// The inside of a prompt box, shared by the composer and a thread's reply box so the two can't
// drift: the prompt, then attach on the left of the row under it, and model, effort, permission
// and the caller's own ends (Structured, send, stop) on the right, after T3 Code's.

use gpui::{
    AnyElement, Div, ElementId, Entity, FontWeight, IntoElement, Stateful, div, prelude::*,
};
use hyprspace_proto::{Agent, Permission};

use super::model_menu::{Spec, effort_chip_label};
use super::pickers;
use crate::assets::{icon, mark};
use crate::input::TextInput;
use crate::zoom::px;
use crate::{colors, widgets};

/// The prompt, across the box.
pub fn field(input: &Entity<TextInput>) -> Div {
    div()
        .px(px(16.))
        .pt(px(16.))
        .pb(px(8.))
        .text_size(px(14.))
        .line_height(px(23.))
        .child(input.clone())
}

pub fn bar(
    attach: impl IntoElement,
    model: impl IntoElement,
    effort: Option<impl IntoElement>,
    permission: impl IntoElement,
    ends: impl IntoIterator<Item = AnyElement>,
) -> Div {
    let alone = effort.is_none();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .pb(px(8.))
        .child(attach)
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_end()
                .gap(px(2.))
                .child(
                    div()
                        .id("model-pill")
                        .flex()
                        .min_w_0()
                        .items_center()
                        .rounded(px(8.))
                        .hover(|s| s.bg(colors::ink(0.08)))
                        .when(alone, |d| d.pr(px(5.)))
                        .child(model)
                        .children(effort),
                )
                .child(permission),
        )
        .children(ends)
}

/// A control for one of the prompt's settings, filling on hover.
pub fn chip(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(8.))
        .min_w_0()
        .rounded(px(8.))
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text1().opacity(0.8))
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
}

pub fn attach(id: impl Into<ElementId>) -> Stateful<Div> {
    widgets::bare_icon_frame(id, 28.)
        .rounded(px(8.))
        .hover(|s| s.bg(colors::ink(0.08)))
        .child(
            icon("paperclip", 16., colors::text2())
                .group_hover("icon-button", |s| s.text_color(colors::text1())),
        )
        .tooltip(widgets::tip("Attach images"))
}

/// One half of the model pill, which fills as a whole.
fn half(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .min_w_0()
        .text_size(px(14.))
        .cursor_pointer()
}

pub fn model(id: impl Into<ElementId>, agent: Agent, label: String) -> Stateful<Div> {
    half(id)
        .pl(px(8.))
        .pr(px(3.))
        .child(mark(agent, 16., colors::brand(agent).0))
        .child(
            div()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1().opacity(0.8))
                .child(label),
        )
}

pub fn effort(id: impl Into<ElementId>, spec: &Spec) -> Stateful<Div> {
    half(id)
        .pl(px(3.))
        .pr(px(8.))
        .text_color(colors::text2())
        .child(effort_chip_label(spec))
}

pub fn permission(id: impl Into<ElementId>, p: Permission) -> Stateful<Div> {
    chip(id)
        .font_weight(FontWeight::NORMAL)
        .text_color(colors::text2())
        .child(icon(pickers::permission_icon(p), 16., colors::text2()))
        .child(pickers::permission_label(p))
}
