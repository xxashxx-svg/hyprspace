// The corner of the window: a small chip that says updates are out, how they're going and how
// they went, for HyprSpace and the agents alike. A click grows it into a list of what changes,
// old version to new, with the one button that updates them all; a click anywhere else folds it
// back. What's new after an update is its own card above it.

use gpui::{
    AnyElement, App, ClickEvent, Div, Entity, FontWeight, MouseButton, Stateful, Transformation,
    div, percentage, prelude::*,
};
use hyprspace_proto::Agent;
use hyprspace_proto::agent_update::Outcome;
use hyprspace_theme::MONO;

use super::{Phase, Swipe, Updater, VERSION, names, running, step_text};
use crate::assets::{icon, logo, mark};
use crate::pace::Looping;
use crate::slide::ease_in;
use crate::zoom::px;
use crate::{colors, widgets};

const CARD: f32 = 270.;

/// What the chip says.
#[derive(Clone, PartialEq)]
enum Chip {
    /// How many updates are out.
    Out(usize),
    /// What's running, and how many of the batch are done.
    Busy(String, Option<(usize, usize)>),
    Done {
        failed: bool,
    },
    Failed,
}

impl Chip {
    fn key(&self) -> usize {
        match self {
            Chip::Out(_) => 0,
            Chip::Busy(..) => 1,
            Chip::Done { .. } => 2,
            Chip::Failed => 3,
        }
    }
}

fn chip_of(u: &Updater) -> Option<Chip> {
    if let Phase::Busy(step) = u.phase {
        return Some(Chip::Busy(step_text(step), None));
    }
    if !u.agents.running.is_empty() {
        return Some(Chip::Busy(
            format!("Updating {}", names(&u.agents.running)),
            u.tally(),
        ));
    }
    let done = u.agents.announced();
    if !done.is_empty() {
        let failed = done
            .iter()
            .any(|(_, o)| matches!(o, Outcome::Failed { .. }));
        return Some(Chip::Done { failed });
    }
    let app = !u.dismissed && matches!(u.phase, Phase::Available(_));
    if !u.dismissed && matches!(u.phase, Phase::Failed(_)) {
        return Some(Chip::Failed);
    }
    let out = usize::from(app) + u.agents.offered().len();
    (out > 0).then_some(Chip::Out(out))
}

/// The cards for the window's bottom right corner, or None when there is nothing to show.
pub fn overlay(updater: &Entity<Updater>, working: usize, cx: &App) -> Option<AnyElement> {
    let u = updater.read(cx);
    let news = u
        .whats_new
        .clone()
        .map(|notes| swipeable(updater, true, whats_new(updater, notes), cx));
    let chip = chip_of(u);
    let open = u.open && matches!(chip, Some(Chip::Out(_) | Chip::Done { .. } | Chip::Failed));
    let corner = match chip {
        Some(_) if open => Some(list(updater, working, cx)),
        Some(chip) => Some(pill(updater, chip)),
        None => None,
    };
    if news.is_none() && corner.is_none() {
        return None;
    }
    Some(
        div()
            .absolute()
            .right(px(14.))
            .bottom(px(14.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(news)
            .children(corner)
            .into_any_element(),
    )
}

fn spinner(size: f32) -> AnyElement {
    icon("ring", size, colors::text1())
        .looping(700, |s, t| {
            s.with_transformation(Transformation::rotate(percentage(t)))
        })
        .into_any_element()
}

fn count(text: String) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded_full()
        .bg(colors::ink(0.1))
        .text_size(px(10.5))
        .text_color(colors::text2())
        .child(text)
}

fn pill(updater: &Entity<Updater>, chip: Chip) -> AnyElement {
    let key = chip.key();
    let opens = !matches!(chip, Chip::Busy(..));
    let (lead, label, tail): (AnyElement, String, Option<String>) = match chip {
        Chip::Out(n) => (
            icon("download", 13., colors::text1()).into_any_element(),
            "Updates".into(),
            Some(n.to_string()),
        ),
        Chip::Busy(what, tally) => (
            spinner(12.),
            what,
            tally.map(|(done, all)| format!("{done} of {all}")),
        ),
        Chip::Done { failed: false } => (
            icon("check", 13., colors::ok()).into_any_element(),
            "Updated".into(),
            None,
        ),
        Chip::Done { failed: true } | Chip::Failed => (
            div()
                .size(px(7.))
                .rounded_full()
                .bg(colors::error())
                .into_any_element(),
            "Update failed".into(),
            None,
        ),
    };
    let open = updater.clone();
    let chip = div()
        .id("update-chip")
        .occlude()
        .flex()
        .items_center()
        .gap(px(7.))
        .h(px(30.))
        .pl(px(11.))
        .pr(px(if opens { 6. } else { 11. }))
        .rounded_full()
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.))
        .text_color(colors::text1())
        .whitespace_nowrap()
        .child(lead)
        .child(label)
        .children(tail.map(count))
        .when(opens, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(colors::surface3()))
                .on_click(move |_: &ClickEvent, _, cx| {
                    open.update(cx, |u, cx| u.set_open(true, cx))
                })
                .child(dismiss(updater))
        });
    ease_in(chip, ("update-chip-in", key), 200, |d, t| {
        d.opacity(t).mt(px(6. * (1. - t)))
    })
}

