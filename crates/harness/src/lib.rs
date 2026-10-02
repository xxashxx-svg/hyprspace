//! Drives agent CLIs over their machine protocols and turns their output into `RunEvent`s.
//! Inference always runs through the user's own binary (CLAUDE.md rule 1): no SDK, no API key,
//! no token. Shape and reasons: docs/adr/0004-harness-protocol.md.
//!
//! One `Harness` per agent starts a `Session`: a task that owns the CLI's process and answers
//! `send`, `interrupt` and `answer` until the session is dropped, which kills the process.

pub mod catalog;
pub mod claude;
pub mod codex;
mod spawn;

use std::io;

use hyprspace_proto::{Agent, Answer, Launch, Prompt, RunEvent};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub use claude::Claude;
pub use codex::Codex;

/// Where a session's events go, in order.
pub type Emit = Box<dyn FnMut(RunEvent) + Send>;

/// One agent CLI's adapter.
pub trait Harness: Send + Sync {
    fn agent(&self) -> Agent;

    /// Spawns the CLI for `launch`. Must be called inside a tokio runtime. A spawn failure
    /// returns here; anything later arrives as `RunEvent::Failed`.
    fn start(&self, launch: Launch, emit: Emit) -> io::Result<Session>;
}

/// The default adapter for each agent, running the user's own CLI from PATH. None for an agent
/// that only runs in a terminal.
pub fn for_agent(agent: Agent) -> Option<Box<dyn Harness>> {
    match agent {
        Agent::Claude => Some(Box::new(Claude::default())),
        Agent::Codex => Some(Box::new(Codex::default())),
        Agent::Gemini => None,
    }
}

/// A live structured session. Dropping it ends the session and kills the CLI.
pub struct Session {
    tx: mpsc::UnboundedSender<Input>,
    task: JoinHandle<()>,
}

pub(crate) enum Input {
    Send(Prompt),
    Interrupt,
    Answer { request: String, answer: Answer },
}

impl Session {
    pub(crate) fn new(tx: mpsc::UnboundedSender<Input>, task: JoinHandle<()>) -> Self {
        Self { tx, task }
    }

    /// Starts a run, or steers the live one: the prompt joins it at the CLI's next step.
    pub fn send(&self, prompt: Prompt) {
        let _ = self.tx.send(Input::Send(prompt));
    }

    /// Stops the live run. It ends with `Finished { status: Interrupted }` and the session
    /// stays open. Does nothing between runs.
    pub fn interrupt(&self) {
        let _ = self.tx.send(Input::Interrupt);
    }

    /// Answers the `RunEvent::Approval` named `request`. An unknown request is ignored.
    pub fn answer(&self, request: String, answer: Answer) {
        let _ = self.tx.send(Input::Answer { request, answer });
    }

    /// True once the CLI is gone and the last event was sent.
    pub fn is_closed(&self) -> bool {
        self.task.is_finished()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // the task owns the child, which is spawned with kill_on_drop
        self.task.abort();
    }
}

/// Tool output past this is cut: the transcript shows a preview, the CLI keeps the rest.
const OUTPUT_CAP: usize = 8 * 1024;

pub(crate) fn cap(text: &str) -> String {
    if text.len() <= OUTPUT_CAP {
        return text.to_string();
    }
    let mut end = OUTPUT_CAP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n...", &text[..end])
}

/// Env a parent Claude Code session leaves behind. A child claude that inherits these thinks it
/// is nested inside that session: it turns transcript saving off (so nothing can be resumed) and
/// takes that session's id and messaging token. Only these: CLAUDE_CODE_* settings a user sets on
/// purpose (Bedrock, Git Bash path, output limits) are left alone.
pub const SESSION_ENV: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SSE_PORT",
];
