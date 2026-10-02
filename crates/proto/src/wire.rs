// What crosses the channel between the UI and the engine. Commands go in, events come out.
// Session messages name the session they are about, so one event stream can serve every view;
// app-level requests (saved state, agents, resume list, clone) answer with an event of their own.
// Shape and reasons: docs/adr/0002-channel-boundary.md and
// docs/adr/0005-app-requests-and-journals.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{Agent, AgentInfo, AgentSession};
use crate::run::{Answer, Launch, Prompt, RunEvent};
use crate::state::{AppState, Entry};

/// Picked by whoever opens the session (the UI today), unique for the life of the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Start a structured session, and send `prompt` as its first run when there is one.
    /// With `journal`, the engine appends the session's prompts, answers and run events to that
    /// journal (see `LoadJournal`). Opening an id that is already open replaces that session.
    OpenStructured {
        id: SessionId,
        launch: Launch,
        prompt: Option<Prompt>,
        journal: Option<String>,
    },
    /// Start a run, or steer the live one: a prompt sent mid-run joins it.
    Send {
        id: SessionId,
        prompt: Prompt,
    },
    /// Stop the live run. The session stays open for the next prompt.
    Interrupt {
        id: SessionId,
    },
    /// Answer a `RunEvent::Approval`.
    Approve {
        id: SessionId,
        request: String,
        answer: Answer,
    },
    /// Read a journal back. Answered with `Event::Journal` for `id`.
    LoadJournal {
        id: SessionId,
        journal: String,
    },
    /// Spawn the default shell in a PTY sized `cols` x `rows`.
    OpenTerminal {
        id: SessionId,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
    },
    /// Keystrokes, pastes and the emulator's answers to terminal queries.
    WriteTerminal {
        id: SessionId,
        bytes: Vec<u8>,
    },
    ResizeTerminal {
        id: SessionId,
        cols: u16,
        rows: u16,
    },
    /// End a session of either type and kill its process.
    Close {
        id: SessionId,
    },
    /// Answered with `Event::State`.
    LoadState,
    SaveState {
        state: AppState,
    },
    /// Which agents are installed and what they can start with. Answered with `Event::Agents`.
    LoadAgents,
    /// The conversations `agent` saved for `cwd`. Answered with `Event::Resumable`.
    ListResumable {
        agent: Agent,
        cwd: PathBuf,
    },
    /// `git clone url` into `parent/name`, or straight into `parent` when `here`. Progress
    /// comes as `CloneProgress`, the end as `Cloned`, both tagged with `request`.
    Clone {
        request: u64,
        url: String,
        parent: PathBuf,
        name: String,
        here: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Run {
        id: SessionId,
        event: RunEvent,
    },
    /// Coalesced PTY output: at most one batch per frame under load.
    TerminalOutput {
        id: SessionId,
        bytes: Vec<u8>,
    },
    TerminalExit {
        id: SessionId,
        code: i32,
    },
    /// The engine could not do what a command asked. `message` is user-facing.
    Failed {
        id: SessionId,
        message: String,
    },
    /// A journal's entries, oldest first. Empty when there is none yet.
    Journal {
        id: SessionId,
        entries: Vec<Entry>,
    },
    /// The saved state, or the default on a first run. A file that would not parse was moved
    /// aside first, so nothing is lost by saving over it.
    State {
        state: AppState,
    },
    Agents {
        agents: Vec<AgentInfo>,
    },
    Resumable {
        agent: Agent,
        cwd: PathBuf,
        sessions: Vec<AgentSession>,
    },
    /// One of git's progress lines.
    CloneProgress {
        request: u64,
        line: String,
    },
    /// The new folder, or why the clone failed.
    Cloned {
        request: u64,
        result: Result<PathBuf, String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Agent;
    use crate::run::RunStatus;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
    }

    #[test]
    fn commands_survive_json() {
        let cmds = [
            Command::OpenStructured {
                id: SessionId(1),
                launch: Launch::new(Agent::Claude, "/tmp/x"),
                prompt: Some(Prompt::text("hi")),
                journal: Some("thread-4".into()),
            },
            Command::Send {
                id: SessionId(1),
                prompt: Prompt {
                    text: "look".into(),
                    images: vec![PathBuf::from("/tmp/a.png")],
                },
            },
            Command::Interrupt { id: SessionId(1) },
            Command::Approve {
                id: SessionId(1),
                request: "r1".into(),
                answer: Answer::AllowAlways,
            },
            Command::LoadJournal {
                id: SessionId(1),
                journal: "thread-4".into(),
            },
            Command::LoadState,
            Command::SaveState {
                state: AppState::default(),
            },
            Command::LoadAgents,
            Command::ListResumable {
                agent: Agent::Codex,
                cwd: PathBuf::from("/w"),
            },
            Command::Clone {
                request: 3,
                url: "https://github.com/a/b".into(),
                parent: PathBuf::from("/w"),
                name: "b".into(),
                here: false,
            },
            Command::OpenTerminal {
                id: SessionId(2),
                cwd: PathBuf::new(),
                cols: 80,
                rows: 24,
            },
            Command::WriteTerminal {
                id: SessionId(2),
                bytes: b"ls\r".to_vec(),
            },
            Command::ResizeTerminal {
                id: SessionId(2),
                cols: 100,
                rows: 30,
            },
            Command::Close { id: SessionId(2) },
        ];
        for cmd in &cmds {
            assert_eq!(&round_trip(cmd), cmd);
        }
    }

    #[test]
    fn events_survive_json() {
        let events = [
            Event::Run {
                id: SessionId(1),
                event: RunEvent::Text { text: "hi".into() },
            },
            Event::Run {
                id: SessionId(1),
                event: RunEvent::Finished {
                    status: RunStatus::Done,
                    ms: 2000,
                    text: "hi".into(),
                    error: None,
                },
            },
            Event::TerminalOutput {
                id: SessionId(2),
                bytes: vec![0x1b, b'[', b'm'],
            },
            Event::TerminalExit {
                id: SessionId(2),
                code: 0,
            },
            Event::Failed {
                id: SessionId(3),
                message: "no shell".into(),
            },
            Event::Journal {
                id: SessionId(1),
                entries: vec![
                    Entry::Prompt {
                        prompt: Prompt::text("hi"),
                    },
                    Entry::Answer {
                        request: "r1".into(),
                        answer: Answer::Deny,
                    },
                ],
            },
            Event::CloneProgress {
                request: 3,
                line: "Receiving objects: 5%".into(),
            },
            Event::Cloned {
                request: 3,
                result: Err("no such repo".into()),
            },
        ];
        for event in &events {
            assert_eq!(&round_trip(event), event);
        }
    }

    #[test]
    fn tags_are_camel_case() {
        let json = serde_json::to_string(&Command::OpenTerminal {
            id: SessionId(7),
            cwd: PathBuf::new(),
            cols: 1,
            rows: 1,
        })
        .unwrap();
        assert!(json.contains(r#""type":"openTerminal""#), "{json}");
        assert!(json.contains(r#""id":7"#), "{json}");
    }
}
