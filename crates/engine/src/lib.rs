//! The engine: sessions, PTYs, git, providers, usage and persistence, with no UI. It runs
//! in-process on its own tokio runtime, and the UI reaches it only through the typed channel in
//! `hyprspace-proto` (docs/adr/0002-channel-boundary.md).
//!
//! The library modules (git, usage, skills...) were copied from the Tauri app's Rust and are
//! called directly for now; each gains a command in proto when the UI first needs it.

pub mod env;
pub mod git;
pub mod hooks;
pub mod persist;
pub mod providers;
pub mod pty;
pub mod sessions;
pub mod skills;
pub mod usage;
mod util;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use hyprspace_harness::{Emit, Session};
use hyprspace_proto::{Client, Command, Event, Events, SessionId};
use tokio::runtime::Runtime;
use tokio::task::block_in_place;

use pty::{PtyManager, Spawn};

pub use util::home_dir;

/// The running engine. The app keeps it only to shut it down; everything else goes through the
/// `Client`.
pub struct Engine {
    ptys: PtyManager,
    runtime: Mutex<Option<Runtime>>,
}

impl Engine {
    /// Starts the engine and returns it with the UI's two ends of the channel.
    pub fn start() -> std::io::Result<(Engine, Client, Events)> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("engine")
            .enable_all()
            .build()?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let (event_tx, events) = mpsc::unbounded();
        let ptys = PtyManager::default();
        runtime.spawn(serve(cmd_rx, event_tx, ptys.clone()));
        let engine = Engine {
            ptys,
            runtime: Mutex::new(Some(runtime)),
        };
        Ok((engine, Client::new(cmd_tx), events))
    }

    /// Kill every session. Call on app quit: ConPTY hosts orphan otherwise, and dropping the
    /// runtime drops each structured session, whose CLI is killed on drop.
    pub fn shutdown(&self) {
        self.ptys.kill_all();
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(rt) = runtime {
            rt.shutdown_timeout(Duration::from_secs(1));
        }
    }
}

// One command at a time, in order. PTY calls block briefly (a write into a full pipe, a resize),
// so they run under `block_in_place`: keystrokes keep their order and structured sessions keep
// moving on the other worker. A structured session is a harness task; its commands only queue.
async fn serve(mut rx: UnboundedReceiver<Command>, tx: UnboundedSender<Event>, ptys: PtyManager) {
    let mut structured: HashMap<SessionId, Session> = HashMap::new();
    while let Some(cmd) = rx.next().await {
        match cmd {
            Command::OpenStructured { id, launch, prompt } => {
                let agent = launch.agent;
                let events = tx.clone();
                let emit: Emit = Box::new(move |event| {
                    let _ = events.unbounded_send(Event::Run { id, event });
                });
                match hyprspace_harness::for_agent(agent).start(launch, emit) {
                    Ok(session) => {
                        if let Some(prompt) = prompt {
                            session.send(prompt);
                        }
                        structured.retain(|_, s| !s.is_closed());
                        structured.insert(id, session);
                    }
                    Err(e) => {
                        let _ = tx.unbounded_send(Event::Failed {
                            id,
                            message: format!("Could not start {}: {e}", agent.name()),
                        });
                    }
                }
            }
            Command::Send { id, prompt } => match structured.get(&id) {
                Some(session) => session.send(prompt),
                None => {
                    let _ = tx.unbounded_send(Event::Failed {
                        id,
                        message: "This session has ended. Start a new one.".into(),
                    });
                }
            },
            Command::Interrupt { id } => {
                if let Some(session) = structured.get(&id) {
                    session.interrupt();
                }
            }
            Command::Approve { id, request, allow } => {
                if let Some(session) = structured.get(&id) {
                    session.answer(request, allow);
                }
            }
            Command::OpenTerminal {
                id,
                cwd,
                cols,
                rows,
            } => {
                let spawn = Spawn {
                    cwd,
                    cols,
                    rows,
                    ..Default::default()
                };
                if let Err(e) = block_in_place(|| ptys.create(id, spawn, tx.clone())) {
                    let _ = tx.unbounded_send(Event::Failed {
                        id,
                        message: format!("Could not start the shell: {e}"),
                    });
                }
            }
            Command::WriteTerminal { id, bytes } => {
                // a write to a session that just exited is not worth reporting
                let _ = block_in_place(|| ptys.write(id, &bytes));
            }
            Command::ResizeTerminal { id, cols, rows } => {
                let _ = block_in_place(|| ptys.resize(id, cols, rows));
            }
            Command::Close { id } => {
                // dropping the session kills its CLI
                structured.remove(&id);
                ptys.kill(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Moves engine events onto a std channel so the test can wait with a timeout.
    fn forward(mut events: Events) -> std::sync::mpsc::Receiver<Event> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Some(e) = futures::executor::block_on(events.next()) {
                if tx.send(e).is_err() {
                    break;
                }
            }
        });
        rx
    }

    #[test]
    fn a_terminal_opens_streams_and_closes_through_the_channel() {
        let (engine, client, events) = Engine::start().unwrap();
        let events = forward(events);
        let id = SessionId(1);
        let wait = Duration::from_secs(30);
        client.send(Command::OpenTerminal {
            id,
            cwd: std::env::temp_dir(),
            cols: 80,
            rows: 24,
        });
        // any shell prints something on start: a prompt, a banner, or a cursor query
        match events.recv_timeout(wait).unwrap() {
            Event::TerminalOutput { id: got, bytes } => {
                assert_eq!(got, id);
                assert!(!bytes.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
        client.send(Command::WriteTerminal {
            id,
            bytes: b"x".to_vec(),
        });
        client.send(Command::ResizeTerminal {
            id,
            cols: 100,
            rows: 30,
        });
        client.send(Command::Close { id });
        loop {
            match events.recv_timeout(wait).unwrap() {
                Event::TerminalExit { id: got, .. } => {
                    assert_eq!(got, id);
                    break;
                }
                Event::TerminalOutput { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(engine.ptys.is_empty());
        engine.shutdown();
    }

    #[test]
    fn a_prompt_for_a_closed_session_says_so() {
        let (engine, client, events) = Engine::start().unwrap();
        let events = forward(events);
        client.send(Command::Send {
            id: SessionId(9),
            prompt: hyprspace_proto::Prompt::text("hi"),
        });
        // commands for a session that is gone are not errors
        client.send(Command::Interrupt { id: SessionId(9) });
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Failed { id, message } => {
                assert_eq!(id, SessionId(9));
                assert!(message.contains("ended"), "{message}");
            }
            other => panic!("unexpected {other:?}"),
        }
        engine.shutdown();
    }
}
