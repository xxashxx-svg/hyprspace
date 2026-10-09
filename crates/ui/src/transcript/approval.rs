// An approval prompt in the transcript: what the agent wants to run, why, the diff or command,
// and the buttons. Once answered or expired it stays as a record of what happened.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, IntoElement, div, prelude::*, px};
use hyprspace_proto::{Answer, Tool};

use super::{TranscriptView, tool};
use crate::assets::icon;
use crate::{colors, widgets};

#[allow(clippy::too_many_arguments)]
pub fn card(
    v: &TranscriptView,
    request: &str,
    t: &Tool,
    reason: Option<&str>,
    always: bool,
    answer: Option<Answer>,
    expired: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    match super::ask::named(t) {
        Some((super::ask::QUESTION, input)) => {
            return super::ask::question_card(v, request, input, answer, expired, cx);
        }
        Some((super::ask::PLAN, input)) => {
            return super::ask::plan_card(v, Some(request), input, answer, expired, 0, cx);
        }
        _ => {}
    }
    let pending = answer.is_none() && !expired;
    let detail = match t {
        Tool::Edit { changes } => Some(tool::diffs(changes)),
        _ => tool::input(t).map(|i| tool::mono(&i)),
    };
    let (mark, tint) = match (answer, expired) {
        (None, false) => ("hand", colors::waiting()),
        (Some(Answer::Deny), _) => ("x", colors::error()),
        (Some(_), _) => ("check", colors::ok()),
        (None, true) => ("clock", colors::text3()),
    };
    let state = match (answer, expired) {
        (Some(Answer::Allow), _) => Some("Allowed"),
        (Some(Answer::AllowAlways), _) => Some("Allowed for this session"),
        (Some(Answer::Deny), _) => Some("Denied"),
        (None, true) => Some("Not answered"),
        (None, false) => None,
    };
    let buttons = pending.then(|| {
        let button = |id: &'static str, label: &'static str, a: Answer, main: bool| {
            let request = request.to_string();
            let b = if main {
                widgets::primary((id, v.id.0), label)
            } else {
                widgets::button((id, v.id.0), label).h(px(26.)).px(px(10.))
            };
            b.on_click(
                cx.listener(move |v, _: &ClickEvent, _, cx| v.answer(request.clone(), a, cx)),
            )
        };
        div()
            .flex()
            .gap_2()
            .child(button("allow", "Allow", Answer::Allow, true))
            .when(always, |d| {
                d.child(button("always", "Always allow", Answer::AllowAlways, false))
            })
            .child(button("deny", "Deny", Answer::Deny, false))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(12.))
        .bg(if pending {
            colors::waiting().opacity(0.08)
        } else {
            colors::ink(0.035)
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon(mark, 14., tint))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(tool::label(t)),
                )
                .children(state.map(|s| {
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(s)
                })),
        )
        .children(reason.map(|r| {
            div()
                .text_size(px(12.5))
                .text_color(colors::text2())
                .child(r.to_string())
        }))
        .children(detail)
        .children(buttons)
        .into_any_element()
}
