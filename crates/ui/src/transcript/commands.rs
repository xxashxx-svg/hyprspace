use gpui::{ClipboardItem, Context};
use hyprspace_proto::{Command, Permission, Rewind};

use super::model::Item;
use super::{TranscriptEvent, TranscriptView};
use crate::composer::model_menu::{Choice, Host as _};
use crate::suggest::Item as Suggestion;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Own {
    Model,
    Effort,
    Fast,
    Plan,
    Btw,
    Context,
    Status,
    Version,
    Copy,
    Export,
    Branch,
    Rename,
    Diff,
    Tasks,
    Todos,
    Skills,
    Resume,
    Help,
    Clear,
    Rewind,
    Login,
}

const OWN: &[(&str, Own, &str)] = &[
    ("model", Own::Model, "Pick the model, or name one"),
    ("effort", Own::Effort, "Pick the effort, or name one"),
    ("fast", Own::Fast, "Turn fast mode on or off"),
    ("plan", Own::Plan, "Plan before changing anything"),
    ("btw", Own::Btw, "Ask a side question while it works"),
    ("context", Own::Context, "How full the context window is"),
    (
        "status",
        Own::Status,
        "Model, effort, permission and context",
    ),
    (
        "version",
        Own::Version,
        "The version of the CLI running here",
    ),
    ("copy", Own::Copy, "Copy the last reply"),
    ("export", Own::Export, "Copy the conversation as Markdown"),
    (
        "branch",
        Own::Branch,
        "Carry the conversation on in a new thread",
    ),
    (
        "fork",
        Own::Branch,
        "Carry the conversation on in a new thread",
    ),
    ("rename", Own::Rename, "Rename this thread"),
    ("diff", Own::Diff, "Show what this thread changed"),
    ("tasks", Own::Tasks, "What runs in the background"),
    ("todos", Own::Todos, "Where the task list stands"),
    ("skills", Own::Skills, "Open skills in Settings"),
    ("resume", Own::Resume, "How threads pick up again"),
    ("help", Own::Help, "What works here"),
    ("clear", Own::Clear, "Start a new thread here"),
    ("new", Own::Clear, "Start a new thread here"),
    ("rewind", Own::Rewind, "Go back to an earlier message"),
    ("login", Own::Login, "Sign in again in a terminal below"),
];

// the interactive CLI's own screens, which its machine mode answers with "isn't available"
const TERMINAL_ONLY: &[&str] = &[
    "add-dir",
    "agents",
    "autofix-pr",
    "background",
    "brief",
    "bug",
    "cd",
    "chrome",
    "cloud-plugins",
    "config",
    "daemon",
    "doctor",
    "exit",
    "feedback",
    "hooks",
    "ide",
    "install",
    "install-github-app",
    "keybindings",
    "logout",
    "mcp",
    "memory",
    "mobile",
    "permissions",
    "plugin",
    "powerup",
    "privacy-settings",
    "radio",
    "release-notes",
    "remote-control",
    "sandbox",
    "session",
    "setup-bedrock",
    "setup-vertex",
    "statusline",
    "stickers",
    "stop",
    "teleport",
    "terminal-setup",
    "theme",
    "tui",
    "ultraplan",
    "upgrade",
    "voice",
    "web-setup",
    "workflows",
];

pub(crate) enum Slash {
    /// One of ours, with what was written after it.
    Own(Own, String),
    TerminalOnly(String),
}

pub(crate) fn parse(text: &str, known: &[String]) -> Option<Slash> {
    let rest = text.strip_prefix('/')?;
    let (name, arg) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    if let Some(&(_, own, _)) = OWN.iter().find(|(n, ..)| *n == name) {
        return Some(Slash::Own(own, arg.trim().to_string()));
    }
    (TERMINAL_ONLY.contains(&name) && !known.iter().any(|k| k == name))
        .then(|| Slash::TerminalOnly(name.to_string()))
}

/// A command that does nothing without words after it, so picking it waits for them.
pub(crate) fn wants_text(own: Own) -> bool {
    matches!(own, Own::Btw | Own::Rename)
}

/// Ours that do something from a phone, as (name, what it does).
pub(crate) fn for_phone() -> Vec<(&'static str, &'static str)> {
    OWN.iter()
        .filter(|(_, own, _)| !matches!(own, Own::Diff | Own::Skills | Own::Rewind | Own::Login))
        .map(|&(name, _, detail)| (name, detail))
        .collect()
}

pub(crate) fn items() -> Vec<Suggestion> {
    OWN.iter()
        .map(|&(name, own, detail)| Suggestion {
            label: name.to_string(),
            detail: detail.to_string(),
            insert: if wants_text(own) {
                format!("/{name} ")
            } else {
                format!("/{name}")
            },
        })
        .collect()
}

