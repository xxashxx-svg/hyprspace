// The transcript of a structured session: the prompt and the streamed reply.

use std::path::PathBuf;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, relative};
use hyprspace_proto::{Client, Command, RunEvent, SessionId};

use crate::colors;

pub struct TranscriptView {
    id: SessionId,
    prompt: String,
    model: Option<String>,
    reply: String,
    status: String,
}

impl TranscriptView {
    pub fn new(id: SessionId, prompt: &str, cwd: PathBuf, client: &Client) -> Self {
        client.send(Command::OpenStructured {
            id,
            cwd,
            prompt: prompt.to_string(),
        });
        Self {
            id,
            prompt: prompt.to_string(),
            model: None,
            reply: String::new(),
            status: "Starting claude".into(),
        }
    }

    pub fn apply(&mut self, event: RunEvent, cx: &mut Context<Self>) {
        match event {
            RunEvent::Started { model } => {
                self.model = Some(model);
                self.status = "Working".into();
            }
            RunEvent::Text { text } => self.reply.push_str(&text),
            RunEvent::Denied { tool } => {
                self.status = format!("Denied {tool}. Approvals aren't built yet.")
            }
            RunEvent::Finished { ok, ms, text } => {
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
            RunEvent::Failed { message } => self.status = message,
        }
        cx.notify();
    }

    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.status = message;
        cx.notify();
    }
}

impl Render for TranscriptView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.model {
            Some(m) => format!("Claude · {m}"),
            None => "Claude".into(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::bg())
            .text_color(colors::text())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(colors::muted())
                    .child(title),
            )
            .child(
                div()
                    .id(("transcript", self.id.0))
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
                            .bg(colors::surface())
                            .child(self.prompt.clone()),
                    )
                    .child(div().text_sm().child(self.reply.clone())),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(colors::muted())
                    .child(self.status.clone()),
            )
    }
}
