use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, Focusable, ListOffset, Subscription,
    Window, div, prelude::*,
};

use super::TranscriptView;
use super::model::Item;
use super::rows::{self, Row};
use crate::input::{InputEvent, Newline, TextInput};
use crate::markdown::{self, find::occurrences};
use crate::zoom::px;
use crate::{colors, widgets};

#[derive(Clone, Copy)]
struct Hit {
    item: usize,
    nth: usize,
    at: f32,
}

pub(super) struct Find {
    input: Entity<TextInput>,
    hits: Vec<Hit>,
    current: Option<usize>,
    aim: Option<(usize, f32, u8)>,
    _sub: Subscription,
}

impl Find {
    pub(super) fn hunt(&self, cx: &gpui::App) -> (String, Option<(usize, usize)>) {
        let query = self.input.read(cx).text().to_string();
        let current = self
            .current
            .and_then(|i| self.hits.get(i))
            .map(|h| (h.item, h.nth));
        (query, current)
    }
}

fn hits(items: &[Item], query: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    for (item, it) in items.iter().enumerate() {
        let mut texts = Vec::new();
        match it {
            Item::User { text, .. } => texts.push(text.clone()),
            Item::Text {
                blocks: Some(b), ..
            } => markdown::find::texts(b, &mut texts),
            Item::Text { source, .. } => {
                markdown::find::texts(&markdown::parse(source), &mut texts)
            }
            _ => continue,
        }
        let total = texts.iter().map(String::len).sum::<usize>().max(1) as f32;
        let (mut before, mut nth) = (0, 0);
        for t in &texts {
            for r in occurrences(t, query) {
                out.push(Hit {
                    item,
                    nth,
                    at: (before + r.start) as f32 / total,
                });
                nth += 1;
            }
            before += t.len();
        }
    }
    out
}

