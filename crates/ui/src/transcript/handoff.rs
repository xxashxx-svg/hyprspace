// Carrying a thread on with the other agent, as when one hits its usage limit. The new thread
// starts in the same folder with the conversation so far as its first message: the first request,
// then as many of the latest turns as fit, with what the tools did named but not their output.

use gpui::Context;
use hyprspace_proto::{Agent, Launch, Prompt};

use super::model::Item;
use super::{TranscriptEvent, TranscriptView};

/// About what a long first message can hold and still leave the agent room to work.
const ROOM: usize = 40_000;

impl TranscriptView {
    /// Starts a thread with the other agent here. `ask` is what to do next, or carry on.
    pub(super) fn hand_off(&mut self, ask: String, cx: &mut Context<Self>) {
        if self.remote {
            self.model
                .note("Handing off works for threads on this computer.");
            return;
        }
        if !self
            .model
            .items
            .iter()
            .any(|i| matches!(i, Item::User { .. }))
        {
            self.model
                .note("Send a message first. /handoff carries the conversation on.");
            return;
        }
        let to = other(self.launch.agent);
        let launch = Launch {
            agent: to,
            cwd: self
                .model
                .cwd
                .clone()
                .unwrap_or_else(|| self.launch.cwd.clone()),
            model: None,
            effort: None,
            permission: self.launch.permission,
            resume: None,
            fast: false,
            rewind: None,
        };
        let text = context(&self.model.items, self.launch.agent, &ask);
        cx.emit(TranscriptEvent::Handoff {
            launch,
            prompt: Prompt::text(text),
        });
    }
}

pub(super) fn other(agent: Agent) -> Agent {
    match agent {
        Agent::Claude => Agent::Codex,
        Agent::Codex => Agent::Claude,
    }
}

/// The handoff message: the conversation as `from` had it, then what to do now.
pub(super) fn context(items: &[Item], from: Agent, ask: &str) -> String {
    let turns = turns(items);
    let (first, rest) = turns
        .split_first()
        .map_or((None, &[][..]), |(f, r)| (Some(f), r));
    let mut kept: Vec<&String> = Vec::new();
    let mut used = first.map_or(0, String::len);
    for turn in rest.iter().rev() {
        if used + turn.len() > ROOM {
            break;
        }
        used += turn.len();
        kept.push(turn);
    }
    kept.reverse();
    let left_out = rest.len() - kept.len();
    let mut out = format!(
        "This conversation started with {}, which can't carry on, so you're taking it over in the \
         same folder. Here is what happened, oldest first. The files are as it left them.\n",
        from.name()
    );
    if let Some(first) = first {
        out.push('\n');
        out.push_str(first);
    }
    if left_out > 0 {
        out.push_str(&format!("\n\n({left_out} turns in between are left out.)"));
    }
    for turn in kept {
        out.push_str("\n\n");
        out.push_str(turn);
    }
    let ask = ask.trim();
    let ask = if ask.is_empty() {
        "Pick up where it left off."
    } else {
        ask
    };
    out.push_str(&format!("\n\nNow: {ask}"));
    out
}

/// Each request with what came back for it, as plain text.
fn turns(items: &[Item]) -> Vec<String> {
    let mut turns: Vec<String> = Vec::new();
    for item in items {
        let part = match item {
            Item::User { text, .. } => {
                turns.push(format!("Request: {}", text.trim()));
                continue;
            }
            Item::Text { source, .. } if !source.trim().is_empty() => {
                format!("Reply: {}", source.trim())
            }
            Item::Tool { tool, .. } => format!("- {}", super::tool::label(tool)),
            Item::Error(e) => format!("Error: {e}"),
            _ => continue,
        };
        match turns.last_mut() {
            Some(turn) => {
                turn.push('\n');
                turn.push_str(&part);
            }
            None => turns.push(part),
        }
    }
    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> Item {
        Item::User {
            text: text.into(),
            images: Vec::new(),
            steer: false,
            at: None,
        }
    }

    #[test]
    fn the_first_request_and_the_latest_turns_go_over_with_what_to_do_now() {
        let mut items = vec![user("Build the login page")];
        for n in 0..200 {
            items.push(user(&format!("step {n} {}", "x".repeat(400))));
        }
        let text = context(&items, Agent::Claude, "");
        assert!(text.starts_with("This conversation started with Claude"));
        assert!(text.contains("Request: Build the login page"));
        assert!(text.contains("step 199"));
        assert!(!text.contains("step 0 "));
        assert!(text.contains("turns in between are left out"));
        assert!(text.ends_with("Now: Pick up where it left off."));
        assert!(text.len() < ROOM + 2_000);
        assert!(
            context(&items[..1], Agent::Codex, "run the tests").ends_with("Now: run the tests")
        );
    }
}
