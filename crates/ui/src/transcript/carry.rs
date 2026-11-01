// What a side chat found, brought back: a chip over the reply box until the next message, which
// takes the side chat's notes along ahead of what was typed.

use gpui::{AnyElement, ClickEvent, Context, MouseButton, div, prelude::*};

use super::model::Item;
use super::{TranscriptView, tool};
use crate::assets::icon;
use crate::zoom::px;
use crate::{colors, widgets};

/// The most of a side chat that goes along, from its end, so a long one stays a note.
const LIMIT: usize = 20_000;
const HEAD: &str = "Notes from a side chat about this thread, for this message:\n";
const CUT: &str = "\n---\n\n";

/// A sent message with side chat notes ahead of it, as (had notes, what was typed). The notes
/// are the agent's own words and may hold a rule of their own, so the last one cuts.
pub(super) fn split(text: &str) -> (bool, &str) {
    match text
        .strip_prefix(HEAD)
        .and_then(|rest| rest.rsplit_once(CUT))
    {
        Some((_, said)) => (true, said),
        None => (false, text),
    }
}

impl TranscriptView {
    /// This conversation as notes for the thread it was forked from.
    pub fn notes(&self) -> String {
        let mut out = String::new();
        for item in &self.model.items {
            match item {
                Item::User { text, .. } => out.push_str(&format!("\nAsked: {}\n", text.trim())),
                Item::Text { source, .. } if !source.trim().is_empty() => {
                    out.push_str(&format!("\n{}\n", source.trim()))
                }
                Item::Tool { tool: t, .. } => out.push_str(&format!("- {}\n", tool::label(t))),
                _ => {}
            }
        }
        let cut = out.len().saturating_sub(LIMIT);
        let start = (cut..=out.len())
            .find(|&i| out.is_char_boundary(i))
            .unwrap_or(out.len());
        format!("{HEAD}{}", &out[start..])
    }

    /// `notes` from the side chat `title`, sent with the next message.
    pub fn carry(&mut self, title: String, notes: String, cx: &mut Context<Self>) {
        self.carry = Some((title, notes));
        cx.notify();
    }

    /// `text` with the side chat's notes ahead of it, once.
    pub(super) fn with_carry(&mut self, text: String) -> String {
        match self.carry.take() {
            Some((_, notes)) => format!("{notes}{CUT}{text}"),
            None => text,
        }
    }
}

pub(super) fn chip(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    let (title, _) = v.carry.as_ref()?;
    Some(
        div()
            .mx(px(12.))
            .mb(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(7.))
            .rounded(px(10.))
            .bg(colors::ink(0.035))
            .text_size(px(12.5))
            .child(icon("message-square", 13., colors::text3()))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(colors::text2())
                    .child(title.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(colors::text3())
                    .child("goes with your next message"),
            )
            .child(
                widgets::icon_button("carry-drop", "x", widgets::SMALL)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .tooltip(widgets::tip("Leave it out"))
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
                        v.carry = None;
                        cx.notify();
                    })),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_with_notes_shows_only_what_was_typed() {
        let sent = format!("{HEAD}\nAsked: why?\n\nA rule\n---\n\nmore{CUT}What now?");
        assert_eq!(split(&sent), (true, "What now?"));
        assert_eq!(split("just this"), (false, "just this"));
    }
}
