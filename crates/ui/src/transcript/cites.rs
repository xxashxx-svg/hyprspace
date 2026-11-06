use std::time::Duration;

use gpui::{
    Anchor, AnyElement, Context, Focusable, FontWeight, IntoElement, Pixels, Point, ScrollHandle,
    ScrollWheelEvent, Window, anchored, deferred, div, point, prelude::*,
};
use hyprspace_proto::Cite;

use super::TranscriptView;
use crate::assets::icon;
use crate::colors;
use crate::input::quote_spans;
use crate::zoom::px;

const LABEL: usize = 32;

impl TranscriptView {
    pub(super) fn cite(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let label = match self.cites.iter().find(|c| c.text == text) {
            Some(c) => c.label.clone(),
            None => {
                let label = label_for(text, &self.cites);
                self.cites.push(Cite {
                    text: text.to_string(),
                    label: label.clone(),
                });
                self.sync_quotes(cx);
                label
            }
        };
        self.input.update(cx, |i, cx| {
            let before = i.text()[..i.cursor_offset()].chars().next_back();
            let lead = if before.is_some_and(|c| !c.is_whitespace()) {
                " "
            } else {
                ""
            };
            i.insert(&format!("{lead}{label} "), cx);
        });
        window.focus(&self.input.focus_handle(cx), cx);
    }

    pub(super) fn peek_quote(
        &mut self,
        at: Option<(usize, Point<Pixels>)>,
        cx: &mut Context<Self>,
    ) {
        if at.is_some() {
            if at.map(|a| a.0) != self.quote_peek.map(|a| a.0) {
                self.quote_scroll = ScrollHandle::new();
            }
            self.peek_close = None;
            self.quote_peek = at;
            cx.notify();
            return;
        }
        self.peek_close = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = this.update(cx, |v, cx| {
                if !v.peek_held {
                    v.quote_peek = None;
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn sync_quotes(&mut self, cx: &mut Context<Self>) {
        let labels = labels(&self.cites);
        self.input.update(cx, |i, cx| i.set_quotes(labels, cx));
    }
}

pub(super) fn labels(cites: &[Cite]) -> Vec<String> {
    cites.iter().map(|c| c.label.clone()).collect()
}

fn label_for(text: &str, cites: &[Cite]) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let short = match flat.char_indices().nth(LABEL) {
        None => flat,
        Some((at, _)) => {
            let cut = flat[..at].rfind(' ').filter(|&i| i > at / 2).unwrap_or(at);
            format!("{}…", flat[..cut].trim_end())
        }
    };
    (1..)
        .map(|n| match n {
            1 => format!("\"{short}\""),
            n => format!("\"{short} ({n})\""),
        })
        .find(|l| !cites.iter().any(|c| c.label == *l))
        .unwrap_or_default()
}

pub(super) fn expand(text: &str, cites: &[Cite]) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (range, n) in quote_spans(text, &labels(cites)) {
        push(&mut out, &text[last..range.start]);
        push(&mut out, &quoted(&cites[n].text));
        last = range.end;
    }
    push(&mut out, &text[last..]);
    out
}

fn push(out: &mut String, part: &str) {
    let part = part.trim();
    if part.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(part);
}

fn quoted(text: &str) -> String {
    text.trim()
        .lines()
        .map(|l| format!("> {l}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn peek(
    v: &TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> Option<AnyElement> {
    let (n, p) = v.quote_peek?;
    let cite = v.cites.get(n)?;
    // the thumb reads the size the body laid out at last frame, so a new card draws twice
    if v.quote_scroll.bounds().size.height <= Pixels::ZERO {
        window.request_animation_frame();
    }
    let lines = cite.text.lines().map(|l| match l.trim().is_empty() {
        true => div().h(px(4.)),
        false => div().child(l.trim_end().to_string()),
    });
    let quote = div().flex().flex_col().gap(px(5.)).children(lines);
    let body = div()
        .id("quote-peek-body")
        .track_scroll(&v.quote_scroll)
        .max_h(px(300.).min(p.y - px(64.)))
        .overflow_y_scroll()
        .pl(px(14.))
        .pr(px(20.))
        .pt(px(10.))
        .pb(px(12.))
        .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()))
        .child(quote);
    let head = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(14.))
        .pt(px(10.))
        .pb(px(8.))
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(icon("quote", 12., colors::text3()))
        .child("Cited text");
    let card = div()
        .id("quote-peek")
        .occlude()
        .on_hover(cx.listener(|v, over: &bool, _, cx| {
            v.peek_held = *over;
            if !*over {
                v.quote_peek = None;
            }
            cx.notify();
        }))
        .w(px(420.))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .child(head)
        .child(
            div()
                .relative()
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(colors::text1())
                .child(body)
                .children(thumb(&v.quote_scroll)),
        );
    Some(
        deferred(
            anchored()
                .anchor(Anchor::BottomLeft)
                .position(point(p.x, p.y - px(6.)))
                .child(card),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

fn thumb(scroll: &ScrollHandle) -> Option<AnyElement> {
    let view = scroll.bounds().size.height;
    let max = scroll.max_offset().y;
    if max <= Pixels::ZERO {
        return None;
    }
    let track = view - px(8.);
    let h = (track * (view / (view + max))).max(px(20.));
    let y = (track - h) * (-scroll.offset().y / max);
    Some(
        div()
            .absolute()
            .top(px(4.) + y)
            .right(px(5.))
            .w(px(3.))
            .h(h)
            .rounded_full()
            .bg(colors::ink(0.2))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cite(text: &str, cites: &[Cite]) -> Cite {
        Cite {
            text: text.into(),
            label: label_for(text, cites),
        }
    }

    #[test]
    fn a_label_is_the_quote_cut_short_and_never_repeats() {
        let long = cite(
            "Should the npm check be on by default? I'd say yes, with an honest switch.",
            &[],
        );
        assert_eq!(long.label, "\"Should the npm check be on by…\"");
        assert_eq!(cite("Click only", &[]).label, "\"Click only\"");
        let again = cite("Should the npm check be on by now, or later?", &[long]);
        assert_eq!(again.label, "\"Should the npm check be on by… (2)\"");
    }

    #[test]
    fn each_quote_opens_where_its_label_was() {
        let one = cite(" first line\n\nsecond line ", &[]);
        let two = cite("other", &[]);
        let text = format!("{} I don't agree\nwhich first? {}", one.label, two.label);
        assert_eq!(
            expand(&text, &[one, two]),
            "> first line\n>\n> second line\n\nI don't agree\nwhich first?\n\n> other"
        );
        assert_eq!(expand(" hi ", &[]), "hi");
    }
}
