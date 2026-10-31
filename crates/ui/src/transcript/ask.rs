use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, Focusable, FontWeight, IntoElement,
    MouseButton, SharedString, Subscription, Window, div, prelude::*, relative,
};
use hyprspace_proto::{Answer, Tool};
use serde_json::Value;

use super::TranscriptView;
use super::model::Item;
use crate::assets::icon;
use crate::input::{InputEvent, TextInput};
use crate::zoom::px;
use crate::{colors, markdown, widgets};

pub const QUESTION: &str = "AskUserQuestion";
pub const PLAN: &str = "ExitPlanMode";
pub const TODO: &str = "TodoWrite";

pub fn named(tool: &Tool) -> Option<(&str, &str)> {
    match tool {
        Tool::Other { name, input } if [QUESTION, PLAN, TODO].contains(&name.as_str()) => {
            Some((name.as_str(), input.as_str()))
        }
        _ => None,
    }
}

pub fn pending(v: &TranscriptView) -> Option<(&str, &str)> {
    v.model.items.iter().find_map(|i| match i {
        Item::Approval {
            request,
            tool,
            answer: None,
            expired: false,
            ..
        } if !v.held.contains(request) => match named(tool) {
            Some((QUESTION, input)) => Some((request.as_str(), input)),
            _ => None,
        },
        _ => None,
    })
}

pub fn special(item: &Item) -> bool {
    matches!(item, Item::Tool { tool, .. } if named(tool).is_some())
}

/// A question call whose questions also came as an approval in the same exchange: the card is
/// the record of it, and the call's row would only repeat it.
pub fn carded(all: &[Item], ix: usize) -> bool {
    let question = |i: &Item| match i {
        Item::Tool { tool, .. } | Item::Approval { tool, .. } => {
            named(tool).is_some_and(|(n, _)| n == QUESTION)
        }
        _ => false,
    };
    if !all
        .get(ix)
        .is_some_and(|i| matches!(i, Item::Tool { .. }) && question(i))
    {
        return false;
    }
    let user = |i: &Item| matches!(i, Item::User { .. });
    let from = all[..ix].iter().rposition(user).map_or(0, |u| u + 1);
    let to = all[ix..]
        .iter()
        .position(user)
        .map_or(all.len(), |u| ix + u);
    all[from..to]
        .iter()
        .any(|i| matches!(i, Item::Approval { .. }) && question(i))
}

pub struct Question {
    pub header: String,
    pub text: String,
    pub options: Vec<(String, String)>,
    pub multi: bool,
}

