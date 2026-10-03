// One thread in the sidebar, as a card like zeron's: the model in small grey type with the age
// on the right (a running count while it works), and under it the status, the agent's mark and
// the title. While the agent works, a line says what it does (the tool it runs, why it waits,
// what it concluded), and each subagent it has running gets a line of its own with its count,
// like the Tauri app's rows.
// Click to open it, right-click for rename and archive.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, IntoElement, MouseButton,
    SharedString, Transformation, div, percentage, prelude::*, px,
};
use hyprspace_proto::{Thread, ThreadKind};
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

/// Subagents shown before the rest fold into a count.
const SUBS_SHOWN: usize = 4;

/// The subagents under a row, on a guide line: each one's task and how long it has run. A new
/// one fades in rather than popping.
fn subagents(thread: u64, subs: &[(String, String, u64)]) -> AnyElement {
    let more = subs.len().saturating_sub(SUBS_SHOWN);
    div()
        .mt(px(3.))
        .ml(px(5.))
        .pl(px(10.))
        .border_l_1()
        .border_color(colors::ink(0.1))
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(subs.iter().take(SUBS_SHOWN).map(|(key, label, secs)| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(18.))
                .text_size(px(11.5))
                .child(icon("bot", 11., colors::text3()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(colors::text2())
                        .child(label.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(MONO)
                        .text_size(px(10.5))
                        .text_color(colors::busy())
                        .child(elapsed(*secs)),
                )
                .with_animation(
                    ElementId::Name(SharedString::from(format!("sub-{thread}-{key}"))),
                    Animation::new(Duration::from_millis(220)).with_easing(crate::slide::ease_out),
                    |d, t| d.opacity(t),
                )
        }))
        .when(more > 0, |d| {
            d.child(
                div()
                    .h(px(16.))
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(format!("{more} more")),
            )
        })
        .into_any_element()
}

impl Root {
    pub(crate) fn thread_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let busy = matches!(status, Status::Working | Status::Waiting);
        // a terminal thread hears from its hooks; a structured one reads its own transcript
        let (running, doing, subs) = match self.views.get(&id) {
            Some(View::Structured(v)) => {
                let v = v.read(cx);
                (v.elapsed(), v.doing(), v.subagents())
            }
            _ => {
                let a = self.activity.get(&id);
                let subs = a
                    .map(|a| {
                        a.subs
                            .iter()
                            .map(|s| {
                                let secs = now.saturating_sub(s.started) / 1000;
                                (s.id.clone(), s.label.clone(), secs)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (
                    self.turns
                        .get(&id)
                        .filter(|_| busy)
                        .map(|t| t.elapsed().as_secs()),
                    a.and_then(|a| a.doing.clone()),
                    subs,
                )
            }
        };
        // what it concluded stays until the next turn, unless it was already seen
        let doing = doing.filter(|_| busy || self.unseen.contains(&id));
        let badge = match t.agent() {
            Some(launch) => {
                mark(launch.agent, 13., colors::brand(launch.agent).0).into_any_element()
            }
            None => icon("terminal", 12., colors::text3()).into_any_element(),
        };
        let detail = match &t.kind {
            ThreadKind::Structured { launch } => self.model_label(launch),
            ThreadKind::Terminal {
                run: Some(launch), ..
            } => self.model_label(launch),
            ThreadKind::Terminal { run: None, .. } => "Terminal".into(),
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
                .text_color(colors::text1())
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
            .flex_col()
            .gap(px(2.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(10.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .child(
                div()
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .child(div().flex_1().min_w_0().truncate().child(detail))
                    .child(div().flex_none().pl(px(8.)).child(right)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h(px(20.))
                    .text_size(px(13.5))
                    .children(glyph(status, self.unseen.contains(&id), id))
                    .child(badge)
                    .child(title),
            )
            .children(doing.map(|doing| {
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.5))
                    .text_color(if status == Status::Waiting {
                        colors::text2()
                    } else {
                        colors::text3()
                    })
                    .child(doing)
            }))
            .when(!subs.is_empty(), |d| d.child(subagents(id, &subs)))
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
