// One thread in the sidebar, as the Tauri app's SessionRow drew it: the agent's mark (a ring turns
// around it while it works) with the model and the age, or a running count, on the right; the
// title; then what it is doing, or its branch or folder, with a dot when it waits on you and a
// tick when it finished. Each subagent it has running gets a small card underneath. A working
// row carries a slow sheen, a waiting one a tint. Hovering shows a button that archives it.
// Click to open it, right-click for rename and archive.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, FontWeight, Hsla,
    IntoElement, MouseButton, SharedString, Transformation, div, linear_color_stop,
    linear_gradient, percentage, prelude::*, px, relative,
};
use hyprspace_proto::{Agent, Thread, ThreadKind};
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

/// A ring turning around a mark `size` across, `inset` pixels outside it.
fn ring(key: impl Into<ElementId>, size: f32, inset: f32) -> AnyElement {
    div()
        .absolute()
        .top(px(-inset))
        .left(px(-inset))
        .child(
            icon("ring", size + 2. * inset, colors::busy()).with_animation(
                key,
                Animation::new(Duration::from_millis(900)).repeat(),
                |s, t| s.with_transformation(Transformation::rotate(percentage(t))),
            ),
        )
        .into_any_element()
}

/// A band of light sweeping slowly across a working row.
fn sheen(key: impl Into<ElementId>) -> AnyElement {
    let glow = colors::busy().opacity(0.07);
    let clear = colors::busy().opacity(0.);
    let half = |from: Hsla, to: Hsla| {
        div().flex_1().h_full().bg(linear_gradient(
            90.,
            linear_color_stop(from, 0.),
            linear_color_stop(to, 1.),
        ))
    };
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(0.6))
        .flex()
        .child(half(clear, glow))
        .child(half(glow, clear))
        .with_animation(
            key,
            Animation::new(Duration::from_millis(2600)).repeat(),
            |d, t| d.left(relative(1.0 - 1.6 * t)),
        )
        .into_any_element()
}

/// Subagents shown before the rest fold into a count.
const SUBS_SHOWN: usize = 4;

/// The subagents under a row: a small card each with its task and how long it has run. A new one
/// fades in rather than popping.
fn subagents(thread: u64, agent: Option<Agent>, subs: &[(String, String, u64)]) -> AnyElement {
    let more = subs.len().saturating_sub(SUBS_SHOWN);
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .mt(px(6.))
        .children(
            subs.iter()
                .take(SUBS_SHOWN)
                .enumerate()
                .map(|(i, (key, label, secs))| {
                    let name = |what: &str| -> ElementId {
                        ElementId::Name(SharedString::from(format!("{what}-{thread}-{key}")))
                    };
                    let badge = match agent {
                        Some(a) => mark(a, 10., colors::brand(a).0).into_any_element(),
                        None => icon("bot", 10., colors::text3()).into_any_element(),
                    };
                    div()
                        .relative()
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .h(px(26.))
                        .pl(px(5.))
                        .pr(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors::border1())
                        .bg(colors::surface2().opacity(0.7))
                        .text_size(px(11.))
                        .text_color(colors::text2())
                        .child(sheen(name("sub-sheen")))
                        .child(
                            div()
                                .relative()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(16.))
                                .rounded_full()
                                .bg(colors::surface3())
                                .child(badge)
                                .child(ring(("sub-ring", thread * 100 + i as u64), 16., 2.)),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                        .child(
                            div()
                                .flex_none()
                                .font_family(MONO)
                                .text_size(px(10.))
                                .text_color(colors::text3())
                                .child(elapsed(*secs)),
                        )
                        .map(|card| {
                            crate::slide::ease_in(card, name("sub"), 220, |d, t| d.opacity(t))
                        })
                }),
        )
        .when(more > 0, |d| {
            d.child(
                div()
                    .pl(px(6.))
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(format!("{more} more")),
            )
        })
        .into_any_element()
}

