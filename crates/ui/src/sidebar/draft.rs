use gpui::{AnyElement, ClickEvent, Context, FontWeight, SharedString, Stateful, div, prelude::*};
use hyprspace_proto::{Draft, Space, Thread};

use super::icon::Look;
use super::{row_hover, tag};
use crate::assets::icon;
use crate::colors;
use crate::root::{Root, Screen};
use crate::zoom::px;

impl Root {
    pub(crate) fn draft_row(&self, d: &Draft, cx: &mut Context<Self>) -> AnyElement {
        let id = d.id;
        let group: SharedString = format!("draft-{id}").into();
        let (space, look) = self
            .state
            .space(d.space)
            .map(|s| (s.name.clone(), self.look_of(s)))
            .unwrap_or_default();
        let label = space.clone();
        draft_card(
            ("draft", id),
            self.drafting() == Some(id),
            (&space, look),
            label,
            &d.text,
        )
        .group(group.clone())
        .child(
            div()
                .absolute()
                .top(px(5.))
                .right(px(6.))
                .flex()
                .p(px(1.))
                .rounded(px(6.))
                .bg(colors::surface3())
                .opacity(0.)
                .group_hover(group, |s| s.opacity(1.))
                .child(
                    self.row_button(id, "discard-draft", "x", cx, |r, id, _, _, cx| {
                        r.discard_draft(id, cx)
                    }),
                ),
        )
        .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
            r.menu = None;
            r.open_draft(id, window, cx)
        }))
        .into_any_element()
    }

    /// A settled or snoozed thread's unsent message, which its hidden row can't show.
    pub(crate) fn thread_draft_row(
        &self,
        s: &Space,
        t: &Thread,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = t.id;
        let text = if t.draft.trim().is_empty() {
            "Pictures to send"
        } else {
            t.draft.as_str()
        };
        draft_card(
            ("thread-draft", id),
            matches!(self.screen, Screen::Thread(on) if on == id),
            (&s.name, self.look_of(s)),
            t.title.clone(),
            text,
        )
        .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
            r.menu = None;
            r.open_thread(id, window, cx)
        }))
        .into_any_element()
    }
}

/// A draft's card: its project and `label` over the start of what it says.
fn draft_card(
    id: (&'static str, u64),
    selected: bool,
    (space, look): (&str, Look),
    label: String,
    text: &str,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .gap(px(3.))
        .pt(px(8.))
        .px(px(8.))
        .pb(px(9.))
        .rounded(px(8.))
        .cursor_pointer()
        .when(selected, |d| d.bg(colors::surface3()))
        .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(7.))
                .min_w_0()
                .child(icon("square-pen", 12., colors::text2()))
                .child(tag(space, look.clone(), false))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(match look.hue {
                            Some(h) => colors::tag(space, Some(h)).1,
                            None => colors::text3(),
                        })
                        .child(label),
                ),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(crate::composer::title_of(text)),
        )
}
