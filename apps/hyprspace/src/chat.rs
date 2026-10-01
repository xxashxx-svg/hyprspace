// A structured Claude session: the prompt and the streamed reply as a transcript.

use std::path::PathBuf;

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{Context, IntoElement, Render, Task, Window, div, prelude::*, relative};
use gpui_tokio::Tokio;

use crate::claude::{self, Event};
use crate::theme;

pub struct ChatView {
    id: u64,
    prompt: String,
    model: Option<String>,
    reply: String,
    status: String,
    _turn: Task<Result<(), gpui_tokio::JoinError>>,
    _pump: Task<()>,
}

impl ChatView {
    pub fn new(id: u64, prompt: &str, cwd: PathBuf, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = mpsc::unbounded();
        let turn = Tokio::spawn(cx, claude::run(prompt.to_string(), cwd, tx));
        let pump = cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                if this.update(cx, |chat, cx| chat.apply(event, cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            id,
            prompt: prompt.to_string(),
            model: None,
            reply: String::new(),
            status: "Starting claude".into(),
            _turn: turn,
            _pump: pump,
        }
    }

    fn apply(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Init { model } => {
                self.model = Some(model);
                self.status = "Working".into();
            }
            Event::Delta(text) => self.reply.push_str(&text),
            Event::Denied(tool) => {
                self.status = format!("Denied {tool}. Approvals aren't built yet.")
            }
            Event::Done { ok, ms, text } => {
                if self.reply.is_empty() {
                    self.reply = text;
                }
                let secs = ms as f32 / 1000.0;
                self.status = if ok {
                    format!("Done in {secs:.1}s")
                } else {
                    format!("Failed after {secs:.1}s")
                };
            }
            Event::Failed(e) => self.status = e,
        }
        cx.notify();
    }
}

impl Render for ChatView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.model {
            Some(m) => format!("Claude · {m}"),
            None => "Claude".into(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .text_color(theme::text())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(title),
            )
            .child(
                div()
                    .id(("chat", self.id))
                    .flex_1()
                    .overflow_y_scroll()
                    .px_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .self_end()
                            .max_w(relative(0.8))
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(theme::surface())
                            .child(self.prompt.clone()),
                    )
                    .child(div().text_sm().child(self.reply.clone())),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(self.status.clone()),
            )
    }
}