impl Root {
    /// A small icon button on a row's hover strip.
    fn row_button(
        &self,
        id: u64,
        key: &'static str,
        glyph: &'static str,
        cx: &mut Context<Self>,
        run: fn(&mut Root, u64, &ClickEvent, &mut gpui::Window, &mut Context<Root>),
    ) -> AnyElement {
        div()
            .id((key, id))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(px(5.))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.1)))
            .child(icon(glyph, 12., colors::text2()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |r, e: &ClickEvent, window, cx| {
                cx.stop_propagation();
                r.menu = None;
                run(r, id, e, window, cx)
            }))
            .into_any_element()
    }

    pub(crate) fn thread_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let working = status == Status::Working;
        let waiting = status == Status::Waiting;
        let busy = working || waiting;
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
        // the line says what it does, or what it concluded until the next turn
        let doing = doing.filter(|_| status != Status::Idle);
        let agent = t.agent().map(|l| l.agent);
        let badge = match agent {
            Some(a) => mark(a, 13., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 12., colors::text3()).into_any_element(),
        };
        let label = match t.agent() {
            Some(launch) => {
                let name = self.model_label(launch);
                match launch.model.as_deref() {
                    Some(m) if crate::models::is_long(m) => format!("{name} (1M context)"),
                    _ => name,
                }
            }
            None => "Terminal".into(),
        };
        let cwd = match &t.kind {
            ThreadKind::Terminal { cwd, .. } => cwd.clone(),
            ThreadKind::Structured { launch } => launch.cwd.clone(),
        };
        // a branch when the folder is a repo, otherwise the folder; the icon says which
        let place = match self.git.get(&cwd).map(|g| &g.branch) {
            Some(b) if b.is_repo && !b.branch.is_empty() => ("git-branch", b.branch.clone()),
            _ => (
                "folder",
                cwd.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            ),
        };
        let title: AnyElement = match &self.rename {
            Some((Rename::Thread(r), input, _)) if *r == id => div()
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
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                // settled work steps back
                .text_color(if t.settled {
                    colors::text2()
                } else {
                    colors::text1()
                })
                .child(t.title.clone())
                .into_any_element(),
        };
        let group: SharedString = format!("row-{id}").into();
        let right = match (running, t.snooze) {
            // a snoozed thread says when it comes back
            (_, Some(snooze)) => Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .child(icon("clock", 10., colors::text3()))
                    .child(self.wake_text(snooze)),
            ),
            (Some(secs), None) => Some(
                div()
                    .font_family(MONO)
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::busy())
                    .child(elapsed(secs)),
            ),
            (None, None) => (t.last_touch() > 0).then(|| {
                div()
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .child(ago(t.last_touch(), now))
            }),
        }
        .map(|d| d.flex_none().group_hover(group.clone(), |s| s.opacity(0.)));
        let state = match status {
            Status::Waiting => Some(
                widgets::status_dot(status)
                    .with_animation(
                        ("waiting", id),
                        Animation::new(Duration::from_millis(1000)).repeat(),
                        |d, t| d.opacity(0.35 + 0.65 * (t * 2.0 - 1.0).abs()),
                    )
                    .into_any_element(),
            ),
            Status::Done => Some(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(12.))
                    .rounded_full()
                    .bg(colors::ok())
                    .child(icon("check", 9., colors::on_accent()))
                    .into_any_element(),
            ),
            Status::Failed => Some(widgets::status_dot(status).into_any_element()),
            Status::Working | Status::Idle => None,
        };
        let foot = match doing {
            Some(doing) => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if waiting {
                    colors::waiting()
                } else {
                    colors::text2()
                })
                .child(doing),
            None => div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(icon(place.0, 11., colors::text3()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(MONO)
                        .text_size(px(10.5))
                        .child(place.1),
                ),
        };
        let mut menu: MenuItems = vec![
            ("Open beside".into(), Action::OpenBeside(id)),
            ("Rename".into(), Action::Rename(Rename::Thread(id))),
        ];
        if t.snooze.is_some() {
            menu.push(("Wake now".into(), Action::Wake(id)));
        } else if !t.settled {
            menu.push(("Snooze".into(), Action::Snooze(id)));
        }
        menu.push(if t.settled {
            ("Un-settle".into(), Action::Settle(id, false))
        } else {
            ("Settle".into(), Action::Settle(id, true))
        });
        menu.push(("Remove".into(), Action::RemoveThread(id)));
        // the buttons that show on hover: snooze and settle, or the one that undoes either
        let buttons: Vec<AnyElement> = if t.snooze.is_some() {
            vec![
                self.row_button(id, "wake", "sun", cx, |r, id, _, window, cx| {
                    r.snooze(id, None, window, cx)
                }),
            ]
        } else if t.settled {
            vec![
                self.row_button(id, "unsettle", "rotate-ccw", cx, |r, id, _, window, cx| {
                    r.settle(id, false, window, cx)
                }),
            ]
        } else {
            vec![
                self.row_button(id, "snooze", "clock", cx, |r, id, e, _, cx| {
                    r.open_snooze_menu(e.position(), id, cx)
                }),
                self.row_button(id, "settle", "circle-check", cx, |r, id, _, window, cx| {
                    r.settle(id, true, window, cx)
                }),
            ]
        };
        div()
            .id(("thread", id))
            .group(group.clone())
            .relative()
            .overflow_hidden()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(3.))
            .pt(px(8.))
            .px(px(8.))
            .pb(px(9.))
            .rounded(px(8.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected && waiting, |d| {
                d.bg(colors::waiting().opacity(0.07))
            })
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .when(working, |d| d.child(sheen(("row-sheen", id))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(16.))
                            .child(badge)
                            .when(working, |d| d.child(ring(("row-ring", id), 16., 3.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .child(label),
                    )
                    .children(right),
            )
            .child(title)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(foot)
                    .children(state),
            )
            .when(!subs.is_empty(), |d| d.child(subagents(id, agent, &subs)))
            .child(
                div()
                    .absolute()
                    .top(px(6.))
                    .right(px(6.))
                    .flex()
                    .gap(px(2.))
                    .p(px(1.))
                    .rounded(px(6.))
                    .bg(colors::surface3())
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .children(buttons),
            )
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
