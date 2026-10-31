use gpui::{AnyElement, ClickEvent, Context, FontWeight, SharedString, div, prelude::*};
use hyprspace_proto::Draft;

use super::{row_hover, tag};
use crate::assets::icon;
use crate::colors;
use crate::root::Root;
use crate::zoom::px;

impl Root {
    pub(crate) fn draft_row(&self, d: &Draft, cx: &mut Context<Self>) -> AnyElement {
        let id = d.id;
        let selected = self.drafting() == Some(id);
        let group: SharedString = format!("draft-{id}").into();
        let (space, look) = self
            .state
            .space(d.space)
            .map(|s| (s.name.clone(), self.look_of(s)))
            .unwrap_or_default();
        div()
            .id(("draft", id))
            .group(group.clone())
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
                    .child(tag(&space, look.clone(), false))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(match look.hue {
                                Some(h) => colors::tag(&space, Some(h)).1,
                                None => colors::text3(),
                            })
                            .child(space.clone()),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(crate::composer::title_of(&d.text)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(5.))
                    .right(px(6.))
                    .flex()
                    .p(px(1.))
                    .rounded(px(7.))
                    .bg(colors::surface3())
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(
                        self.row_button(id, "discard-draft", "x", cx, |r, id, _, _, cx| {
                            r.discard_draft(id, cx)
                        })
                        .tooltip(crate::widgets::tip("Discard draft")),
                    ),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_draft(id, window, cx)
            }))
            .into_any_element()
    }
}
