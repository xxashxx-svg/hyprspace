//! Drives agent CLIs over their machine protocols and turns their output into `RunEvent`s.
//! Inference always runs through the user's own binary (AGENTS.md): no SDK, no API key,
//! no token. Shape and reasons: docs/internals/sessions.md.
//!
//! One `Harness` per agent starts a `Session`: a task that owns the CLI's process and answers
//! `send`, `interrupt` and `answer` until the session is dropped, which kills the process.

pub mod catalog;
pub mod claude;
pub mod codex;
mod images;
pub mod protocol;
mod spawn;
pub mod title;

use std::io;
use std::path::PathBuf;

use hyprspace_proto::{Agent, Answer, Launch, Prompt, RunEvent};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub use claude::Claude;
pub use codex::Codex;
pub use title::title;

/// Where a session's events go, in order.
pub type Emit = Box<dyn FnMut(RunEvent) + Send>;

/// One agent CLI's adapter.
pub trait Harness: Send + Sync {
    fn agent(&self) -> Agent;

    /// Spawns the CLI for `launch`. Must be called inside a tokio runtime. A spawn failure
    /// returns here; anything later arrives as `RunEvent::Failed`.
    fn start(&self, launch: Launch, emit: Emit) -> io::Result<Session>;
}

/// The default adapter for each agent, running the user's own CLI from PATH.
pub fn for_agent(agent: Agent) -> Box<dyn Harness> {
    with_mcp(agent, None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mcp {
    pub command: PathBuf,
    pub args: Vec<String>,
}

pub fn with_mcp(agent: Agent, mcp: Option<Mcp>) -> Box<dyn Harness> {
    match agent {
        Agent::Claude => Box::new(Claude::default().with_mcp(mcp)),
        Agent::Codex => Box::new(Codex::default().with_mcp(mcp)),
    }
}

/// A live structured session. Dropping it ends the session and kills the CLI.
pub struct Session {
    tx: mpsc::UnboundedSender<Input>,
    task: JoinHandle<()>,
    pid: Option<u32>,
}

pub(crate) enum Input {
    Send(Prompt),
    Interrupt,
    StopTask(String),
    TaskOutput(String),
    Answer {
        request: String,
        answer: Answer,
        answers: Vec<(String, String)>,
    },
    Configure(Launch),
}

impl Session {
    pub(crate) fn new(
        tx: mpsc::UnboundedSender<Input>,
        task: JoinHandle<()>,
        pid: Option<u32>,
    ) -> Self {
        Self { tx, task, pid }
    }

    /// The CLI's process, or what started it, like `cmd /c` for an npm shim.
    pub fn pid(&self) -> Option<u32> {
        self.pid
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

    /// Stops one task of `RunEvent::Background`. An unknown task is ignored.
    pub fn stop_task(&self, task: String) {
        let _ = self.tx.send(Input::StopTask(task));
    }

    /// Asks for the end of a background task's output, which comes as `RunEvent::TaskOutput`.
    pub fn task_output(&self, task: String) {
        let _ = self.tx.send(Input::TaskOutput(task));
    }

    /// Answers the `RunEvent::Approval` named `request`. An unknown request is ignored.
    pub fn answer(&self, request: String, answer: Answer, answers: Vec<(String, String)>) {
        let _ = self.tx.send(Input::Answer {
            request,
            answer,
            answers,
        });
    }

    pub fn configure(&self, launch: Launch) {
        let _ = self.tx.send(Input::Configure(launch));
    }

    /// True once the CLI is gone and the last event was sent.
    pub fn is_closed(&self) -> bool {
        self.task.is_finished()
    }

    pub fn handle(&self) -> Handle {
        Handle(self.tx.clone())
    }
}

/// Sends and interrupts for a session from another task. It doesn't keep the CLI alive.
#[derive(Clone)]
pub struct Handle(mpsc::UnboundedSender<Input>);

impl Handle {
    pub fn send(&self, prompt: Prompt) {
        let _ = self.0.send(Input::Send(prompt));
    }

    pub fn interrupt(&self) {
        let _ = self.0.send(Input::Interrupt);
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