pub fn questions(input: &str) -> Vec<Question> {
    let v: Value = serde_json::from_str(input).unwrap_or_default();
    v["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| Question {
            header: q["header"].as_str().unwrap_or_default().to_string(),
            text: q["question"].as_str().unwrap_or_default().to_string(),
            options: q["options"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|o| {
                    (
                        o["label"].as_str().unwrap_or_default().to_string(),
                        o["description"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect(),
            multi: q["multiSelect"].as_bool().unwrap_or(false),
        })
        .collect()
}

pub fn plan(input: &str) -> String {
    let v: Value = serde_json::from_str(input).unwrap_or_default();
    v["plan"].as_str().unwrap_or_default().to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Pending,
    Doing,
    Done,
}

pub struct Todo {
    pub text: String,
    pub doing: String,
    pub step: Step,
}

pub fn todos(input: &str) -> Vec<Todo> {
    let v: Value = serde_json::from_str(input).unwrap_or_default();
    v["todos"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| Todo {
            text: t["content"].as_str().unwrap_or_default().to_string(),
            doing: t["activeForm"].as_str().unwrap_or_default().to_string(),
            step: match t["status"].as_str() {
                Some("completed") => Step::Done,
                Some("in_progress") => Step::Doing,
                _ => Step::Pending,
            },
        })
        .collect()
}

/// The text field of a question's Other choice, open on one question at a time.
pub struct Other {
    request: String,
    question: usize,
    input: Entity<TextInput>,
    _sub: Subscription,
}

impl Other {
    pub fn is_for(&self, request: &str) -> bool {
        self.request == request
    }
}

/// What one question's picks say: the labels picked, and the typed answer when Other, one past
/// the last option, is picked too.
fn said(q: &Question, picked: &[usize], other: &str) -> String {
    let mut parts: Vec<&str> = picked
        .iter()
        .filter_map(|&o| q.options.get(o).map(|(l, _)| l.as_str()))
        .collect();
    if picked.contains(&q.options.len()) && !other.trim().is_empty() {
        parts.push(other.trim());
    }
    parts.join(", ")
}

/// Each question with what its picks say so far, answered or not.
pub fn so_far(qs: &[Question], picks: &[Vec<usize>], others: &[String]) -> Vec<(String, String)> {
    qs.iter()
        .enumerate()
        .map(|(i, q)| {
            let picked = picks.get(i).map_or(&[][..], |p| p.as_slice());
            let other = others.get(i).map_or("", |o| o.as_str());
            (q.text.clone(), said(q, picked, other))
        })
        .collect()
}

/// The answers to send, once every question has one.
pub fn answers(
    qs: &[Question],
    picks: &[Vec<usize>],
    others: &[String],
) -> Option<Vec<(String, String)>> {
    so_far(qs, picks, others)
        .into_iter()
        .map(|(q, a)| (!a.is_empty()).then_some((q, a)))
        .collect()
}

impl TranscriptView {
    /// Picks a question's Other choice and opens its text field.
    pub(super) fn pick_other(
        &mut self,
        request: &str,
        question: usize,
        ix: usize,
        multi: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was = self
            .picks
            .get(request)
            .and_then(|p| p.get(question))
            .is_some_and(|c| c.contains(&ix));
        self.pick_option(request, question, ix, multi, cx);
        if multi && was {
            self.other = None;
            return;
        }
        let open = self
            .other
            .as_ref()
            .filter(|o| o.request == request && o.question == question)
            .map(|o| o.input.clone());
        let input = open.unwrap_or_else(|| {
            let text = self
                .others
                .get(request)
                .and_then(|o| o.get(question))
                .cloned()
                .unwrap_or_default();
            let input = cx.new(|cx| {
                let mut i = TextInput::new("Your answer", false, cx);
                i.set_text(text, cx);
                i
            });
            let key = request.to_string();
            let sub = cx.subscribe(&input, move |v, input, e: &InputEvent, cx| match e {
                InputEvent::Changed => {
                    let text = input.read(cx).text().to_string();
                    let typed = v.others.entry(key.clone()).or_default();
                    if typed.len() <= question {
                        typed.resize(question + 1, String::new());
                    }
                    typed[question] = text;
                    cx.notify();
                }
                InputEvent::Submit => v.submit_other(cx),
                _ => {}
            });
            self.other = Some(Other {
                request: request.to_string(),
                question,
                input: input.clone(),
                _sub: sub,
            });
            input
        });
        window.focus(&input.focus_handle(cx), cx);
    }

    pub(super) fn pick_key(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((request, input)) = pending(self).map(|(r, i)| (r.to_string(), i.to_string()))
        else {
            return;
        };
        let qs = questions(&input);
        let step = self
            .steps
            .get(&request)
            .copied()
            .unwrap_or(0)
            .min(qs.len().saturating_sub(1));
        let Some(q) = qs.get(step) else {
            return;
        };
        if ix < q.options.len() {
            self.pick_option(&request, step, ix, q.multi, cx);
        } else if ix == q.options.len() {
            self.pick_other(&request, step, ix, q.multi, window, cx);
        }
    }

    /// Enter in an Other field: the next question, or the answers once the last one has one.
    fn submit_other(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.other.as_ref().map(|o| o.request.clone()) else {
            return;
        };
        let Some(input) = self.model.items.iter().find_map(|i| match i {
            Item::Approval {
                request: r,
                tool,
                answer: None,
                ..
            } if *r == request => named(tool).map(|(_, input)| input.to_string()),
            _ => None,
        }) else {
            return;
        };
        let qs = questions(&input);
        let step = self.steps.get(&request).copied().unwrap_or(0);
        let picks = self.picks.get(&request).cloned().unwrap_or_default();
        let others = self.others.get(&request).cloned().unwrap_or_default();
        if step + 1 < qs.len() {
            if picks.get(step).is_some_and(|p| !p.is_empty()) {
                self.show_question(&request, step + 1, cx);
            }
        } else if let Some(a) = answers(&qs, &picks, &others) {
            self.answer_with(request, Answer::Allow, a, cx);
        }
    }
}

fn frame(pending: bool) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .px(px(16.))
        .py(px(14.))
        .rounded(px(12.))
        .border_1()
        .border_color(if pending {
            colors::border2()
        } else {
            colors::border1()
        })
        .bg(colors::ink(0.03))
}

fn heading(glyph: &'static str, tint: gpui::Hsla, title: &str, state: Option<&str>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(icon(glyph, 14., tint))
        .child(
            div()
                .flex_1()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(SharedString::from(title.to_string())),
        )
        .children(state.map(|s| {
            div()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(SharedString::from(s.to_string()))
        }))
}

pub fn question_card(
    v: &TranscriptView,
    request: &str,
    input: &str,
    answer: Option<Answer>,
    expired: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let qs = questions(input);
    if answer.is_none() && !expired {
        let picks = v.picks.get(request).cloned().unwrap_or_default();
        let others = v.others.get(request).cloned().unwrap_or_default();
        asking(v, request, &qs, &picks, &others, cx)
    } else {
        let given = v.answered.get(request).cloned().unwrap_or_default();
        asked(&qs, &given, answer)
    }
}

fn asking(
    v: &TranscriptView,
    request: &str,
    qs: &[Question],
    picks: &[Vec<usize>],
    others: &[String],
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let step = v
        .steps
        .get(request)
        .copied()
        .unwrap_or(0)
        .min(qs.len().saturating_sub(1));
    let picked = |qi: usize| picks.get(qi).is_some_and(|p| !p.is_empty());
    let keyed = pending(v).is_some_and(|(r, _)| r == request);
    let cap =
        |ix: usize| (keyed && ix < 9).then(|| super::approval::alt_cap(&(ix + 1).to_string()));
    let title = match qs.len() {
        0 | 1 => "Claude has a question".to_string(),
        n => format!("Claude has {n} questions"),
    };
    let mut card = frame(true).child(heading("circle-help", colors::waiting(), &title, None));
    if qs.len() > 1 {
        card = card.child(div().flex().flex_wrap().gap(px(4.)).children(
            qs.iter().enumerate().map(|(qi, q)| {
                let on = qi == step;
                let request = request.to_string();
                let name = if q.header.is_empty() {
                    format!("Question {}", qi + 1)
                } else {
                    q.header.clone()
                };
                div()
                    .id(SharedString::from(format!("q-tab-{request}-{qi}")))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .h(px(24.))
                    .px(px(9.))
                    .rounded(px(6.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .text_color(if on { colors::text1() } else { colors::text3() })
                    .when(on, |d| d.bg(colors::ink(0.08)))
                    .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.05))))
                    .when(picked(qi), |d| {
                        d.child(icon("check", 11., colors::accent()))
                    })
                    .child(SharedString::from(name))
                    .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                        v.show_question(&request, qi, cx)
                    }))
            }),
        ));
    }
    let Some(q) = qs.get(step) else {
        return card.into_any_element();
    };
    let chosen = picks.get(step).cloned().unwrap_or_default();
    let last = step + 1 >= qs.len();
    card = card.child(
        div()
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(colors::text1())
                    .child(SharedString::from(q.text.clone())),
            )
            .when(q.multi, |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child("Pick one or more."),
                )
            }),
    );
    let options = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .children(q.options.iter().enumerate().map(|(oi, (label, about))| {
            let on = chosen.contains(&oi);
            let request = request.to_string();
            let multi = q.multi;
            div()
                .id(SharedString::from(format!("q-{request}-{step}-{oi}")))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(on, |d| d.bg(colors::ink(0.07)))
                .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.04))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if on { colors::text1() } else { colors::text2() })
                                .child(SharedString::from(label.clone())),
                        )
                        .when(!about.is_empty() && about != label, |d| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(colors::text3())
                                    .child(SharedString::from(about.clone())),
                            )
                        }),
                )
                .children(cap(oi))
                .child(
                    div()
                        .flex_none()
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.child(icon("check", 14., colors::accent()))),
                )
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    v.pick_option(&request, step, oi, multi, cx)
                }))
        }));
    let other_ix = q.options.len();
    let other_on = chosen.contains(&other_ix);
    let typed = others.get(step).cloned().unwrap_or_default();
    let field = v
        .other
        .as_ref()
        .filter(|o| other_on && o.request == request && o.question == step)
        .map(|o| o.input.clone());
    let other = {
        let request = request.to_string();
        let multi = q.multi;
        div()
            .id(SharedString::from(format!("q-{request}-{step}-other")))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .cursor_pointer()
            .when(other_on, |d| d.bg(colors::ink(0.07)))
            .when(!other_on, |d| d.hover(|s| s.bg(colors::ink(0.04))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if other_on {
                                colors::text1()
                            } else {
                                colors::text2()
                            })
                            .child("Other"),
                    )
                    .child(match field {
                        Some(input) => div()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .px(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(colors::border2())
                            .bg(colors::bg())
                            .text_size(px(13.))
                            .text_color(colors::text1())
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(input)
                            .into_any_element(),
                        None => div()
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child(SharedString::from(if typed.trim().is_empty() {
                                "Type your own answer".to_string()
                            } else {
                                typed.clone()
                            }))
                            .into_any_element(),
                    }),
            )
            .children(cap(other_ix))
            .child(
                div()
                    .flex_none()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(other_on, |d| d.child(icon("check", 14., colors::accent()))),
            )
            .on_click(cx.listener(move |v, _: &ClickEvent, window, cx| {
                v.pick_other(&request, step, other_ix, multi, window, cx)
            }))
    };
    card = card.child(options.child(other));
    let go = if last {
        let ready = answers(qs, picks, others);
        let request = request.to_string();
        widgets::primary(("q-answer", v.id.0), "Answer")
            .when(ready.is_none(), |d| d.opacity(0.5))
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                if let Some(a) = ready.clone() {
                    v.answer_with(request.clone(), Answer::Allow, a, cx);
                }
            }))
    } else {
        let request = request.to_string();
        let can = !chosen.is_empty();
        widgets::primary(("q-next", v.id.0), "Next")
            .when(!can, |d| d.opacity(0.5))
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                if can {
                    v.show_question(&request, step + 1, cx)
                }
            }))
    };
    let skip = {
        let request = request.to_string();
        widgets::button(("q-skip", v.id.0), "Skip")
            .h(px(26.))
            .px(px(10.))
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                v.answer_with(request.clone(), Answer::Deny, Vec::new(), cx)
            }))
    };
    // turned down with the answers so far, which the harness sends as a wish to talk it over
    let chat = {
        let request = request.to_string();
        let so_far = so_far(qs, picks, others);
        widgets::button(("q-chat", v.id.0), "Chat about this")
            .h(px(26.))
            .px(px(10.))
            .on_click(cx.listener(move |v, _: &ClickEvent, window, cx| {
                v.answer_with(request.clone(), Answer::Deny, so_far.clone(), cx);
                window.focus(&v.input.focus_handle(cx), cx);
            }))
    };
    card.child(div().flex().gap_2().child(go).child(skip).child(chat))
        .into_any_element()
}

