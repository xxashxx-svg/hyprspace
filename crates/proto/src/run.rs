// What a structured session is started with and what its runs report. Each harness turns its
// CLI's own protocol into these types, so the transcript never sees raw CLI JSON.
// Shape and reasons: docs/adr/0004-harness-protocol.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::Agent;

/// How much an agent may do without asking. Each harness maps these onto its CLI's own modes
/// (docs/adr/0004-harness-protocol.md has the table).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// Reads and plans only.
    Plan,
    /// Asks before edits and commands.
    #[default]
    Ask,
    /// Edits on its own inside the folder, asks before the rest.
    Auto,
    /// Never asks. Only for folders the user trusts.
    Bypass,
}

/// Everything a structured session starts with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub agent: Agent,
    pub cwd: PathBuf,
    /// None lets the CLI use its own default.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission: Permission,
    /// A thread id from an earlier `Started` or the resume list. Claude only resumes in the
    /// folder the conversation started in, so its harness finds that folder itself.
    pub resume: Option<String>,
}

impl Launch {
    pub fn new(agent: Agent, cwd: impl Into<PathBuf>) -> Self {
        Self {
            agent,
            cwd: cwd.into(),
            model: None,
            effort: None,
            permission: Permission::default(),
            resume: None,
        }
    }
}

/// One message from the user: text plus image files to attach.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub text: String,
    pub images: Vec<PathBuf>,
}

impl Prompt {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            images: Vec::new(),
        }
    }
}

/// What a structured session reports, in order. A run starts when a prompt is sent while no
/// run is live, and ends with exactly one `Finished`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEvent {
    /// The CLI is up. `thread` is the id `Launch::resume` takes to reopen this conversation,
    /// and `cwd` the folder it runs in (for a resumed Claude thread, its original folder).
    Started {
        agent: Agent,
        model: String,
        thread: String,
        cwd: PathBuf,
    },
    /// A piece of the reply.
    Text { text: String },
    /// A piece of the model's visible reasoning.
    Thinking { text: String },
    /// A tool call began. The same id can come again with more detail filled in.
    Tool { id: String, tool: Tool },
    /// A tool call ended. `output` is capped, for display.
    ToolDone {
        id: String,
        ok: bool,
        output: String,
    },
    /// The agent waits for a yes or no before running `tool`. Answer with `Command::Approve`.
    Approval {
        request: String,
        tool: Tool,
        reason: Option<String>,
    },
    /// A prompt sent while the run was live joined it.
    Steered,
    /// Tokens the run used.
    Usage { input: u64, output: u64 },
    /// Something went wrong that the user should see. The run may still go on.
    Error { message: String },
    Finished {
        status: RunStatus,
        ms: u64,
        /// The final reply, for a CLI that sent no partial text.
        text: String,
        error: Option<String>,
    },
    /// The session is over: the CLI exited or could not start. `message` is user-facing.
    Failed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Done,
    Interrupted,
    Failed,
}

/// A tool call, reduced to what the transcript shows for each kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Tool {
    Command {
        command: String,
    },
    Read {
        path: String,
    },
    /// File edits, each with a diff.
    Edit {
        changes: Vec<FileChange>,
    },
    Search {
        pattern: String,
        path: Option<String>,
    },
    /// A web search or a page fetch.
    Web {
        target: String,
    },
    Mcp {
        server: String,
        tool: String,
        input: String,
    },
    /// Anything else, with its input as compact JSON.
    Other {
        name: String,
        input: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    /// Unified-diff lines (` `, `-`, `+`, `@@`). Claude's edits carry no line numbers, so their
    /// hunks have a bare `@@`.
    pub diff: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Add,
    Update,
    Delete,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
    }

    #[test]
    fn run_events_survive_json() {
        let events = [
            RunEvent::Started {
                agent: Agent::Codex,
                model: "gpt-5.5".into(),
                thread: "t1".into(),
                cwd: PathBuf::from("/w"),
            },
            RunEvent::Tool {
                id: "c1".into(),
                tool: Tool::Edit {
                    changes: vec![FileChange {
                        path: "a.rs".into(),
                        kind: ChangeKind::Update,
                        diff: "@@\n-a\n+b".into(),
                    }],
                },
            },
            RunEvent::Approval {
                request: "7".into(),
                tool: Tool::Command {
                    command: "ls".into(),
                },
                reason: None,
            },
            RunEvent::Finished {
                status: RunStatus::Interrupted,
                ms: 10,
                text: String::new(),
                error: None,
            },
        ];
        for event in &events {
            assert_eq!(&round_trip(event), event);
        }
    }

    #[test]
    fn launch_defaults_to_asking() {
        let l = Launch::new(Agent::Claude, "/w");
        assert_eq!(l.permission, Permission::Ask);
        assert_eq!(round_trip(&l), l);
        let json = serde_json::to_string(&Tool::Web { target: "x".into() }).unwrap();
        assert!(json.contains(r#""kind":"web""#), "{json}");
    }
}
