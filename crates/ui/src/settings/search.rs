// Settings search: the box at the top of the nav, the hits that take the views' place while it
// has text, and the jump that opens a hit's view, scrolls its row to the middle and rings it.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, Context, Entity, Focusable, FontWeight,
    HighlightStyle, KeyBinding, MouseButton, Pixels, StyledText, Window, actions, div, point,
    prelude::*,
};

use super::index::{self, Hit, Spot};
use super::{Root, TABS};
use crate::assets::icon;
use crate::input::{InputEvent, TextInput};
use crate::zoom::px;
use crate::{colors, widgets};

actions!(settings, [SearchPrev, SearchNext, FindSetting]);

/// Call after `input::bind_keys`, so the arrows walk the hits instead of the box.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SearchPrev, Some("SettingsSearch > TextInput")),
        KeyBinding::new("down", SearchNext, Some("SettingsSearch > TextInput")),
        KeyBinding::new("secondary-f", FindSetting, Some("Settings")),
    ]);
}

pub(super) struct Jump {
    spot: Spot,
    n: usize,
    scrolled: Rc<Cell<bool>>,
}

fn flash(t: f32) -> f32 {
    if t < 0.12 {
        t / 0.12
    } else if t < 0.55 {
        1.
    } else {
        1. - (t - 0.55) / 0.45
    }
}

