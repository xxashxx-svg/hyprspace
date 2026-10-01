// What crosses the channel between the UI and the engine. Commands go in, events come out, and
// every message names the session it is about, so one event stream can serve every view.
// Shape and reasons: docs/adr/0002-channel-boundary.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Picked by whoever opens the session (the UI today), unique for the life of the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Start a structured Claude session in `cwd` and send `prompt` as its first run.
    OpenStructured {
        id: SessionId,
        cwd: PathBuf,
        prompt: String,
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

/// What one run of a structured session reports, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEvent {
    Started {
        model: String,
    },
    /// A piece of the main thread's reply text.
    Text {
        text: String,
    },
    /// A tool asked for approval and was denied, because approvals have no UI yet.
    Denied {
        tool: String,
    },
    Finished {
        ok: bool,
        ms: u64,
        /// The full reply, for a CLI that sent no partial text.
        text: String,
    },
    Failed {
        message: String,
    },
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
    fn commands_survive_json() {
        let cmds = [
            Command::OpenStructured {
                id: SessionId(1),
                cwd: PathBuf::from("/tmp/x"),
                prompt: "hi".into(),
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
                    ok: true,
                    ms: 2000,
                    text: "hi".into(),
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
