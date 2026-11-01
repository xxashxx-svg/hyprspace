// A delegated thread in its parent's transcript: one card per `delegate` call, kept out of the
// run's fold. It names the agent and model, then says what the child is doing, the first line of
// its answer, or why it failed, and opens the child on a click.

use std::f32::consts::TAU;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, Transformation, WeakEntity, div, prelude::*,
    radians,
};
use hyprspace_proto::{Agent, Tool};

use super::model::Item;
use super::render::took;
use super::{TranscriptEvent, TranscriptView};
use crate::assets::{icon, mark};
use crate::colors;
use crate::pace::Looping;
use crate::zoom::px;

/// A thread this one delegated to, and its view, read for what it is doing now.
#[derive(Clone, PartialEq)]
pub struct Child {
    pub id: u64,
    pub title: String,
    pub view: Option<WeakEntity<TranscriptView>>,
}

pub fn is(tool: &Tool) -> bool {
    matches!(tool, Tool::Mcp { server, tool, .. } if server == "hyprspace" && tool == "delegate")
}

pub fn item(item: &Item) -> bool {
    matches!(item, Item::Tool { tool, .. } if is(tool))
}

struct Ask {
    agent: Option<Agent>,
    model: Option<String>,
    /// The child's title, made the way the thread was named.
    title: String,
}

fn ask(input: &str) -> Ask {
    let v: serde_json::Value = serde_json::from_str(input).unwrap_or_default();
    let s = |k: &str| {
        v[k].as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    let agent = match s("agent").as_deref() {
        Some("claude") => Some(Agent::Claude),
        Some("codex") => Some(Agent::Codex),
        _ => None,
    };
    let title = s("title").unwrap_or_else(|| {
        let task = s("task").unwrap_or_default();
        task.lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(60)
            .collect()
    });
    Ask {
        agent,
        model: s("model"),
        title,
    }
}

fn input_of(item: &Item) -> Option<&str> {
    match item {
        Item::Tool {
            tool: tool @ Tool::Mcp { input, .. },
            ..
        } if is(tool) => Some(input),
        _ => None,
    }
}

/// The first line of an answer, without its Markdown.
fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    line.trim_start_matches(['#', '>', '-', '*', ' '])
        .replace("**", "")
        .replace('`', "")
}

enum State {
    Working,
    Done,
    Failed,
    Stopped,
}

fn badge(name: &'static str, color: Hsla) -> AnyElement {
    div()
        .absolute()
        .right(px(-4.))
        .bottom(px(-4.))
        .size(px(12.))
        .rounded_full()
        .bg(colors::bg())
        .flex()
        .items_center()
        .justify_center()
        .child(icon(name, 11., color))
        .into_any_element()
}

pub fn card(
    v: &TranscriptView,
    ix: usize,
    id: &str,
    input: &str,
    done: Option<&(bool, String)>,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let a = ask(input);
    // two calls can ask for the same title; they map to the children of that title in order
    let nth = v.model.items[..ix]
        .iter()
        .filter_map(input_of)
        .filter(|i| ask(i).title == a.title)
        .count();
    let child = v
        .delegates
        .iter()
        .filter(|c| c.title == a.title)
        .nth(nth)
        .cloned();
    let state = match done {
        None if v.model.running() => State::Working,
        None => State::Stopped,
        Some((true, _)) => State::Done,
        Some((false, _)) => State::Failed,
    };
    let line = match (&state, done) {
        (State::Working, _) => child
            .as_ref()
            .and_then(|c| c.view.as_ref()?.upgrade())
            .and_then(|view| view.read(cx).doing())
            .unwrap_or_else(|| "Starting".into()),
        (State::Stopped, _) => "Stopped before it answered".into(),
        (_, Some((_, out))) => first_line(out),
        (_, None) => String::new(),
    };
    let secs = v
        .model
        .timing(id)
        .map(|(since, took)| took.unwrap_or_else(|| since.elapsed()).as_secs());
    let meta = match (a.agent, a.model) {
        (Some(agent), Some(model)) => format!("{} \u{b7} {model}", agent.name()),
        (Some(agent), None) => agent.name().to_string(),
        (None, model) => model.unwrap_or_default(),
    };
    let face = match a.agent {
        Some(agent) => mark(agent, 15., colors::brand(agent).0).into_any_element(),
        None => icon("bot", 15., colors::text2()).into_any_element(),
    };
    let ring = matches!(state, State::Working).then(|| {
        div()
            .absolute()
            .top(px(-3.))
            .left(px(-3.))
            .child(icon("spin", 28., colors::text1()).looping(800, |s, t| {
                s.with_transformation(Transformation::rotate(radians(t * TAU)))
            }))
            .into_any_element()
    });
    let badge = match state {
        State::Done => Some(badge("check", colors::ok())),
        State::Failed => Some(badge("x", colors::error())),
        State::Working | State::Stopped => None,
    };
    let failed = matches!(state, State::Failed);
    div()
        .id(("delegate", ix))
        .flex()
        .items_start()
        .gap(px(11.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(12.))
        .bg(colors::ink(0.035))
        .border_1()
        .border_color(colors::ink(0.06))
        .child(
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(22.))
                .mt(px(1.))
                .child(face)
                .children(ring)
                .children(badge),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .min_w_0()
                        .text_size(px(13.))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child(a.title),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(12.))
                                .text_color(colors::text3())
                                .child(meta),
                        )
                        .child(div().flex_1())
                        .children(secs.filter(|s| *s > 0).map(|s| {
                            div()
                                .flex_none()
                                .text_size(px(12.))
                                .text_color(colors::text3())
                                .child(took(s))
                        })),
                )
                .when(!line.is_empty(), |d| {
                    d.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(if failed {
                                colors::error()
                            } else {
                                colors::text2()
                            })
                            .child(line),
                    )
                }),
        )
        .when_some(child, |d, child| {
            let target = child.id;
            d.cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.055)))
                .child(div().flex_none().mt(px(3.)).child(icon(
                    "chevron-right",
                    14.,
                    colors::text3(),
                )))
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    cx.emit(TranscriptEvent::GoTo(target))
                }))
        })
        .into_any_element()
}