impl Root {
    pub(super) fn search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some(input) = &self.settings.search {
            return input.clone();
        }
        let input = cx.new(|cx| TextInput::new("Search settings", false, cx));
        self._subs.push(
            cx.subscribe_in(
                &input,
                window,
                |r, input, e: &InputEvent, window, cx| match e {
                    InputEvent::Changed => {
                        r.settings.sel = 0;
                        r.settings.hits.scroll_to_item(0);
                        cx.notify();
                    }
                    InputEvent::Submit => {
                        let hits = index::search(input.read(cx).text());
                        if let Some(hit) = hits.get(r.settings.sel) {
                            r.jump_to(*hit, cx);
                        }
                    }
                    InputEvent::Cancel => r.settings_escape(window, cx),
                    _ => {}
                },
            ),
        );
        self.settings.search = Some(input.clone());
        input
    }

    pub(crate) fn clear_search(&self, cx: &mut Context<Self>) {
        if let Some(input) = &self.settings.search {
            input.update(cx, |i, cx| i.set_text("", cx));
        }
    }

    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.search_input(window, cx);
        input.update(cx, |i, cx| i.select_all_text(cx));
        window.focus(&input.focus_handle(cx), cx);
    }

    pub(super) fn settings_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
        if self.settings.restore.take().is_some() || self.settings.menu.take().is_some() {
            return;
        }
        if let Some(input) = self.settings.search.clone()
            && !input.read(cx).text().is_empty()
        {
            input.update(cx, |i, cx| i.set_text("", cx));
            return;
        }
        self.close_settings(window, cx);
    }

    fn step_hit(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(input) = &self.settings.search else {
            return;
        };
        let n = index::search(input.read(cx).text()).len() as isize;
        if n == 0 {
            return;
        }
        self.settings.sel = (self.settings.sel as isize + by).rem_euclid(n) as usize;
        // the count line sits above the first hit
        self.settings.hits.scroll_to_item(self.settings.sel + 1);
        cx.notify();
    }

    fn jump_to(&mut self, hit: Hit, cx: &mut Context<Self>) {
        self.settings.set_tab(hit.spot.tab);
        if hit.spot.section.is_empty() {
            self.settings
                .page
                .set_offset(point(Pixels::ZERO, Pixels::ZERO));
        } else {
            self.settings.jumps += 1;
            self.settings.jump = Some(Jump {
                spot: hit.spot,
                n: self.settings.jumps,
                scrolled: Rc::default(),
            });
        }
        cx.notify();
    }

    pub(super) fn spot(&self, spot: Spot, el: AnyElement, section: bool) -> AnyElement {
        let Some(jump) = self.settings.jump.as_ref().filter(|j| j.spot == spot) else {
            return el;
        };
        let (y, r) = if section { (-8., 15.) } else { (3., 9.) };
        let ring = div()
            .absolute()
            .top(px(y))
            .bottom(px(y))
            .left(px(-10.))
            .right(px(-10.))
            .rounded(px(r))
            .bg(colors::ink(0.045))
            .border_1()
            .border_color(colors::accent().opacity(0.45));
        let ring = if crate::slide::animations() {
            ring.with_animation(
                ("settings-ring", jump.n),
                Animation::new(Duration::from_millis(1500)),
                |d, t| d.opacity(flash(t)),
            )
            .into_any_element()
        } else {
            ring.into_any_element()
        };
        let page = self.settings.page.clone();
        let scrolled = jump.scrolled.clone();
        div()
            .relative()
            .child(ring)
            .child(el)
            .on_children_prepainted(move |b, window, _| {
                let Some(row) = b.get(1) else {
                    return;
                };
                if scrolled.replace(true) {
                    return;
                }
                let view = page.bounds();
                let offset = page.offset();
                let y = offset.y - (row.center().y - view.center().y);
                let y = if y > Pixels::ZERO {
                    Pixels::ZERO
                } else if y < -page.max_offset().y {
                    -page.max_offset().y
                } else {
                    y
                };
                page.set_offset(point(offset.x, y));
                window.refresh();
            })
            .into_any_element()
    }

    pub(super) fn search_box(
        &self,
        input: &Entity<TextInput>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = input.focus_handle(cx);
        let focused = handle.is_focused(window);
        let end = if input.read(cx).text().is_empty() {
            widgets::keycap(if cfg!(target_os = "macos") {
                "Cmd F"
            } else {
                "Ctrl F"
            })
            .into_any_element()
        } else {
            let input = input.clone();
            div()
                .id("settings-search-clear")
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .size(px(18.))
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.08)))
                .child(icon("x", 12., colors::text3()))
                .tooltip(widgets::tip("Clear"))
                .on_click(move |_: &ClickEvent, window, cx| {
                    input.update(cx, |i, cx| i.set_text("", cx));
                    window.focus(&input.focus_handle(cx), cx);
                })
                .into_any_element()
        };
        div()
            .id("settings-search")
            .key_context("SettingsSearch")
            .on_action(cx.listener(|r, _: &SearchPrev, _, cx| r.step_hit(-1, cx)))
            .on_action(cx.listener(|r, _: &SearchNext, _, cx| r.step_hit(1, cx)))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .pl(px(10.))
            .pr(px(8.))
            .mt(px(2.))
            .mb(px(8.))
            .rounded(px(7.))
            .border_1()
            .border_color(if focused {
                colors::ink(0.12)
            } else {
                gpui::transparent_black()
            })
            .bg(colors::ink(if focused { 0.06 } else { 0.045 }))
            .when(!focused, |d| d.hover(|s| s.bg(colors::ink(0.06))))
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&handle, cx);
                // or Settings, which tracks focus too, takes it back
                window.prevent_default();
            })
            .child(icon("search", 14., colors::text3()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .child(input.clone()),
            )
            .child(end)
            .into_any_element()
    }

    pub(super) fn search_hits(&self, query: &str, cx: &mut Context<Self>) -> AnyElement {
        let hits = index::search(query);
        let list = div()
            .id("settings-hits")
            .track_scroll(&self.settings.hits)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(2.));
        if hits.is_empty() {
            return list
                .px(px(10.))
                .py(px(6.))
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(colors::text2())
                        .child(format!("No setting matches \"{}\".", query.trim())),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child("Try a word like font, theme or phone."),
                )
                .into_any_element();
        }
        let count = div()
            .flex_none()
            .px(px(10.))
            .pt(px(2.))
            .pb(px(6.))
            .text_size(px(11.5))
            .text_color(colors::text3())
            .child(match hits.len() {
                1 => "1 result".to_string(),
                n => format!("{n} results"),
            });
        let lit = HighlightStyle {
            color: Some(colors::text1()),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        };
        let rows = hits.into_iter().enumerate().map(|(i, hit)| {
            let on = i == self.settings.sel;
            let entry = TABS.iter().find(|e| e.tab == hit.spot.tab);
            let sub = match (hit.spot.section, entry) {
                ("", Some(e)) => e.desc.to_string(),
                (s, Some(e)) if s == hit.spot.title => e.label.to_string(),
                (s, Some(e)) => format!("{} \u{b7} {s}", e.label),
                (s, None) => s.to_string(),
            };
            let title = StyledText::new(hit.spot.title).with_highlights(
                index::matched(hit.spot.title, query)
                    .into_iter()
                    .map(|r| (r, lit)),
            );
            let keys = (!hit.keys.is_empty()).then(|| {
                div()
                    .flex()
                    .flex_none()
                    .gap(px(3.))
                    .mt(px(1.))
                    .children(hit.keys.iter().map(|k| widgets::keycap(*k)))
            });
            div()
                .id(("settings-hit", i))
                .flex_none()
                .flex()
                .items_start()
                .gap(px(10.))
                .px(px(10.))
                .py(px(7.))
                .rounded(px(7.))
                .cursor_pointer()
                .when(on, |d| d.bg(colors::ink(0.07)))
                .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.04))))
                .child(div().flex_none().mt(px(1.)).child(icon(
                    entry.map_or("settings", |e| e.icon),
                    15.,
                    if on { colors::text1() } else { colors::text3() },
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.))
                                .text_color(if on { colors::text1() } else { colors::text2() })
                                .child(title),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child(sub),
                        ),
                )
                .children(keys)
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.settings.sel = i;
                    r.jump_to(hit, cx);
                    // Settings took focus on the click; the arrows should still walk the hits
                    if let Some(input) = &r.settings.search {
                        window.focus(&input.focus_handle(cx), cx);
                    }
                }))
        });
        list.child(count).children(rows).into_any_element()
    }
}
