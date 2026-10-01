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
use hyprspace_proto::{Client, Command, Event, Events, SessionId};
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, block_in_place};

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
    /// runtime drops each structured run, whose child is killed on drop.
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
// so they run under `block_in_place`: keystrokes keep their order and structured runs keep moving
// on the other worker.
async fn serve(mut rx: UnboundedReceiver<Command>, tx: UnboundedSender<Event>, ptys: PtyManager) {
    let mut runs: HashMap<SessionId, AbortHandle> = HashMap::new();
    while let Some(cmd) = rx.next().await {
        match cmd {
            Command::OpenStructured { id, cwd, prompt } => {
                let tx = tx.clone();
                let task = tokio::spawn(async move {
                    hyprspace_harness::claude::run(prompt, &cwd, |event| {
                        let _ = tx.unbounded_send(Event::Run { id, event });
                    })
                    .await;
                });
                runs.retain(|_, h| !h.is_finished());
                runs.insert(id, task.abort_handle());
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
                if let Some(run) = runs.remove(&id) {
                    run.abort();
                }
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
}
