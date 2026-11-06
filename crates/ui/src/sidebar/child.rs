// A delegated thread under its parent: one line on a guide that hangs off the parent, with who
// runs it, how it went and for how long. It shares the parent's project and branch, so it
// doesn't repeat them.

use gpui::{AnyElement, ClickEvent, Context, MouseButton, SharedString, canvas, div, prelude::*};
use hyprspace_proto::Thread;
use hyprspace_theme::MONO;

use super::row::{elapsed, ring};
use super::{EDGE, row_hover};
use crate::assets::{icon, mark};
use crate::colors;
use crate::root::{Root, Screen, View};
use crate::time::ago;
use crate::transcript::Status;
use crate::zoom::px;

const HEIGHT: f32 = 28.;
/// The guide's distance from the slot's left edge: under the parent's project tag.
const GUIDE: f32 = EDGE + 15.;

/// The guide down the left of a child's slot: a tick to the row, and on down to the next child
/// unless this is the last.
pub(super) fn guide(last: bool) -> AnyElement {
    let line = colors::ink(0.1);
    let mid = 2. + HEIGHT / 2.;
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(GUIDE))
        .w(px(10.))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(1.))
                .when(last, |d| d.h(px(mid)))
                .when(!last, |d| d.bottom_0())
                .bg(line),
        )
        .child(
            div()
                .absolute()
                .top(px(mid))
                .left_0()
                .w(px(8.))
                .h(px(1.))
                .bg(line),
        )
        .into_any_element()
}

pub(super) const INDENT: f32 = GUIDE + 10. - EDGE;

fn badge(name: &'static str, color: gpui::Hsla) -> AnyElement {
    div()
        .absolute()
        .right(px(-3.))
        .bottom(px(-3.))
        .size(px(9.))
        .rounded_full()
        .bg(colors::surface1())
        .flex()
        .items_center()
        .justify_center()
        .child(icon(name, 9., color))
        .into_any_element()
}

impl Root {
    fn kids(&self, parent: u64) -> Vec<&Thread> {
        self.state
            .spaces
            .iter()
            .flat_map(|s| &s.threads)
            .filter(|t| !t.side && t.parent == Some(parent) && t.active())
            .collect()
    }

    /// Whether a parent's delegated threads show under it: as picked by hand, else while one of
    /// them works or waits.
    pub(super) fn kids_shown(&self, parent: u64) -> bool {
        self.kids_open.get(&parent).copied().unwrap_or_else(|| {
            self.kids(parent).iter().any(|t| {
                matches!(
                    self.status.get(&t.id),
                    Some(Status::Working | Status::Waiting)
                )
            })
        })
    }

    /// On a parent's row: how many threads it delegated to, folding them away or back.
    pub(super) fn kids_chip(&self, parent: u64, cx: &mut Context<Self>) -> Option<AnyElement> {
        let n = self.kids(parent).len();
        if n == 0 {
            return None;
        }
        let open = self.kids_shown(parent);
        Some(
            div()
                .id(("kids", parent))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(3.))
                .h(px(16.))
                .pl(px(6.))
                .pr(px(3.))
                .rounded_full()
                .bg(colors::ink(0.06))
                .text_size(px(10.5))
                .text_color(colors::text2())
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.1)))
                .child(if n == 1 {
                    "1 agent".to_string()
                } else {
                    format!("{n} agents")
                })
                .child(icon(
                    if open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    10.,
                    colors::text3(),
                ))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    let open = r.kids_shown(parent);
                    r.kids_open.insert(parent, !open);
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    pub(crate) fn child_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let running = match self.views.get(&id) {
            Some(View::Structured(v)) => v.read(cx).elapsed(),
            _ => None,
        };
        let agent = t.agent().map(|l| l.agent);
        let face = match agent {
            Some(a) => mark(a, 12., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 11., colors::text3()).into_any_element(),
        };
        let (state, right): (Option<AnyElement>, AnyElement) = match status {
            Status::Working => (
                Some(ring(16., 3.)),
                div()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::busy())
                    .children(running.map(elapsed))
                    .into_any_element(),
            ),
            Status::Waiting => (
                None,
                div()
                    .text_color(colors::waiting())
                    .child("Needs you")
                    .into_any_element(),
            ),
            Status::Failed => (
                Some(badge("x", colors::error())),
                div()
                    .text_color(colors::error())
                    .child("Failed")
                    .into_any_element(),
            ),
            Status::Done => (
                Some(badge("check", colors::ok())),
                div()
                    .text_color(colors::text3())
                    .when(t.last_touch() > 0, |d| d.child(ago(t.last_touch(), now)))
                    .into_any_element(),
            ),
            Status::Idle => (
                None,
                div()
                    .text_color(colors::text3())
                    .when(t.last_touch() > 0, |d| d.child(ago(t.last_touch(), now)))
                    .into_any_element(),
            ),
        };
        let menu = self.thread_menu(t);
        let group: SharedString = format!("child-{id}").into();
        div()
            .id(("child", id))
            .group(group)
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(HEIGHT))
            .px(px(8.))
            .rounded(px(8.))
            .cursor_pointer()
            .text_size(px(12.5))
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .children(self.key_ring(id))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .child(face)
                    .children(state),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(colors::text1())
                    .child(t.title.clone()),
            )
            .child(div().flex_none().text_size(px(11.)).child(right))
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_thread(id, window, cx)
            }))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .on_hover(cx.listener(move |r, on: &bool, _, cx| r.row_hovered(id, *on, cx)))
            .child({
                let bounds = self.row_bounds.clone();
                canvas(
                    |_, _, _| (),
                    move |b, _, _, _| {
                        bounds.borrow_mut().insert(id, b);
                    },
                )
                .absolute()
                .size_full()
            })
            .into_any_element()
    }
}
