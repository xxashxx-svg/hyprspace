// What crosses the channel between the UI and the engine. Commands go in, events come out, and
// every message names the session it is about, so one event stream can serve every view.
// Shape and reasons: docs/adr/0002-channel-boundary.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::run::{Launch, Prompt, RunEvent};

/// Picked by whoever opens the session (the UI today), unique for the life of the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Start a structured session, and send `prompt` as its first run when there is one.
    OpenStructured {
        id: SessionId,
        launch: Launch,
        prompt: Option<Prompt>,
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
        allow: bool,
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
                allow: true,
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
