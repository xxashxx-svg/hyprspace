// The space's half of the title row (`crate::root::titlebar`): a new thread, the Open button (the
// folder in an editor, Explorer or Finder), and the dock toggle. The space's name and folder sit
// on the left, or the thread's agent, title and model when a structured thread is on screen, as it
// has no header of its own.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseButton, Window, div, prelude::*,
    px,
};
use hyprspace_proto::{Opener, ThreadKind};
use hyprspace_theme::MONO;

use super::ToggleDock;
use super::header::{opener_logo, short};
use crate::assets::{icon, mark};
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
    pub(crate) fn bar(&self, space: u64, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.state.space(space) else {
            return div().into_any_element();
        };
        let folder = s.cwd.clone();
        let open_with = self.state.open_with;
        let menu_open = self.work.popup.is_some();
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
                        .when(menu_open, |d| d.bg(colors::surface2()))
                        .hover(|s| s.bg(colors::surface2()))
                        .child(icon("chevron-down", 13., colors::text3()))
                        .on_click(cx.listener(|r, e: &ClickEvent, _, cx| r.toggle_popup(e, cx))),
                )
        });
        let thread = self
            .structured_on_screen()
            .and_then(|id| self.state.thread(id))
            .and_then(|(_, t)| match &t.kind {
                ThreadKind::Structured { launch } => Some((t.title.clone(), launch.clone())),
                _ => None,
            });
        let (badge, name, detail) = match thread {
            Some((title, launch)) => (
                Some(mark(launch.agent, 14., colors::brand(launch.agent).0)),
                title,
                Some(format!(
                    "{} · {}",
                    short(&launch.cwd),
                    self.model_label(&launch)
                )),
            ),
            None => (None, s.name.clone(), folder.as_deref().map(short)),
        };
        // the bar is part of the title row: its empty stretch drags the window, so the controls
        // occlude it to keep their clicks
        let controls = div()
            .id("bar-controls")
            .occlude()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.));
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(14.))
            .pr(px(6.))
            .children(badge.map(|b| div().flex_none().mr(px(2.)).child(b)))
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(name),
            )
            .children(detail.map(|d| {
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(d)
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
            .child(
                controls
                    .child(bar_button("bar-new", "plus", false).on_click(cx.listener(
                        move |r, _: &ClickEvent, window, cx| {
                            r.act(Action::NewThread(space), window, cx)
                        },
                    )))
                    .children(open_button)
                    .child(div().w(px(1.)).h(px(16.)).mx(px(8.)).bg(colors::border2()))
                    .child(self.limits.clone())
                    .child(
                        bar_button("bar-dock", "panel-right", self.state.dock.open).on_click(
                            cx.listener(|r, _: &ClickEvent, window, cx| {
                                r.toggle_dock(&ToggleDock, window, cx)
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// The structured thread on screen, which has no header and runs up to the title row.
    pub(crate) fn structured_on_screen(&self) -> Option<u64> {
        let crate::root::Screen::Thread(id) = self.screen else {
            return None;
        };
        let (_, t) = self.state.thread(id)?;
        matches!(t.kind, ThreadKind::Structured { .. }).then_some(id)
    }

    fn toggle_popup(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.work.popup = match self.work.popup {
            Some(_) => None,
            None => Some(e.position()),
        };
        cx.notify();
    }

    pub(crate) fn bar_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let at = self.work.popup?;
        let close = cx.listener(|r, _: &(), _, cx| {
            r.work.popup = None;
            cx.notify();
        });
        // open below the button, centered under the click
        let at = gpui::point(at.x - px(110.), at.y + px(18.));
        let content = self.open_menu(cx);
        Some(widgets::layer(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            content,
        ))
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
