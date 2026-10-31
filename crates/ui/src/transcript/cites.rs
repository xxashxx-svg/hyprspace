use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable, IntoElement, MouseButton, Subscription,
    Window, div, prelude::*,
};
use hyprspace_proto::Cite;

use super::TranscriptView;
use crate::assets::icon;
use crate::colors;
use crate::input::{InputEvent, TextInput};
use crate::widgets;
use crate::zoom::px;

pub(super) struct Noting {
    at: usize,
    input: Entity<TextInput>,
    _sub: Subscription,
}

impl TranscriptView {
    pub(super) fn cite(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.cites.push(Cite {
            text: text.to_string(),
            note: String::new(),
        });
        self.keep_draft(cx);
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }

    fn uncite(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.cites.len() {
            return;
        }
        self.cites.remove(ix);
        self.noting = None;
        self.keep_draft(cx);
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }

    fn open_note(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.cites.get(ix).map(|c| c.note.clone()) else {
            return;
        };
        let input = cx.new(|cx| {
            let mut i = TextInput::new("Add a note about this quote", false, cx);
            i.set_text(note, cx);
            i
        });
        let sub = cx.subscribe_in(&input, window, |v, _, e: &InputEvent, window, cx| match e {
            InputEvent::Submit => v.close_note(true, window, cx),
            InputEvent::Cancel => v.close_note(false, window, cx),
            _ => {}
        });
        window.focus(&input.focus_handle(cx), cx);
        self.noting = Some(Noting {
            at: ix,
            input,
            _sub: sub,
        });
        cx.notify();
    }

    fn close_note(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(n) = self.noting.take() else {
            return;
        };
        if save && let Some(cite) = self.cites.get_mut(n.at) {
            cite.note = n.input.read(cx).text().trim().to_string();
            self.keep_draft(cx);
        }
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }
}

/// The message the agent gets: each quote with its note under it, then what was typed.
pub(super) fn with_cites(cites: &[Cite], text: &str) -> String {
    cites
        .iter()
        .map(|c| match c.note.as_str() {
            "" => quoted(&c.text),
            note => format!("{}\n\n{note}", quoted(&c.text)),
        })
        .chain((!text.is_empty()).then(|| text.to_string()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn quoted(text: &str) -> String {
    text.trim()
        .lines()
        .map(|l| format!("> {l}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn tray(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    if v.cites.is_empty() {
        return None;
    }
    let editing = v.noting.as_ref().map(|n| n.at);
    let chips = v
        .cites
        .iter()
        .enumerate()
        .map(|(ix, c)| chip(ix, c, editing == Some(ix), cx))
        .collect::<Vec<_>>();
    let editor = v
        .noting
        .as_ref()
        .and_then(|n| Some(editor(v.cites.get(n.at)?, &n.input, cx)));
    Some(
        div()
            .px(px(14.))
            .pt(px(12.))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().flex_wrap().gap_2().children(chips))
            .children(editor)
            .into_any_element(),
    )
}

fn chip(ix: usize, cite: &Cite, editing: bool, cx: &mut Context<TranscriptView>) -> AnyElement {
    let label = match cite.note.is_empty() {
        true => one_line(&cite.text),
        false => one_line(&cite.note),
    };
    let tip: String = cite.text.chars().take(400).collect();
    div()
        .id(("cite", ix))
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .max_w(px(340.))
        .pl(px(9.))
        .pr(px(3.))
        .rounded(px(8.))
        .bg(colors::ink(if editing { 0.1 } else { 0.06 }))
        .text_size(px(12.5))
        .text_color(colors::text1())
        .tooltip(widgets::tip_text(tip))
        .child(icon("quote", 12., colors::text3()))
        .child(div().min_w_0().truncate().child(label))
        .child(
            div()
                .flex()
                .flex_none()
                .child(
                    widgets::icon_button(("cite-note", ix), "pencil", 22.)
                        .tooltip(widgets::tip(if cite.note.is_empty() {
                            "Add a note"
                        } else {
                            "Edit the note"
                        }))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |v, _: &ClickEvent, window, cx| {
                            match v.noting.as_ref().is_some_and(|n| n.at == ix) {
                                true => v.close_note(false, window, cx),
                                false => v.open_note(ix, window, cx),
                            }
                        })),
                )
                .child(
                    widgets::icon_button(("cite-x", ix), "x", 22.)
                        .tooltip(widgets::tip("Remove"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |v, _: &ClickEvent, window, cx| {
                            v.uncite(ix, window, cx)
                        })),
                ),
        )
        .into_any_element()
}

fn editor(cite: &Cite, input: &Entity<TextInput>, cx: &mut Context<TranscriptView>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::ink(0.03))
        .child(
            div()
                .pl(px(10.))
                .border_l_2()
                .border_color(colors::ink(0.16))
                .max_h(px(57.))
                .overflow_hidden()
                .text_size(px(12.5))
                .line_height(px(19.))
                .text_color(colors::text3())
                .child(cite.text.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .px(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors::border2())
                        .bg(colors::bg())
                        .text_size(px(13.))
                        .text_color(colors::text1())
                        .cursor_text()
                        .child(input.clone()),
                )
                .child(widgets::button("cite-cancel", "Cancel").on_click(
                    cx.listener(|v, _: &ClickEvent, window, cx| v.close_note(false, window, cx)),
                ))
                .child(widgets::primary("cite-save", "Save").h(px(28.)).on_click(
                    cx.listener(|v, _: &ClickEvent, window, cx| v.close_note(true, window, cx)),
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_go_first_each_with_its_note() {
        let cites = [
            Cite {
                text: "  first line\n\nsecond line  ".into(),
                note: "I don't agree".into(),
            },
            Cite {
                text: "other".into(),
                note: String::new(),
            },
        ];
        assert_eq!(
            with_cites(&cites, "Why?"),
            "> first line\n>\n> second line\n\nI don't agree\n\n> other\n\nWhy?"
        );
        assert_eq!(with_cites(&cites[1..], ""), "> other");
        assert_eq!(with_cites(&[], "hi"), "hi");
    }
}
