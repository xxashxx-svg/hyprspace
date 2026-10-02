// An approval prompt in the transcript: what the agent wants to run, why, the diff or command,
// and the buttons. Once answered or expired it stays as a record of what happened.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, IntoElement, div, prelude::*};
use hyprspace_proto::{Answer, Tool};

use super::{TranscriptView, tool};
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
    let pending = answer.is_none() && !expired;
    let detail = match t {
        Tool::Edit { changes } => Some(tool::diffs(changes)),
        _ => tool::input(t).map(|i| tool::mono(&i)),
    };
    let footer: AnyElement = match (answer, expired) {
        (Some(a), _) => div()
            .text_xs()
            .text_color(colors::text2())
            .child(match a {
                Answer::Allow => "Allowed",
                Answer::AllowAlways => "Allowed for the rest of this session",
                Answer::Deny => "Denied",
            })
            .into_any_element(),
        (None, true) => div()
            .text_xs()
            .text_color(colors::text3())
            .child("No answer. The run ended first.")
            .into_any_element(),
        (None, false) => {
            let button = |id: &'static str, label: &'static str, a: Answer, main: bool| {
                let request = request.to_string();
                let b = if main {
                    widgets::primary((id, v.id.0), label)
                } else {
                    widgets::button((id, v.id.0), label)
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
                .into_any_element()
        }
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(if pending {
            colors::waiting().opacity(0.6)
        } else {
            colors::border1()
        })
        .bg(colors::surface2())
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if pending {
                            colors::waiting()
                        } else {
                            colors::text3()
                        })
                        .child("Approval"),
                )
                .child(div().flex_1().min_w_0().truncate().child(tool::label(t))),
        )
        .children(reason.map(|r| {
            div()
                .text_xs()
                .text_color(colors::text2())
                .child(r.to_string())
        }))
        .children(detail)
        .child(footer)
        .into_any_element()
}
