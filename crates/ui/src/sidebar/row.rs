// One thread in the sidebar, on one line like zeron's: a status slot, the agent's mark, the
// title, and its age on the right (a running count while it works). Click to open it,
// right-click for rename and archive.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, IntoElement, MouseButton,
    Transformation, div, percentage, prelude::*, px,
};
use hyprspace_proto::Thread;
use hyprspace_theme::MONO;

use super::row_hover;
use crate::assets::{icon, mark};
use crate::colors;
use crate::root::{Action, MenuItems, Rename, Root, Screen, View};
use crate::time::ago;
use crate::transcript::Status;
use crate::widgets;

/// A running count, the way a stopwatch shows it: 45s, 3m 05s, 1h 02m.
fn elapsed(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs / 60 % 60),
    }
}

/// What sits in the row's status slot: a turning ring while it works, a pulsing dot when it
/// needs you, a check when it finished and you have not looked yet, a red dot when it failed.
fn glyph(status: Status, unseen: bool, id: u64) -> Option<AnyElement> {
    match status {
        Status::Working => Some(
            icon("ring", 11., colors::busy())
                .with_animation(
                    ("ring", id),
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |s, t| s.with_transformation(Transformation::rotate(percentage(t))),
                )
                .into_any_element(),
        ),
        Status::Waiting => Some(
            widgets::status_dot(status)
                .with_animation(
                    ("waiting", id),
                    Animation::new(Duration::from_millis(1000)).repeat(),
                    |d, t| d.opacity(0.35 + 0.65 * (t * 2.0 - 1.0).abs()),
                )
                .into_any_element(),
        ),
        Status::Done if unseen => Some(icon("check", 13., colors::ok()).into_any_element()),
        Status::Failed => Some(widgets::status_dot(status).into_any_element()),
        Status::Done | Status::Idle => None,
    }
}

impl Root {
    pub(crate) fn thread_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let running = match self.views.get(&id) {
            Some(View::Structured(v)) => v.read(cx).elapsed(),
            _ => None,
        };
        let badge = match t.agent() {
            Some(launch) => {
                mark(launch.agent, 13., colors::brand(launch.agent).0).into_any_element()
            }
            None => icon("terminal", 12., colors::text3()).into_any_element(),
        };
        let title: AnyElement = match &self.rename {
            Some((Rename::Thread(r), input, _)) if *r == id => div()
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::accent())
                .bg(colors::bg())
                .text_color(colors::text1())
                .child(input.clone())
                .into_any_element(),
            _ => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if selected {
                    colors::text1()
                } else {
                    colors::text2()
                })
                .child(t.title.clone())
                .into_any_element(),
        };
        let right = match running {
            Some(secs) => div()
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(colors::busy())
                .child(elapsed(secs)),
            None => div()
                .text_size(px(11.))
                .text_color(colors::text3())
                .child(ago(t.created, now)),
        };
        let mut menu: MenuItems = Vec::new();
        // an archived thread can't be a pane
        if !t.archived {
            menu.push(("Open beside".into(), Action::OpenBeside(id)));
        }
        menu.extend([
            ("Rename".into(), Action::Rename(Rename::Thread(id))),
            if t.archived {
                ("Restore".into(), Action::ArchiveThread(id, false))
            } else {
                ("Archive".into(), Action::ArchiveThread(id, true))
            },
            ("Remove".into(), Action::RemoveThread(id)),
        ]);
        div()
            .id(("thread", id))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(30.))
            .px(px(8.))
            .rounded(px(8.))
            .text_size(px(13.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(14.))
                    .children(glyph(status, self.unseen.contains(&id), id)),
            )
            .child(badge)
            .child(title)
            .child(div().flex_none().pl(px(4.)).child(right))
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_thread(id, window, cx)
            }))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_like_a_stopwatch() {
        assert_eq!(elapsed(45), "45s");
        assert_eq!(elapsed(185), "3m 05s");
        assert_eq!(elapsed(3720), "1h 02m");
    }
}
