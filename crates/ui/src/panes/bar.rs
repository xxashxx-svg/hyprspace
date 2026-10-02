// The row above a space's panes, carrying what the Tauri app's titlebar held for a space: a new
// thread, the layout picker, the Open button (the folder in an editor, Explorer or Finder), and
// the dock toggle. The space's name and folder sit on the left.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseButton, Window, div, prelude::*,
    px,
};
use hyprspace_proto::Opener;
use hyprspace_theme::MONO;

use super::header::{opener_logo, short};
use super::layout::{self, Layout};
use super::{Popup, ToggleDock};
use crate::assets::icon;
use crate::colors;
use crate::root::{Action, Root};
use crate::widgets;

/// A bar button: a square that lifts on hover, or stays lifted while its menu is open.
fn bar_button(id: &'static str, name: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(px(6.))
        .cursor_pointer()
        .when(on, |d| d.bg(colors::surface2()))
        .hover(|s| s.bg(colors::surface2()))
        .child(icon(
            name,
            14.,
            if on { colors::text1() } else { colors::text2() },
        ))
}

impl Root {
    pub(crate) fn bar(&self, space: u64, panes: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.state.space(space) else {
            return div().into_any_element();
        };
        let folder = s.cwd.clone();
        let open_with = self.state.open_with;
        let popup = self.work.popup.map(|(_, p)| p);
        let open_button = folder.clone().map(|dir| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(26.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border1())
                .overflow_hidden()
                .child(
                    div()
                        .id("open-main")
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h_full()
                        .pl(px(7.))
                        .pr(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(colors::surface2()))
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(opener_logo(open_with, 14.))
                        .child("Open")
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.open_in(open_with, &dir, cx)
                        })),
                )
                .child(
                    div()
                        .id("open-caret")
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.))
                        .h_full()
                        .border_l_1()
                        .border_color(colors::border1())
                        .cursor_pointer()
                        .when(popup == Some(Popup::Open), |d| d.bg(colors::surface2()))
                        .hover(|s| s.bg(colors::surface2()))
                        .child(icon("chevron-down", 13., colors::text3()))
                        .on_click(cx.listener(|r, e: &ClickEvent, _, cx| {
                            r.toggle_popup(Popup::Open, e, cx)
                        })),
                )
        });
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(52.))
            .pl(px(14.))
            .pr(px(10.))
            .border_b_1()
            .border_color(colors::border0())
            .child(
                div()
                    .flex_none()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(s.name.clone()),
            )
            .children(folder.as_deref().map(|f| {
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(short(f))
            }))
            .child(div().flex_1())
            .children(self.work.notice.clone().map(|n| {
                div()
                    .min_w_0()
                    .truncate()
                    .mr(px(6.))
                    .text_size(px(12.))
                    .text_color(colors::error())
                    .child(n)
            }))
            .child(bar_button("bar-new", "plus", false).on_click(cx.listener(
                move |r, _: &ClickEvent, window, cx| r.act(Action::NewThread(space), window, cx),
            )))
            .when(panes >= 2 && !layout::presets(panes).is_empty(), |d| {
                d.child(
                    bar_button("bar-layout", "layout-grid", popup == Some(Popup::Layout)).on_click(
                        cx.listener(|r, e: &ClickEvent, _, cx| {
                            r.toggle_popup(Popup::Layout, e, cx)
                        }),
                    ),
                )
            })
            .children(open_button)
            .child(div().w(px(1.)).h(px(16.)).mx(px(8.)).bg(colors::border2()))
            .child(
                bar_button("bar-dock", "panel-right", self.state.dock.open).on_click(cx.listener(
                    |r, _: &ClickEvent, window, cx| r.toggle_dock(&ToggleDock, window, cx),
                )),
            )
            .into_any_element()
    }

    fn toggle_popup(&mut self, which: Popup, e: &ClickEvent, cx: &mut Context<Self>) {
        self.work.popup = match self.work.popup {
            Some((_, p)) if p == which => None,
            _ => Some((e.position(), which)),
        };
        cx.notify();
    }

    pub(crate) fn bar_popup(
        &self,
        space: u64,
        panes: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (at, which) = self.work.popup?;
        let close = cx.listener(|r, _: &(), _, cx| {
            r.work.popup = None;
            cx.notify();
        });
        // open below the button, centered under the click
        let at = gpui::point(at.x - px(110.), at.y + px(18.));
        let content = match which {
            Popup::Layout => self.layout_menu(space, panes, cx),
            Popup::Open => self.open_menu(cx),
        };
        Some(widgets::layer(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            content,
        ))
    }

    /// Thumbnails of the layouts for this many panes, drawn from the same data the grid uses.
    fn layout_menu(&self, space: u64, panes: usize, cx: &mut Context<Self>) -> AnyElement {
        let picked = self
            .state
            .space(space)
            .and_then(|s| s.grid.layouts.get(&panes).cloned());
        let current = layout::resolve(panes, picked.as_deref()).id;
        let options = layout::presets(panes).into_iter().map(|l| {
            let id = l.id;
            let on = id == current;
            div()
                .id(id)
                .group(id)
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .pt(px(8.))
                .px(px(6.))
                .pb(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(if on {
                    colors::accent().opacity(0.55)
                } else {
                    colors::border1().opacity(0.)
                })
                .when(on, |d| d.bg(colors::accent().opacity(0.12)))
                .text_color(if on { colors::text1() } else { colors::text3() })
                .cursor_pointer()
                .hover(|s| s.bg(colors::surface3()).text_color(colors::text1()))
                .child(thumb(&l, on, id))
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_center()
                        .line_height(px(13.))
                        .child(l.label),
                )
                .on_click(
                    cx.listener(move |r, _: &ClickEvent, _, cx| r.set_layout(space, panes, id, cx)),
                )
        });
        div()
            .w(px(224.))
            .p(px(8.))
            .rounded(px(10.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .child(
                div()
                    .px(px(4.))
                    .pt(px(2.))
                    .pb(px(8.))
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text3())
                    .child(format!("LAYOUT · {panes} PANES")),
            )
            .child(div().grid().grid_cols(2).gap(px(6.)).children(options))
            .child(
                div()
                    .px(px(4.))
                    .pt(px(8.))
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child("Ctrl+click a thread in the sidebar to open it beside these."),
            )
            .into_any_element()
    }

    /// The Open button's menu: which app the button opens the folder in.
    fn open_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.state.open_with;
        let row = |o: Opener, cx: &mut Context<Self>| {
            div()
                .id(o.name())
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .px(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(colors::text1())
                .hover(|s| s.bg(colors::surface3()))
                .when(o == current, |d| d.bg(colors::surface3()))
                .child(opener_logo(o, 14.))
                .child(div().flex_1().child(o.name()))
                .when(o == current, |d| {
                    d.child(icon("check", 13., colors::text2()))
                })
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.pick_opener(o, cx)))
        };
        let editors: Vec<_> = self
            .work
            .openers
            .iter()
            .copied()
            .filter(|o| *o != Opener::Files)
            .map(|o| row(o, cx))
            .collect();
        let has_editors = !editors.is_empty();
        div()
            .id("open-menu")
            .min_w(px(190.))
            .p(px(5.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .rounded(px(10.))
            .border_1()
            .border_color(colors::ink(0.13))
            .bg(colors::surface3())
            .shadow(colors::shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(editors)
            .when(has_editors, |d| d.child(widgets::menu_rule()))
            .child(row(Opener::Files, cx))
            .into_any_element()
    }
}

/// A layout drawn small: its cells as rounded blocks on the same grid.
fn thumb(l: &Layout, on: bool, group: &'static str) -> AnyElement {
    let (w, h, gap) = (90., 40., 3.);
    let cw = (w - gap * (l.cols as f32 - 1.)) / l.cols as f32;
    let rh = (h - gap * (l.rows as f32 - 1.)) / l.rows as f32;
    let cells = l.cells.iter().map(|c| {
        let x = c.cols.start as f32 * (cw + gap);
        let y = c.rows.start as f32 * (rh + gap);
        let cw_span = c.cols.len() as f32 * (cw + gap) - gap;
        let rh_span = c.rows.len() as f32 * (rh + gap) - gap;
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(cw_span))
            .h(px(rh_span))
            .rounded(px(2.))
            .bg(if on {
                colors::accent().opacity(0.9)
            } else {
                colors::text3().opacity(0.5)
            })
            .group_hover(group, |s| s.bg(colors::accent().opacity(0.9)))
    });
    div()
        .relative()
        .w(px(w))
        .h(px(h))
        .children(cells)
        .into_any_element()
}
