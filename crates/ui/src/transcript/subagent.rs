// A subagent in the transcript: one bordered card for each Agent call, titled with what the
// subagent was asked to do, with a live status and a count of what it did. Opened, it shows the
// subagent's own tool calls and then its report; the prompt it was given waits behind its own
// small disclosure. After zeron's subagent chip, drawn in our tokens.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, div, prelude::*, px, relative,
};

use super::model::{AgentState, Item, Subagent};
use super::render::{fold_head, pulse, tool_card};
use super::{TranscriptView, tool};
use crate::assets::icon;
use crate::{colors, markdown};

pub fn card(ix: usize, a: &Subagent, cx: &mut Context<TranscriptView>) -> AnyElement {
    let state: AnyElement = match a.state {
        AgentState::Working => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(pulse(("agent-live", ix), 6.))
            .child(elapsed(a.since.elapsed().as_secs()))
            .into_any_element(),
        AgentState::Done => div().child("Done").into_any_element(),
        AgentState::Failed => div()
            .text_color(colors::error())
            .child("Failed")
            .into_any_element(),
        AgentState::Stopped => div().child("Stopped").into_any_element(),
    };
    let head = div()
        .id(("agent", ix))
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .cursor_pointer()
        .child(icon(
            if a.open {
                "chevron-down"
            } else {
                "chevron-right"
            },
            12.,
            colors::text3(),
        ))
        .child(icon("bot", 14., colors::text2()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(if a.description.is_empty() {
                    "Subagent".to_string()
                } else {
                    a.description.clone()
                }),
        )
        .when(!a.agent_type.is_empty(), |d| {
            d.child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(a.agent_type.clone()),
            )
        })
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(state),
        )
        .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
            if let Some(Item::Agent(a)) = v.model.items.get_mut(ix) {
                a.open = !a.open;
            }
            cx.notify();
        }));
    // lines up under the title, past the chevron and the icon
    let indent = px(42.);
    let did = (!a.calls.is_empty()).then(|| {
        div()
            .pl(indent)
            .truncate()
            .text_size(px(12.))
            .text_color(colors::text3())
            .child(tool::summary(a.calls.iter().map(|c| &c.tool)))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border1())
        .child(head)
        .children(did)
        .when(a.open, |d| d.child(body(ix, a, cx)))
        .into_any_element()
}

fn elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

/// The opened card: the prompt behind its disclosure, the subagent's calls, then its answer.
fn body(ix: usize, a: &Subagent, cx: &mut Context<TranscriptView>) -> AnyElement {
    let prompt = (!a.prompt.trim().is_empty()).then(|| {
        let head = fold_head(
            ("agent-prompt", ix),
            a.prompt_open,
            "Prompt".into(),
            None,
            cx,
            move |v| {
                if let Some(Item::Agent(a)) = v.model.items.get_mut(ix) {
                    a.prompt_open = !a.prompt_open;
                }
            },
        );
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(head)
            .when(a.prompt_open, |d| {
                d.child(
                    div()
                        .ml(px(18.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(colors::surface2())
                        .text_size(px(12.5))
                        .line_height(relative(1.55))
                        .text_color(colors::text2())
                        .child(a.prompt.trim().to_string()),
                )
            })
    });
    // a call still open when its subagent stopped will never end
    let ended = a.state != AgentState::Working;
    let unfinished = (false, String::new());
    let calls = a.calls.iter().enumerate().map(|(j, c)| {
        let done = c.done.as_ref().or(ended.then_some(&unfinished));
        tool_card(
            format!("agent-{ix}-call-{j}").into(),
            &c.tool,
            done,
            c.open,
            cx,
            move |v| {
                if let Some(Item::Agent(a)) = v.model.items.get_mut(ix)
                    && let Some(c) = a.calls.get_mut(j)
                {
                    c.open = !c.open;
                }
            },
        )
    });
    let answer = a.blocks.as_ref().filter(|b| !b.is_empty()).map(|b| {
        div()
            .pt(px(4.))
            .text_size(px(13.5))
            .line_height(relative(1.6))
            .text_color(colors::text1())
            .child(markdown::render(b, &format!("agent{ix}")))
    });
    div()
        .pt(px(6.))
        .pl(px(20.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(prompt)
        .children(calls)
        .children(answer)
        .into_any_element()
}
