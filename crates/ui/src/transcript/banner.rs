use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*};

use super::{TranscriptEvent, TranscriptView};
use crate::assets::icon;
use crate::zoom::px;
use crate::{colors, widgets};

pub fn provider(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    let status = v.provider.as_ref().filter(|_| v.blocked())?;
    let name = v.launch.agent.name();
    let (line, label, event): (String, &str, fn() -> TranscriptEvent) = if !status.installed {
        (
            format!("{name} isn't installed on this computer, so this thread can't run."),
            "Agents settings",
            || TranscriptEvent::Settings(crate::settings::Tab::Agents),
        )
    } else if status.signed_out() {
        (
            format!("{name} is signed out, so this thread can't run."),
            "Sign in",
            || TranscriptEvent::SignIn,
        )
    } else {
        return None;
    };
    Some(
        div()
            .mb(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .py(px(10.))
            .rounded(px(12.))
            .bg(colors::ink(0.035))
            .border_1()
            .border_color(colors::waiting().opacity(0.35))
            .child(icon("circle-alert", 14., colors::waiting()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(line),
            )
            .child(
                widgets::button("provider-fix", label)
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(event()))),
            )
            .into_any_element(),
    )
}