impl TranscriptView {
    /// Runs a command typed in the box, or sent from the phone (`phone`), where something that
    /// only shows on this screen says so instead.
    pub(super) fn run_slash(
        &mut self,
        slash: Slash,
        phone: bool,
        images: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let (own, arg) = match slash {
            Slash::Own(own, arg) => (own, arg),
            Slash::TerminalOnly(name) => {
                self.model
                    .note(format!("/{name} only works in a terminal thread."));
                cx.notify();
                return;
            }
        };
        let here = match own {
            Own::Copy
            | Own::Export
            | Own::Diff
            | Own::Skills
            | Own::Rewind
            | Own::Login
            | Own::Clear => true,
            Own::Model | Own::Effort => arg.is_empty(),
            _ => false,
        };
        if phone && here {
            let name = OWN
                .iter()
                .find(|(_, o, _)| *o == own)
                .map_or("", |(n, ..)| *n);
            self.model.note(format!("/{name} works on the computer."));
            cx.notify();
            return;
        }
        match own {
            Own::Model => self.name_model(&arg, cx),
            Own::Effort => self.name_effort(&arg, cx),
            Own::Fast => self.toggle_fast(&arg, cx),
            Own::Plan if self.launch.permission == Permission::Plan => {
                self.model.note("This thread already plans first.");
            }
            Own::Plan => self.pick_permission(Permission::Plan, cx),
            Own::Btw => self.ask_aside(arg, images, cx),
            Own::Context => {
                let line = self.context_line();
                self.model.note(line);
            }
            Own::Status | Own::Version => self.about(own == Own::Status),
            Own::Copy => self.copy_last(cx),
            Own::Export => {
                cx.write_to_clipboard(ClipboardItem::new_string(markdown(&self.model.items)));
                self.model.note("Copied the conversation as Markdown.");
            }
            Own::Branch => self.branch(cx),
            Own::Rename if arg.is_empty() => self.model.note("Write the new name after /rename."),
            Own::Rename => cx.emit(TranscriptEvent::Rename(arg)),
            Own::Diff => cx.emit(TranscriptEvent::ViewChanges),
            Own::Tasks => {
                let line = match self.model.background.len() {
                    0 => "Nothing is running in the background.".to_string(),
                    1 => "1 task runs in the background. It shows above the box, with Stop.".into(),
                    n => format!(
                        "{n} tasks run in the background. They show above the box, with Stop."
                    ),
                };
                self.model.note(line);
            }
            Own::Todos => {
                let line = match super::ask::progress(&self.model.items) {
                    Some((done, total, _)) => format!(
                        "{done} of {total} tasks are done. The list shows above the box while it works."
                    ),
                    None => format!("{} hasn't made a task list here.", self.launch.agent.name()),
                };
                self.model.note(line);
            }
            Own::Skills => cx.emit(TranscriptEvent::Settings(crate::settings::Tab::Skills)),
            Own::Resume => self
                .model
                .note("Threads here pick up where they left off. Open another from the sidebar."),
            Own::Help if phone => self.model.note(
                "Here: /model, /effort, /fast, /plan, /btw, /context, /status, /copy, /export, \
                 /branch, /rename, /tasks, /todos and /clear. Others go to the agent.",
            ),
            Own::Help => self.model.note(
                "Here: /model, /effort, /fast, /plan, /btw, /context, /status, /copy, /export, \
                 /branch, /rename, /diff, /tasks, /todos, /skills and /clear. Skills and the \
                 agent's own commands, like /compact and /usage, go to the agent.",
            ),
            Own::Clear => cx.emit(TranscriptEvent::NewThread),
            Own::Rewind => self
                .model
                .note("Point at one of your messages and click the pencil to edit from there."),
            Own::Login => {
                if self.open && !self.model.running() {
                    self.client.send(Command::Close { id: self.id });
                    self.open = false;
                }
                cx.emit(TranscriptEvent::SignIn);
            }
        }
        cx.notify();
    }

    fn about(&mut self, status: bool) {
        let agent = self.launch.agent.name();
        let version = match self.cli_version.clone() {
            Some(v) => format!("{agent} {v}."),
            None => format!("{agent} hasn't said its version."),
        };
        let line = if status {
            let spec = self.model_spec();
            let model = spec
                .as_ref()
                .and_then(|s| s.models.iter().find(|m| m.id == s.model))
                .map_or_else(|| self.model_label(), |m| m.label.clone());
            let effort = self
                .launch
                .effort
                .as_deref()
                .map_or("default effort".to_string(), |e| {
                    format!("{} effort", crate::composer::effort_label(e))
                });
            let fast = if self.launch.fast { ", fast" } else { "" };
            format!(
                "{version} {model}, {effort}{fast}, {}. {}",
                crate::composer::permission_label(self.launch.permission),
                self.context_line()
            )
        } else {
            version
        };
        self.model.note(line);
    }