impl TranscriptView {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            let input = cx.new(|cx| TextInput::new("Find in thread", false, cx));
            let sub = cx.subscribe_in(&input, window, |v, _, e: &InputEvent, window, cx| match e {
                InputEvent::Changed => v.refind(true, cx),
                InputEvent::Submit => v.step_find(true, cx),
                InputEvent::Cancel => v.close_find(window, cx),
                InputEvent::Images(_)
                | InputEvent::Unmarked(_)
                | InputEvent::Peek(_)
                | InputEvent::Open(_) => {}
            });
            self.find = Some(Find {
                input,
                hits: Vec::new(),
                current: None,
                aim: None,
                _sub: sub,
            });
        }
        if let Some(f) = &self.find {
            let input = f.input.clone();
            input.update(cx, |i, cx| i.select_all_text(cx));
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    pub(super) fn finding(&self) -> bool {
        self.find.as_ref().is_some_and(|f| f.current.is_some())
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }

    pub(super) fn refind(&mut self, jump: bool, cx: &mut Context<Self>) {
        let top = self.rows[self.list.logical_scroll_top().item_ix.min(self.rows.len())..]
            .iter()
            .find_map(|r| match r {
                Row::Item { ix, .. } | Row::Part { ix, .. } => Some(*ix),
                _ => None,
            })
            .unwrap_or(0);
        let Some(f) = self.find.as_mut() else {
            return;
        };
        let query = f.input.read(cx).text().to_string();
        let before = f
            .current
            .and_then(|i| f.hits.get(i))
            .map(|h| (h.item, h.nth));
        f.hits = hits(&self.model.items, &query);
        f.current = if f.hits.is_empty() {
            None
        } else if jump {
            Some(f.hits.iter().position(|h| h.item >= top).unwrap_or(0))
        } else {
            before.and_then(|b| f.hits.iter().position(|h| (h.item, h.nth) == b))
        };
        if jump {
            self.reveal_hit();
        }
        cx.notify();
    }

    fn step_find(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(f) = self.find.as_mut() else {
            return;
        };
        let n = f.hits.len();
        if n == 0 {
            return;
        }
        let i = f.current.unwrap_or(if down { n - 1 } else { 0 });
        f.current = Some(if down { (i + 1) % n } else { (i + n - 1) % n });
        self.reveal_hit();
        cx.notify();
    }

    fn reveal_hit(&mut self) {
        let Some(h) = self
            .find
            .as_ref()
            .and_then(|f| f.current.and_then(|i| f.hits.get(i)).copied())
        else {
            return;
        };
        if matches!(self.model.items.get(h.item), Some(Item::User { .. })) {
            self.expanded.insert(h.item);
        }
        let Some(first) = self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Item { ix, .. } if *ix == h.item))
        else {
            return;
        };
        let (part, at) = self.piece_of(h.item, h.at);
        let row = first + part;
        self.follow.wheel(true);
        self.list.scroll_to(ListOffset {
            item_ix: row,
            offset_in_item: px(0.),
        });
        if let Some(f) = self.find.as_mut() {
            f.aim = Some((row, at, 4));
        }
    }

    /// The piece of a reply cut into rows that holds the point `at` of the way through its text,
    /// and how far through that piece it is.
    fn piece_of(&self, item: usize, at: f32) -> (usize, f32) {
        let parts = self
            .rows
            .iter()
            .filter(|r| matches!(r, Row::Part { ix, .. } if *ix == item));
        if parts.count() == 0 {
            return (0, at);
        }
        let Some(Item::Text {
            blocks: Some(b), ..
        }) = self.model.items.get(item)
        else {
            return (0, at);
        };
        let units = markdown::units(b);
        let lens: Vec<usize> = rows::stretches(&units)
            .into_iter()
            .map(|r| {
                let mut texts = Vec::new();
                for &u in &units[r] {
                    markdown::find::unit_texts(b, u, &mut texts);
                }
                texts.iter().map(String::len).sum()
            })
            .collect();
        let total = lens.iter().sum::<usize>().max(1) as f32;
        let mut before = 0.;
        for (part, len) in lens.iter().enumerate() {
            let len = *len as f32;
            if at * total <= before + len || part + 1 == lens.len() {
                return (part, ((at * total - before) / len.max(1.)).clamp(0., 1.));
            }
            before += len;
        }
        (0, at)
    }

    // a row far off screen has no height yet, so the match is aimed at once the list measured it
    pub(super) fn aim_find(&mut self, window: &mut Window) {
        let Some((row, at, tries)) = self.find.as_ref().and_then(|f| f.aim) else {
            return;
        };
        let aim = match self.list.bounds_for_item(row) {
            Some(b) => {
                let y = b.size.height * at - self.list.viewport_bounds().size.height * 0.35;
                if y > px(0.) {
                    self.list.scroll_to(ListOffset {
                        item_ix: row,
                        offset_in_item: y,
                    });
                }
                None
            }
            None if tries > 0 => {
                window.request_animation_frame();
                Some((row, at, tries - 1))
            }
            None => None,
        };
        if let Some(f) = self.find.as_mut() {
            f.aim = aim;
        }
    }

    pub(super) fn find_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let f = self.find.as_ref()?;
        let empty = f.input.read(cx).text().is_empty();
        let count = match (f.current, f.hits.len()) {
            (_, 0) if empty => String::new(),
            (_, 0) => "No results".to_string(),
            (Some(i), n) => format!("{}/{n}", i + 1),
            (None, n) => n.to_string(),
        };
        Some(
            div()
                .id("thread-find")
                .absolute()
                .top(px(10.))
                .right(px(18.))
                .flex()
                .items_center()
                .gap(px(4.))
                .w(px(340.))
                .h(px(38.))
                .pl(px(12.))
                .pr(px(6.))
                .rounded(px(10.))
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface2())
                .shadow(colors::shadow())
                .occlude()
                .capture_action(cx.listener(|v, _: &Newline, _, cx| {
                    v.step_find(false, cx);
                    cx.stop_propagation();
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.))
                        .text_color(colors::text1())
                        .child(f.input.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .px(px(6.))
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(count),
                )
                .child(
                    widgets::icon_button("thread-find-up", "chevron-up", 26.)
                        .tooltip(widgets::tip("Previous"))
                        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.step_find(false, cx))),
                )
                .child(
                    widgets::icon_button("thread-find-down", "chevron-down", 26.)
                        .tooltip(widgets::tip("Next"))
                        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.step_find(true, cx))),
                )
                .child(
                    widgets::icon_button("thread-find-close", "x", 26.)
                        .tooltip(widgets::tip("Close"))
                        .on_click(
                            cx.listener(|v, _: &ClickEvent, window, cx| v.close_find(window, cx)),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_count_each_message_in_order() {
        let items = [
            Item::User {
                text: "Fix the login page".into(),
                images: Vec::new(),
                steer: false,
            },
            Item::Thinking {
                text: "login login".into(),
                open: false,
            },
            Item::Text {
                source: "The **login** form posts twice.

```
login()
```
"
                .into(),
                blocks: None,
            },
        ];
        let found: Vec<_> = hits(&items, "LOGIN")
            .iter()
            .map(|h| (h.item, h.nth))
            .collect();
        assert_eq!(found, [(0, 0), (2, 0), (2, 1)]);
        assert!(hits(&items, "").is_empty());
    }
}
