// A delegated thread's tie to the thread that sent it: who asked, in place of its first message,
// and while the sender waits for the answer, a bar in place of the composer, since what is typed
// here would not reach the sender.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, MouseButton, div, prelude::*, relative};
use hyprspace_proto::Agent;

use super::render::took;
use super::{TranscriptEvent, TranscriptView};
use crate::assets::{icon, mark};
use crate::zoom::px;
use crate::{colors, spinner, widgets};

/// The thread that delegated this one.
#[derive(Clone, PartialEq)]
pub struct Sender {
    pub id: u64,
    pub title: String,
    pub agent: Option<Agent>,
    /// Still waiting for this thread's answer.
    pub waiting: bool,
}

fn face(agent: Option<Agent>, size: f32) -> AnyElement {
    match agent {
        Some(a) => mark(a, size, colors::brand(a).0).into_any_element(),
        None => icon("bot", size, colors::text2()).into_any_element(),
    }
}

/// What the sender asked, where this thread's first message would be.
pub fn ask(s: &Sender, text: &str, cx: &mut Context<TranscriptView>) -> AnyElement {
    let target = s.id;
    let note = if s.waiting {
        "waits for this thread's answer"
    } else {
        "sent this"
    };
    div()
        .flex()
        .gap(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(12.))
        .bg(colors::ink(0.03))
        .border_1()
        .border_color(colors::ink(0.06))
        .text_size(px(12.5))
        .child(div().flex_none().mt(px(2.)).child(face(s.agent, 13.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .min_w_0()
                        .text_color(colors::text3())
                        .child(
                            div()
                                .id(("sender", target))
                                .flex_none()
                                .max_w(px(260.))
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .cursor_pointer()
                                .hover(|st| st.underline())
                                .child(format!("From {}", s.title))
                                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                    cx.emit(TranscriptEvent::GoTo(target))
                                })),
                        )
                        .child(format!("\u{b7} {note}")),
                )
                .child(
                    div()
                        .line_height(relative(1.55))
                        .text_color(colors::text2())
                        .child(text.trim().to_string()),
                ),
        )
        .into_any_element()
}

/// In place of the composer while the sender waits: the model, how long it has worked, a stop
/// while it runs, and the way back.
pub fn bar(v: &TranscriptView, s: &Sender, cx: &mut Context<TranscriptView>) -> AnyElement {
    let running = v.model.running();
    let target = s.id;
    let status = match v.elapsed().filter(|_| running) {
        Some(secs) => format!("Working {} \u{b7} reports back to {}", took(secs), s.title),
        None => format!("Reports back to {}", s.title),
    };
    widgets::composer_box(false)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .min_h(px(52.))
                .pl(px(16.))
                .pr(px(10.))
                .text_size(px(13.))
                .child(face(Some(v.launch.agent), 14.))
                .child(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(v.model_label()),
                )
                .when(running, |d| {
                    d.child(spinner::ring("sender-bar", colors::text3()))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(colors::text3())
                        .child(status),
                )
                .child(
                    widgets::button_frame("open-sender")
                        .rounded_full()
                        .child(icon("arrow-left", 13., colors::text2()))
                        .child(
                            div()
                                .max_w(px(160.))
                                .truncate()
                                .child(format!("Open {}", s.title)),
                        )
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(TranscriptEvent::GoTo(target))
                        })),
                )
                .when(running, |d| {
                    d.child(
                        widgets::stop("sender-stop")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.interrupt(cx))),
                    )
                }),
        )
        .into_any_element()
}
