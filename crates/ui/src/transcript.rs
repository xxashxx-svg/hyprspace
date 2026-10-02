// The transcript of a structured session: the prompt, then the run's reply, reasoning and tool
// calls as they stream. Approval prompts come with the UI shell (docs/REWRITE.md step 4); until
// then every approval is denied, through the channel like any answer.

use std::path::PathBuf;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, relative};
use hyprspace_proto::{
    Agent, Client, Command, Launch, Prompt, RunEvent, RunStatus, SessionId, Tool,
};

use crate::colors;

enum Item {
    Text(String),
    Thinking(String),
    Tool {
        id: String,
        label: String,
        ok: Option<bool>,
    },
}

pub struct TranscriptView {
    id: SessionId,
    client: Client,
    agent: Agent,
    prompt: String,
    model: Option<String>,
    items: Vec<Item>,
    status: String,
}

impl TranscriptView {
    pub fn new(id: SessionId, agent: Agent, prompt: &str, cwd: PathBuf, client: &Client) -> Self {
        client.send(Command::OpenStructured {
            id,
            launch: Launch::new(agent, cwd),
            prompt: Some(Prompt::text(prompt)),
        });
        Self {
            id,
            client: client.clone(),
            agent,
            prompt: prompt.to_string(),
            model: None,
            items: Vec::new(),
            status: format!("Starting {}", agent.name()),
        }
    }

    pub fn apply(&mut self, event: RunEvent, cx: &mut Context<Self>) {
        match event {
            RunEvent::Started { model, .. } => {
                self.model = Some(model);
                self.status = "Working".into();
            }
            RunEvent::Text { text } => match self.items.last_mut() {
                Some(Item::Text(t)) => t.push_str(&text),
                _ => self.items.push(Item::Text(text)),
            },
            RunEvent::Thinking { text } => match self.items.last_mut() {
                Some(Item::Thinking(t)) => t.push_str(&text),
                _ => self.items.push(Item::Thinking(text)),
            },
            RunEvent::Tool { id, tool } => {
                let label = label(&tool);
                match self.tool(&id) {
                    Some(Item::Tool { label: l, .. }) => *l = label,
                    _ => self.items.push(Item::Tool {
                        id,
                        label,
                        ok: None,
                    }),
                }
            }
            RunEvent::ToolDone { id, ok, .. } => {
                if let Some(Item::Tool { ok: done, .. }) = self.tool(&id) {
                    *done = Some(ok);
                }
            }
            RunEvent::Approval { request, tool, .. } => {
                self.client.send(Command::Approve {
                    id: self.id,
                    request,
                    allow: false,
                });
                self.status = format!(
                    "Denied: {}. HyprSpace can't approve tools yet.",
                    label(&tool)
                );
            }
            RunEvent::Steered | RunEvent::Usage { .. } => {}
            RunEvent::Error { message } => self.status = message,
            RunEvent::Finished {
                status,
                ms,
                text,
                error,
            } => {
                if !self.items.iter().any(|i| matches!(i, Item::Text(_))) && !text.is_empty() {
                    self.items.push(Item::Text(text));
                }
                let secs = ms as f32 / 1000.0;
                self.status = match (status, error) {
                    (RunStatus::Done, _) => format!("Done in {secs:.1}s"),
                    (RunStatus::Interrupted, _) => format!("Stopped after {secs:.1}s"),
                    (RunStatus::Failed, Some(e)) => e,
                    (RunStatus::Failed, None) => format!("Failed after {secs:.1}s"),
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

    fn tool(&mut self, id: &str) -> Option<&mut Item> {
        self.items
            .iter_mut()
            .rev()
            .find(|i| matches!(i, Item::Tool { id: t, .. } if t == id))
    }
}

/// One line saying what a tool did.
fn label(tool: &Tool) -> String {
    match tool {
        Tool::Command { command } => format!("Ran {command}"),
        Tool::Read { path } => format!("Read {path}"),
        Tool::Edit { changes } => {
            let paths: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
            format!("Edited {}", paths.join(", "))
        }
        Tool::Search { pattern, .. } => format!("Searched for {pattern}"),
        Tool::Web { target } => format!("Looked up {target}"),
        Tool::Mcp { server, tool, .. } => format!("Used {server} {tool}"),
        Tool::Other { name, .. } => format!("Used {name}"),
    }
}

impl Render for TranscriptView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.model {
            Some(m) => format!("{} · {m}", self.agent.name()),
            None => self.agent.name().into(),
        };
        let items = self.items.iter().map(|item| match item {
            Item::Text(t) => div().text_sm().child(t.clone()),
            Item::Thinking(t) => div().text_xs().text_color(colors::muted()).child(t.clone()),
            Item::Tool { label, ok, .. } => {
                let mark = match ok {
                    None => "...",
                    Some(true) => "done",
                    Some(false) => "failed",
                };
                div()
                    .text_xs()
                    .text_color(colors::muted())
                    .child(format!("{label} ({mark})"))
            }
        });
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
                    .children(items),
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

#[cfg(test)]
mod tests {
    use hyprspace_proto::run::{ChangeKind, FileChange};

    use super::*;

    #[test]
    fn labels_say_what_happened() {
        assert_eq!(
            label(&Tool::Command {
                command: "ls".into()
            }),
            "Ran ls"
        );
        let edit = Tool::Edit {
            changes: vec![FileChange {
                path: "a.rs".into(),
                kind: ChangeKind::Update,
                diff: String::new(),
            }],
        };
        assert_eq!(label(&edit), "Edited a.rs");
    }
}
