use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, SharedString, div, prelude::*, px,
    relative,
};
use hyprspace_proto::{Answer, Tool};
use serde_json::Value;

use super::TranscriptView;
use super::model::Item;
use crate::assets::icon;
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

pub fn special(item: &Item) -> bool {
    matches!(item, Item::Tool { tool, .. } if named(tool).is_some())
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

pub fn answers(qs: &[Question], picks: &[Vec<usize>]) -> Option<Vec<(String, String)>> {
    qs.iter()
        .enumerate()
        .map(|(i, q)| {
            let chosen: Vec<&str> = picks
                .get(i)
                .into_iter()
                .flatten()
                .filter_map(|&o| q.options.get(o).map(|(l, _)| l.as_str()))
                .collect();
            (!chosen.is_empty()).then(|| (q.text.clone(), chosen.join(", ")))
        })
        .collect()
}

fn frame(pending: bool) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .px(px(16.))
        .py(px(14.))
        .rounded(px(14.))
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
    let pending = answer.is_none() && !expired;
    let picks = match v.answered.get(request).filter(|_| !pending) {
        Some(given) => qs
            .iter()
            .map(|q| {
                let said = given
                    .iter()
                    .find(|(text, _)| *text == q.text)
                    .map(|(_, a)| a.as_str())
                    .unwrap_or_default();
                q.options
                    .iter()
                    .enumerate()
                    .filter(|(_, (label, _))| said.split(", ").any(|s| s == label))
                    .map(|(i, _)| i)
                    .collect()
            })
            .collect(),
        None => v.picks.get(request).cloned().unwrap_or_default(),
    };
    let state = match (answer, expired) {
        (Some(Answer::Deny), _) => Some("Skipped"),
        (Some(_), _) => Some("Answered"),
        (None, true) => Some("Not answered"),
        (None, false) => None,
    };
    let mut card = frame(pending).child(heading(
        "circle-alert",
        if pending {
            colors::waiting()
        } else {
            colors::text3()
        },
        if qs.len() > 1 {
            "Claude has a few questions"
        } else {
            "Claude has a question"
        },
        state,
    ));
    for (qi, q) in qs.iter().enumerate() {
        let chosen = picks.get(qi).cloned().unwrap_or_default();
        let mut block = div().flex().flex_col().gap(px(8.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .when(!q.header.is_empty(), |d| {
                    d.child(
                        div()
                            .px(px(7.))
                            .py(px(1.))
                            .rounded_full()
                            .bg(colors::ink(0.07))
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text2())
                            .child(SharedString::from(q.header.clone())),
                    )
                })
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(colors::text1())
                        .child(SharedString::from(q.text.clone())),
                ),
        );
        for (oi, (label, about)) in q.options.iter().enumerate() {
            let on = chosen.contains(&oi);
            let request = request.to_string();
            let multi = q.multi;
            block = block.child(
                div()
                    .id(SharedString::from(format!("q-{request}-{qi}-{oi}")))
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(9.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if on {
                        colors::accent().opacity(0.6)
                    } else {
                        colors::border1()
                    })
                    .bg(if on {
                        colors::accent().opacity(0.1)
                    } else {
                        gpui::transparent_black()
                    })
                    .when(pending, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(colors::ink(0.05)))
                            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                                v.pick_option(&request, qi, oi, multi, cx)
                            }))
                    })
                    .child(
                        div()
                            .mt(px(2.))
                            .size(px(14.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(if multi { 4. } else { 7. }))
                            .border_1()
                            .border_color(if on {
                                colors::accent()
                            } else {
                                colors::text3()
                            })
                            .when(on, |d| {
                                d.bg(colors::accent()).child(icon(
                                    "check",
                                    10.,
                                    colors::on_accent(),
                                ))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors::text1())
                                    .child(SharedString::from(label.clone())),
                            )
                            .when(!about.is_empty(), |d| {
                                d.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(colors::text3())
                                        .child(SharedString::from(about.clone())),
                                )
                            }),
                    ),
            );
        }
        card = card.child(block);
    }
    if pending {
        let ready = answers(&qs, &picks);
        let send = {
            let request = request.to_string();
            widgets::primary(("q-answer", v.id.0), "Answer")
                .when(ready.is_none(), |d| d.opacity(0.5))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    if let Some(a) = ready.clone() {
                        v.answer_with(request.clone(), Answer::Allow, a, cx);
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
        card = card.child(div().flex().gap_2().child(send).child(skip));
    }
    card.into_any_element()
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
                .rounded(px(10.))
                .bg(colors::ink(0.035))
                .text_size(px(13.5))
                .line_height(relative(1.6))
                .text_color(colors::text1())
                .child(markdown::render(&blocks, &format!("plan{ix}"))),
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
        assert_eq!(answers(&qs, &[vec![1]]), None);
        assert_eq!(
            answers(&qs, &[vec![1], vec![0, 1]]),
            Some(vec![
                ("Which db?".into(), "SQLite".into()),
                ("Extras?".into(), "Auth, Logs".into())
            ])
        );
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
        }];
        assert_eq!(progress(&items), Some((1, 3, "Fixing the bug".into())));
    }
}