fn asked(qs: &[Question], given: &[(String, String)], answer: Option<Answer>) -> AnyElement {
    let state = match answer {
        Some(Answer::Deny) if !given.is_empty() => "Chose to talk it over",
        Some(Answer::Deny) => "Skipped",
        Some(_) => "Answered",
        None => "Not answered",
    };
    let title = match qs.len() {
        0 | 1 => "Claude asked a question".to_string(),
        n => format!("Claude asked {n} questions"),
    };
    frame(false)
        .gap(px(10.))
        .child(heading("circle-help", colors::text3(), &title, Some(state)))
        .children(qs.iter().map(|q| {
            let said = given
                .iter()
                .find(|(text, _)| *text == q.text)
                .map(|(_, a)| a.as_str())
                .unwrap_or_default();
            let reply = if said.is_empty() {
                div()
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .child("No answer")
            } else {
                div()
                    .flex()
                    .items_start()
                    .gap(px(7.))
                    .child(div().mt(px(2.)).child(icon("check", 13., colors::accent())))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text1())
                            .child(SharedString::from(said.to_string())),
                    )
            };
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(SharedString::from(q.text.clone())),
                )
                .child(reply)
        }))
        .into_any_element()
}

pub fn plan_card(
    v: &TranscriptView,
    request: Option<&str>,
    input: &str,
    answer: Option<Answer>,
    expired: bool,
    ix: usize,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let pending = request.is_some() && answer.is_none() && !expired;
    let state = match (request, answer, expired) {
        (None, _, _) => None,
        (_, Some(Answer::Deny), _) => Some("Kept planning"),
        (_, Some(_), _) => Some("Approved"),
        (_, None, true) => Some("Not answered"),
        (_, None, false) => None,
    };
    let blocks = markdown::parse(&plan(input));
    let mut card = frame(pending)
        .child(heading("list-checks", colors::text2(), "Plan", state))
        .child(
            div()
                .px(px(14.))
                .py(px(10.))
                .rounded(px(12.))
                .bg(colors::ink(0.035))
                .text_size(px(13.5))
                .line_height(relative(1.6))
                .text_color(colors::text1())
                .child(markdown::render(&blocks, &format!("plan{ix}"), &[])),
        );
    if let (true, Some(request)) = (pending, request) {
        let approve = {
            let request = request.to_string();
            widgets::primary(("plan-yes", v.id.0), "Approve plan").on_click(
                cx.listener(move |v, _: &ClickEvent, _, cx| v.approve_plan(request.clone(), cx)),
            )
        };
        let keep = {
            let request = request.to_string();
            widgets::button(("plan-no", v.id.0), "Keep planning")
                .h(px(26.))
                .px(px(10.))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    v.answer_with(request.clone(), Answer::Deny, Vec::new(), cx)
                }))
        };
        card = card.child(div().flex().gap_2().child(approve).child(keep));
    }
    card.into_any_element()
}