fn dismiss(updater: &Entity<Updater>) -> Stateful<Div> {
    let updater = updater.clone();
    div()
        .id("update-chip-close")
        .flex_none()
        .size(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .hover(|s| s.bg(colors::ink(0.08)))
        .child(icon("x", 11., colors::text3()))
        .on_click(move |_: &ClickEvent, _, cx| {
            cx.stop_propagation();
            updater.update(cx, |u, cx| u.dismiss_all(cx))
        })
}

/// One thing in the list: its mark, its name, and what's on the right.
fn row(lead: AnyElement, name: &str, right: AnyElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_h(px(28.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(px(12.))
        .child(div().w(px(14.)).flex().justify_center().child(lead))
        .child(div().flex_1().child(name.to_string()))
        .child(right)
}

fn versions(from: &str, to: &str) -> AnyElement {
    div()
        .flex()
        .gap(px(4.))
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(colors::text3())
        .child(from.to_string())
        .child("→")
        .child(div().text_color(colors::text1()).child(to.to_string()))
        .into_any_element()
}

fn ended(outcome: &Outcome) -> AnyElement {
    match outcome {
        Outcome::Updated { to, .. } | Outcome::Reverted { to } => div()
            .flex()
            .items_center()
            .gap(px(4.))
            .text_color(colors::ok())
            .child(icon("check", 11., colors::ok()))
            .child(div().font_family(MONO).text_size(px(11.)).child(to.clone()))
            .into_any_element(),
        Outcome::Current { version } => div()
            .font_family(MONO)
            .text_size(px(11.))
            .text_color(colors::text3())
            .child(version.clone())
            .into_any_element(),
        Outcome::Failed { .. } => div()
            .text_size(px(11.))
            .text_color(colors::error())
            .child("Couldn't update")
            .into_any_element(),
    }
}

fn why(text: String) -> Div {
    div()
        .pl(px(30.))
        .pr(px(8.))
        .pb(px(4.))
        .text_size(px(11.))
        .line_height(px(15.))
        .text_color(colors::text3())
        .child(text)
}

fn list(updater: &Entity<Updater>, working: usize, cx: &App) -> AnyElement {
    let u = updater.read(cx);
    let mut rows: Vec<AnyElement> = Vec::new();
    let app = match &u.phase {
        Phase::Available(r) => {
            rows.push(
                row(
                    logo(13.).into_any_element(),
                    "HyprSpace",
                    versions(VERSION, &r.version),
                )
                .into_any_element(),
            );
            true
        }
        Phase::Failed(m) if !u.dismissed => {
            rows.push(
                row(
                    logo(13.).into_any_element(),
                    "HyprSpace",
                    ended(&Outcome::Failed { why: m.clone() }),
                )
                .into_any_element(),
            );
            rows.push(why(m.clone()).into_any_element());
            true
        }
        _ => false,
    };
    let behind = u.agents.behind();
    for agent in [Agent::Claude, Agent::Codex] {
        let lead = mark(agent, 12., colors::brand(agent).0).into_any_element();
        if let Some((_, outcome)) = u.agents.announced().into_iter().find(|(a, _)| *a == agent) {
            rows.push(row(lead, agent.name(), ended(&outcome)).into_any_element());
            if let Outcome::Failed { why: w } = &outcome {
                rows.push(why(w.clone()).into_any_element());
            }
        } else if let Some((_, to)) = behind.iter().find(|(a, _)| *a == agent) {
            let from = u
                .agents
                .install(agent)
                .and_then(|i| i.version.clone())
                .unwrap_or_default();
            rows.push(row(lead, agent.name(), versions(&from, to)).into_any_element());
        }
    }
    let agents: Vec<Agent> = behind.iter().map(|(a, _)| *a).collect();
    let label = match (&u.phase, u.confirm) {
        (Phase::Available(_) | Phase::Failed(_), Some(_)) => Some("Update anyway"),
        (Phase::Failed(_), None) if app => Some("Retry"),
        (Phase::Available(_), None) if agents.is_empty() => Some("Restart and update"),
        (Phase::Available(_), None) => Some("Restart and update all"),
        _ if agents.len() > 1 => Some("Update all"),
        _ if agents.len() == 1 => Some("Update"),
        _ => None,
    };
    let updated = u
        .agents
        .announced()
        .iter()
        .any(|(_, o)| matches!(o, Outcome::Updated { .. }));
    let note = match u.confirm {
        Some(n) if app => Some(running(n)),
        _ if updated && label.is_none() => {
            Some("Open threads switch to the new version when they next start.".into())
        }
        _ => None,
    };
    let go = updater.clone();
    let button = label.map(|label| {
        widgets::primary("update-go", label)
            .mt(px(4.))
            .mx(px(2.))
            .h(px(28.))
            .justify_center()
            .font_weight(FontWeight::NORMAL)
            .on_click(move |_: &ClickEvent, _, cx| {
                let agents = agents.clone();
                go.update(cx, |u, cx| {
                    if app {
                        u.install(working, cx)
                    } else {
                        u.update_agents(&agents, true, cx)
                    }
                })
            })
    });
    let close = updater.clone();
    let card = div()
        .id("update-list")
        .occlude()
        .w(px(CARD))
        .overflow_hidden()
        .p(px(6.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .rounded(px(14.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_color(colors::text1())
        .children(rows)
        .children(note.map(|n| {
            div()
                .px(px(8.))
                .pt(px(4.))
                .text_size(px(11.))
                .line_height(px(15.))
                .text_color(colors::text3())
                .child(n)
        }))
        .children(button)
        .on_mouse_down_out(move |_, _, cx| close.update(cx, |u, cx| u.set_open(false, cx)));
    // grows out of the chip it replaces
    ease_in(card, "update-list-in", 240, |d, t| {
        d.max_w(px(120. + (CARD - 120.) * t))
            .max_h(px(30. + 290. * t))
            .opacity(0.4 + 0.6 * t)
    })
}

pub(super) const DISMISS: f32 = 90.;

fn swipeable(updater: &Entity<Updater>, news: bool, card: AnyElement, cx: &App) -> AnyElement {
    let dx = updater
        .read(cx)
        .swipe
        .filter(|s| s.news == news)
        .map_or(0., |s| s.dx);
    let (down, moved, up, out) = (
        updater.clone(),
        updater.clone(),
        updater.clone(),
        updater.clone(),
    );
    div()
        .relative()
        // clicks on the card stay on the card
        .occlude()
        .left(px(dx))
        .opacity(1. - (dx / (DISMISS * 2.)).min(0.7))
        .child(card)
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            down.update(cx, |u, cx| {
                u.swipe = Some(Swipe {
                    news,
                    from: e.position.x.into(),
                    dx: 0.,
                });
                cx.notify();
            })
        })
        .on_mouse_move(move |e, _, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                return;
            }
            moved.update(cx, |u, cx| {
                if let Some(s) = u.swipe.as_mut().filter(|s| s.news == news) {
                    let x: f32 = e.position.x.into();
                    s.dx = ((x - s.from) / crate::zoom::zoom()).max(0.);
                    cx.notify();
                }
            })
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |u, cx| u.end_swipe(news, cx))
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            out.update(cx, |u, cx| u.end_swipe(news, cx))
        })
        .into_any_element()
}

fn frame() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(9.))
        .py(px(8.))
        .pl(px(12.))
        .pr(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.))
        .text_color(colors::text1())
}

fn close_news(updater: &Entity<Updater>) -> AnyElement {
    let updater = updater.clone();
    div()
        .id("whats-new-close")
        .flex_none()
        .px(px(2.))
        .cursor_pointer()
        .child(icon("x", 13., colors::text3()))
        .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, |u, cx| u.close_whats_new(cx)))
        .into_any_element()
}

fn whats_new(updater: &Entity<Updater>, notes: Vec<String>) -> AnyElement {
    let body: Vec<AnyElement> = if notes.is_empty() {
        vec![div().child("Thanks for updating.").into_any_element()]
    } else {
        notes
            .into_iter()
            .map(|n| {
                div()
                    .flex()
                    .gap(px(7.))
                    .child(div().flex_none().text_color(colors::text3()).child("•"))
                    .child(div().min_w_0().child(n))
                    .into_any_element()
            })
            .collect()
    };
    frame()
        .items_start()
        .w(px(360.))
        .py(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("What's new in v{VERSION}")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .text_color(colors::text2())
                        .line_height(px(17.))
                        .children(body),
                ),
        )
        .child(close_news(updater))
        .into_any_element()
}
