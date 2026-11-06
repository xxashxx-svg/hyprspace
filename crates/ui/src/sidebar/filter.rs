use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    prelude::*,
};

use super::{RADIUS, row_hover, tag};
use crate::assets::icon;
use crate::colors;
use crate::root::{Action, Root};
use crate::widgets;
use crate::zoom::px;

impl Root {
    pub(super) fn filter_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let picked: Vec<_> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived && self.filter.contains(&s.id))
            .take(2)
            .map(|s| tag(&s.name, self.look_of(s), false))
            .collect();
        let on = !picked.is_empty();
        div()
            .id("sidebar-filter")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(RADIUS))
            .cursor_pointer()
            .when(on, |d| d.bg(row_hover()))
            .hover(|s| s.bg(row_hover()))
            .tooltip(widgets::tip("Filter by space"))
            .map(|d| {
                if on {
                    d.children(
                        picked
                            .into_iter()
                            .enumerate()
                            .map(|(i, t)| div().when(i > 0, |d| d.ml(px(-6.))).child(t)),
                    )
                } else {
                    d.child(icon("list-filter", 15., colors::text3()))
                }
            })
            .on_click(cx.listener(|r, e: &ClickEvent, _, cx| {
                r.menu = None;
                r.filter_menu = Some(e.position());
                cx.notify();
            }))
            .into_any_element()
    }

    pub(super) fn shows_space(&self, space: u64) -> bool {
        self.filter.is_empty()
            || self.filter.contains(&space)
            || !self
                .state
                .spaces
                .iter()
                .any(|s| !s.archived && self.filter.contains(&s.id))
    }

    fn pick_space(&mut self, space: u64, add: bool, cx: &mut Context<Self>) {
        if add {
            if !self.filter.remove(&space) {
                self.filter.insert(space);
            }
        } else if self.filter.len() == 1 && self.filter.contains(&space) {
            self.filter.clear();
        } else {
            self.filter = [space].into();
        }
        cx.notify();
    }

    pub(crate) fn filter_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let at = self.filter_menu?;
        let all = div()
            .id("filter-all")
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(28.))
            .px(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.06)))
            .text_size(px(12.5))
            .child(icon("list-filter", 14., colors::text2()))
            .child(div().flex_1().child("All spaces"))
            .when(self.filter.is_empty(), |d| {
                d.child(icon("check", 13., colors::text1()))
            })
            .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                r.filter.clear();
                r.filter_menu = None;
                cx.notify();
            }));
        let rows: Vec<AnyElement> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .map(|s| self.space_choice(s, at, cx))
            .collect();
        let close = cx.listener(|r, _: &(), _, cx| {
            r.filter_menu = None;
            cx.notify();
        });
        Some(widgets::popup(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            div()
                .w(px(260.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(all)
                .child(div().my(px(4.)).h(px(1.)).bg(colors::border1()))
                .child(
                    div()
                        .id("filter-spaces")
                        .max_h(px(360.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .children(rows),
                )
                .child(
                    div()
                        .px(px(10.))
                        .pt(px(6.))
                        .pb(px(2.))
                        .text_size(px(11.))
                        .text_color(colors::text3())
                        .child("Shift+click picks more than one"),
                ),
        ))
    }

    fn space_choice(
        &self,
        s: &hyprspace_proto::Space,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = s.id;
        let group = format!("filter-{id}");
        let picked = self.filter.contains(&id);
        let mirrored = s.machine.is_some();
        let action = |key: &'static str, glyph: &'static str, tip: &'static str| {
            div()
                .id((key, id as usize))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.08)))
                .tooltip(widgets::tip(tip))
                .child(icon(glyph, 13., colors::text2()))
        };
        div()
            .id(("filter-space", id as usize))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(28.))
            .pl(px(10.))
            .pr(px(4.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.06)))
            .text_size(px(12.5))
            .child(tag(&s.name, self.look_of(s), false))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(picked, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(s.name.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(1.))
                    .opacity(0.)
                    .group_hover(group.clone(), |s| s.opacity(1.))
                    .when(!mirrored, |d| {
                        d.child(action("filter-look", "palette", "Icon and color").on_click(
                            cx.listener(move |r, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                r.filter_menu = None;
                                r.open_icon_picker(id, at, window, cx);
                            }),
                        ))
                    })
                    .child(
                        action("filter-new", "square-pen", "New thread here").on_click(
                            cx.listener(move |r, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                r.filter_menu = None;
                                r.act(Action::NewThread(id), window, cx);
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(16.))
                    .when(picked, |d| d.child(icon("check", 13., colors::text1()))),
            )
            .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                r.pick_space(id, e.modifiers().shift, cx);
                if !e.modifiers().shift {
                    r.filter_menu = None;
                }
            }))
            .into_any_element()
    }
}