    fn name_model(&mut self, name: &str, cx: &mut Context<Self>) {
        if name.is_empty() {
            self.want_menu = Some(false);
            return;
        }
        let want = name.to_lowercase();
        let models = self.model_spec().map(|s| s.models).unwrap_or_default();
        let hit = models
            .iter()
            .find(|m| m.id.to_lowercase() == want || m.label.to_lowercase().starts_with(&want))
            .or_else(|| {
                models
                    .iter()
                    .find(|m| m.label.to_lowercase().contains(&want))
            });
        match hit {
            Some(m) => self.choose(Choice::Model(m.agent, m.id.clone()), cx),
            None => {
                self.model.note(format!("No model here is called {name}."));
                self.want_menu = Some(false);
            }
        }
    }

    fn name_effort(&mut self, name: &str, cx: &mut Context<Self>) {
        if name.is_empty() {
            self.want_menu = Some(true);
            return;
        }
        let efforts = self.model_spec().map(|s| s.efforts).unwrap_or_default();
        let hit = efforts.iter().find(|e| {
            e.eq_ignore_ascii_case(name)
                || crate::composer::effort_label(e).eq_ignore_ascii_case(name)
        });
        match hit {
            Some(e) => self.choose(Choice::Effort(e.clone()), cx),
            None if efforts.is_empty() => self.model.note("This model takes no effort setting."),
            None => {
                let named: Vec<String> = efforts
                    .iter()
                    .map(|e| crate::composer::effort_label(e))
                    .collect();
                self.model
                    .note(format!("This model takes {}.", named.join(", ")));
            }
        }
    }

    fn toggle_fast(&mut self, arg: &str, cx: &mut Context<Self>) {
        let Some(now) = self.model_spec().and_then(|s| s.fast) else {
            self.model.note("This model has no fast mode.");
            return;
        };
        let on = match arg.to_lowercase().as_str() {
            "on" => true,
            "off" => false,
            _ => !now,
        };
        if on == now {
            let state = if on { "on" } else { "off" };
            self.model.note(format!("Fast mode is already {state}."));
        } else {
            self.choose(Choice::Fast(on), cx);
        }
    }

    fn context_line(&self) -> String {
        match self.model.context {
            Some((used, window)) => format!(
                "{}% of the context window is used, {} of {} tokens.",
                (used as f64 / window.max(1) as f64 * 100.).round(),
                tokens(used),
                tokens(window)
            ),
            None => format!(
                "{} hasn't said how full its context is yet. Send a message first.",
                self.launch.agent.name()
            ),
        }
    }

    fn copy_last(&mut self, cx: &mut Context<Self>) {
        let last = self.model.items.iter().rev().find_map(|i| match i {
            Item::Text { source, .. } if !source.trim().is_empty() => Some(source.clone()),
            _ => None,
        });
        match last {
            Some(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.model.note("Copied the last reply.");
            }
            None => self.model.note("There's no reply to copy yet."),
        }
    }

    /// The whole conversation, carried on in a new thread that starts where this one is.
    pub(super) fn branch(&mut self, cx: &mut Context<Self>) {
        if self.remote {
            self.model
                .note("/branch works for threads on this computer.");
            return;
        }
        if self.launch.resume.is_none() {
            self.model
                .note("Send a message first. /branch carries the conversation on.");
            return;
        }
        let mut launch = self.launch.clone();
        launch.rewind = Some(Rewind {
            text: String::new(),
            nth: 0,
            fork: true,
        });
        cx.emit(TranscriptEvent::Fork {
            launch,
            keep: usize::MAX,
            text: String::new(),
            images: Vec::new(),
        });
        self.model
            .note("The conversation goes on in a new thread too.");
    }
}

/// The conversation as Markdown: what was asked, what came back, and a line for each tool call.
fn markdown(items: &[Item]) -> String {
    let mut out = String::new();
    for item in items {
        let part = match item {
            Item::User { text, .. } => format!("## You\n\n{}", text.trim()),
            Item::Text { source, .. } if !source.trim().is_empty() => source.trim().to_string(),
            Item::Tool { tool, .. } => format!("- {}", super::tool::label(tool)),
            _ => continue,
        };
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&part);
    }
    out.push('\n');
    out
}

fn tokens(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{}k", n / 1_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_commands_win_and_screens_stay_in_the_terminal() {
        assert!(matches!(parse("/model", &[]), Some(Slash::Own(Own::Model, a)) if a.is_empty()));
        assert!(matches!(
            parse("/model  sonnet ", &[]),
            Some(Slash::Own(Own::Model, a)) if a == "sonnet"
        ));
        assert!(
            matches!(parse("/btw what now?", &[]), Some(Slash::Own(Own::Btw, a)) if a == "what now?")
        );
        assert!(matches!(
            parse("/fork", &[]),
            Some(Slash::Own(Own::Branch, _))
        ));
        assert!(matches!(parse("/mcp", &[]), Some(Slash::TerminalOnly(n)) if n == "mcp"));
        assert!(parse("/mcp", &["mcp".into()]).is_none());
        assert!(parse("/compact", &[]).is_none());
        assert!(parse("/usr/bin/env", &[]).is_none());
        assert!(parse("model", &[]).is_none());
    }

    #[test]
    fn token_counts_read_short() {
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(68_400), "68k");
        assert_eq!(tokens(1_000_000), "1.0M");
    }
}