pub fn todo_card(input: &str) -> AnyElement {
    let list = todos(input);
    let done = list.iter().filter(|t| t.step == Step::Done).count();
    let rows = list.iter().map(|t| {
        let (glyph, tint) = match t.step {
            Step::Done => ("circle-check", colors::ok()),
            Step::Doing => ("circle-play", colors::busy()),
            Step::Pending => ("square", colors::text3()),
        };
        div()
            .flex()
            .items_start()
            .gap(px(9.))
            .child(div().mt(px(2.)).child(icon(glyph, 13., tint)))
            .child(
                div()
                    .min_w_0()
                    .text_size(px(13.))
                    .text_color(match t.step {
                        Step::Done => colors::text3(),
                        Step::Doing => colors::text1(),
                        Step::Pending => colors::text2(),
                    })
                    .when(t.step == Step::Done, |d| d.line_through())
                    .child(SharedString::from(t.text.clone())),
            )
    });
    frame(false)
        .gap(px(9.))
        .child(heading(
            "list-checks",
            colors::text2(),
            "Tasks",
            Some(&format!("{done} of {} done", list.len())),
        ))
        .children(rows)
        .into_any_element()
}

pub fn progress(items: &[Item]) -> Option<(usize, usize, String)> {
    let input = items.iter().rev().find_map(|i| match i {
        Item::Tool {
            tool: Tool::Other { name, input },
            ..
        } if name == TODO => Some(input.as_str()),
        _ => None,
    })?;
    let list = todos(input);
    let done = list.iter().filter(|t| t.step == Step::Done).count();
    if list.is_empty() || done == list.len() {
        return None;
    }
    let now = list
        .iter()
        .find(|t| t.step == Step::Doing)
        .map(|t| {
            if t.doing.is_empty() {
                t.text.clone()
            } else {
                t.doing.clone()
            }
        })
        .unwrap_or_default();
    Some((done, list.len(), now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claudes_questions_and_answers_them_by_text() {
        let qs = questions(
            r#"{"questions":[{"question":"Which db?","header":"DB","multiSelect":false,
            "options":[{"label":"Postgres","description":"Server"},{"label":"SQLite"}]},
            {"question":"Extras?","header":"Add","multiSelect":true,
            "options":[{"label":"Auth"},{"label":"Logs"}]}]}"#,
        );
        assert_eq!(qs.len(), 2);
        assert!(qs[1].multi);
        assert_eq!(answers(&qs, &[vec![1]], &[]), None);
        assert_eq!(
            answers(&qs, &[vec![1], vec![0, 1]], &[]),
            Some(vec![
                ("Which db?".into(), "SQLite".into()),
                ("Extras?".into(), "Auth, Logs".into())
            ])
        );
    }

    #[test]
    fn other_sends_what_was_typed() {
        let qs = questions(
            r#"{"questions":[{"question":"Which db?","multiSelect":false,
            "options":[{"label":"Postgres"},{"label":"SQLite"}]},
            {"question":"Extras?","multiSelect":true,"options":[{"label":"Auth"}]}]}"#,
        );
        let typed = vec!["MySQL".to_string(), " Rate limits ".to_string()];
        // Other is one past the last option, and an empty one is no answer
        assert_eq!(answers(&qs, &[vec![2], vec![1]], &[String::new()]), None);
        assert_eq!(
            answers(&qs, &[vec![2], vec![0, 1]], &typed),
            Some(vec![
                ("Which db?".into(), "MySQL".into()),
                ("Extras?".into(), "Auth, Rate limits".into())
            ])
        );
        assert_eq!(
            so_far(&qs, &[vec![1]], &[]),
            vec![
                ("Which db?".into(), "SQLite".into()),
                ("Extras?".into(), String::new())
            ]
        );
    }

    #[test]
    fn a_question_with_a_card_has_no_row_of_its_own() {
        let q = || Tool::Other {
            name: QUESTION.into(),
            input: "{}".into(),
        };
        let call = || Item::Tool {
            id: "t".into(),
            tool: q(),
            done: None,
            open: false,
            images: Vec::new(),
            live: String::new(),
        };
        let user = || Item::User {
            text: "hi".into(),
            images: Vec::new(),
            steer: false,
            at: None,
        };
        let card = || Item::Approval {
            request: "r".into(),
            tool: q(),
            reason: None,
            always: false,
            answer: None,
            expired: false,
        };
        assert!(carded(&[user(), call(), card()], 1));
        assert!(!carded(&[user(), call(), user(), card()], 1));
        assert!(!carded(&[user(), call()], 1));
    }

    #[test]
    fn todos_report_the_step_in_progress() {
        let input = r#"{"todos":[{"content":"Read","status":"completed","activeForm":"Reading"},
            {"content":"Fix","status":"in_progress","activeForm":"Fixing the bug"},
            {"content":"Test","status":"pending","activeForm":"Testing"}]}"#;
        let items = vec![Item::Tool {
            id: "t".into(),
            tool: Tool::Other {
                name: TODO.into(),
                input: input.into(),
            },
            done: None,
            open: false,
            images: Vec::new(),
            live: String::new(),
        }];
        assert_eq!(progress(&items), Some((1, 3, "Fixing the bug".into())));
    }
}
