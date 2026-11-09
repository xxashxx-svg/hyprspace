// The inside of a prompt box, shared by the composer and a thread's reply box so the two can't
// drift: the prompt, then attach, model, effort and permission on the left of the row under it,
// and the caller's own ends (Structured, send, stop) on the right.

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
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(8.))
        .pb(px(8.))
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(4.))
                .child(attach)
                .child(model)
                .children(effort)
                .child(permission),
        )
        .children(ends)
}

/// A pill for one of the prompt's settings, brightening on hover.
pub fn chip(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(9.))
        .min_w_0()
        .rounded(px(8.))
        .text_size(px(14.))
        .text_color(colors::text2())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.08)))
}

pub fn attach(id: impl Into<ElementId>) -> Stateful<Div> {
    widgets::bare_icon_frame(id, 28.)
        .rounded(px(8.))
        .hover(|s| s.bg(colors::ink(0.08)))
        .child(
            icon("paperclip", 14., colors::text3())
                .group_hover("icon-button", |s| s.text_color(colors::text1())),
        )
        .tooltip(widgets::tip("Attach images"))
}

pub fn model(id: impl Into<ElementId>, agent: Agent, label: String) -> Stateful<Div> {
    chip(id)
        .child(mark(agent, 13., colors::brand(agent).0))
        .child(
            div()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(label),
        )
}

pub fn effort(id: impl Into<ElementId>, spec: &Spec) -> Stateful<Div> {
    chip(id).child(effort_chip_label(spec))
}

pub fn permission(id: impl Into<ElementId>, p: Permission) -> Stateful<Div> {
    chip(id)
        .gap(px(5.))
        .child(icon(pickers::permission_icon(p), 12., colors::text2()))
        .child(pickers::permission_label(p))
}
