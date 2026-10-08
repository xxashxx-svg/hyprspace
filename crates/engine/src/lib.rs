//! The engine: sessions, PTYs, git, providers, usage and persistence, with no UI. It runs
//! in-process on its own tokio runtime, and the UI reaches it only through the typed channel in
//! `hyprspace-proto` (docs/internals/overview.md).
//!
//! The library modules (git, usage, skills...) were copied from the Tauri app's Rust and are
//! called directly for now; each gains a command in proto when the UI first needs it.

pub mod env;
mod folder;
pub mod git;
pub mod hooks;
mod images;
pub mod journal;
mod legacy;
mod open;
pub mod persist;
mod phone;
pub mod providers;
pub mod pty;
mod requests;
mod running;
pub mod sessions;
pub mod skills;
pub mod terminal;
mod update;
pub mod usage;
mod util;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use hyprspace_harness::{Emit, Session};
use hyprspace_proto::{Client, Command, Entry, Event, Events, SessionId};
use tokio::runtime::Runtime;
use tokio::task::block_in_place;

use folder::Folders;
use journal::Journal;
use persist::Store;
use phone::Phone;
use pty::PtyManager;
use requests::Requests;
use terminal::Terminals;

pub use util::home_dir;

/// The running engine. The app keeps it only to shut it down; everything else goes through the
/// `Client`.
pub struct Engine {
    terminals: Terminals,
    #[cfg(test)]
    ptys: PtyManager,
    runtime: Mutex<Option<Runtime>>,
}

impl Engine {
    /// Starts the engine with its state in `persist::state_dir()` and returns it with the UI's
    /// two ends of the channel.
    pub fn start() -> std::io::Result<(Engine, Client, Events)> {
        update::sweep();
        Self::start_in(persist::state_dir())
    }

    /// Starts the engine with its saved state and journals under `dir`.
    pub fn start_in(dir: PathBuf) -> std::io::Result<(Engine, Client, Events)> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("engine")
            .enable_all()
            .build()?;
        let store = Store::open(dir.clone())?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        // everything the engine says passes the phone bridge on its way to the UI
        let (event_tx, said) = mpsc::unbounded();
        let (ui_tx, events) = mpsc::unbounded();
        let ptys = PtyManager::default();
        let phone = Phone::new(ui_tx.clone(), ptys.clone(), dir);
        runtime.spawn(tap(said, ui_tx, phone.clone()));
        let terminals = Terminals::new(ptys.clone(), event_tx.clone());
        runtime.spawn(serve(cmd_rx, event_tx, terminals.clone(), store, phone));
        let engine = Engine {
            terminals,
            #[cfg(test)]
            ptys,
            runtime: Mutex::new(Some(runtime)),
        };
        Ok((engine, Client::new(cmd_tx), events))
    }

    /// Kill every session. Call on app quit: ConPTY hosts orphan otherwise, and dropping the
    /// runtime drops each structured session, whose CLI is killed on drop.
    pub fn shutdown(&self) {
        self.terminals.shutdown();
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

/// A live structured session and the journal it writes to, if any.
struct Live {
    session: Session,
    journal: Option<Arc<Journal>>,
}

impl Live {
    fn record(&self, entry: Entry) {
        if let Some(j) = &self.journal {
            j.record(entry);
        }
    }
}

/// Hands each event to the phone bridge, then to the UI.
async fn tap(mut said: UnboundedReceiver<Event>, ui: UnboundedSender<Event>, phone: Phone) {
    while let Some(event) = said.next().await {
        phone.tap(&event);
        if ui.unbounded_send(event).is_err() {
            break;
        }
    }
}

// One command at a time, in order. PTY calls block briefly (a write into a full pipe, a resize),
// so they run under `block_in_place`: keystrokes keep their order and structured sessions keep
// moving on the other worker. A structured session is a harness task; its commands only queue.
async fn serve(
    mut rx: UnboundedReceiver<Command>,
    tx: UnboundedSender<Event>,
    terminals: Terminals,
    store: Store,
    phone: Phone,
) {
    let journals = store.dir().join("journals");
    let requests = Requests::new(store, tx.clone());
    let folders = Folders::default();
    let mut structured: HashMap<SessionId, Live> = HashMap::new();
    while let Some(cmd) = rx.next().await {
        match cmd {
            Command::OpenStructured {
                id,
                launch,
                prompt,
                journal,
            } => {
                // the old session's CLI dies with it, before the new one starts
                structured.remove(&id);
                let agent = launch.agent;
                let journal = journal.map(|name| {
                    let j = Arc::new(Journal::open(&journal::path(&journals, &name)));
                    phone.journal_opened(&name, &j);
                    j
                });
                let events = tx.clone();
                let record = journal.clone();
                let emit: Emit = Box::new(move |event| {
                    if let Some(j) = &record {
                        j.record(Entry::Run {
                            event: event.clone(),
                        });
                    }
                    let _ = events.unbounded_send(Event::Run { id, event });
                });
                match hyprspace_harness::for_agent(agent).start(launch, emit) {
                    Ok(session) => {
                        let live = Live { session, journal };
                        if let Some(prompt) = prompt {
                            live.record(Entry::Prompt {
                                prompt: prompt.clone(),
                            });
                            live.session.send(prompt);
                        }
                        structured.retain(|_, s| !s.session.is_closed());
                        structured.insert(id, live);
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
                Some(live) => {
                    live.record(Entry::Prompt {
                        prompt: prompt.clone(),
                    });
                    live.session.send(prompt);
                }
                None => {
                    let _ = tx.unbounded_send(Event::Failed {
                        id,
                        message: "This session has ended. Start a new one.".into(),
                    });
                }
            },
            Command::Interrupt { id } => {
                if let Some(live) = structured.get(&id) {
                    live.session.interrupt();
                }
            }
            Command::Approve {
                id,
                request,
                answer,
            } => {
                if let Some(live) = structured.get(&id) {
                    live.record(Entry::Answer {
                        request: request.clone(),
                        answer,
                    });
                    live.session.answer(request, answer);
                }
            }
            Command::LoadJournal { id, journal } => {
                let file = journal::path(&journals, &journal);
                let events = tx.clone();
                tokio::task::spawn_blocking(move || {
                    let entries = journal::load(&file);
                    let _ = events.unbounded_send(Event::Journal { id, entries });
                });
            }
            Command::LoadState => requests.load_state(),
            Command::SaveState { state } => block_in_place(|| requests.save_state(&state)),
            Command::LoadAgents => requests.load_agents(),
            Command::ListResumable { agent, cwd } => requests.list_resumable(agent, cwd),
            Command::Clone {
                request,
                url,
                parent,
                name,
                here,
            } => requests.clone_repo(request, url, parent, name, here),
            Command::OpenTerminal {
                id,
                cwd,
                cols,
                rows,
                run,
                prompt,
            } => {
                // before the shell starts, so the bridge keeps its first output too
                phone.opened_terminal(id, cols, rows);
                let opened = block_in_place(|| terminals.open(id, cwd, (cols, rows), run, prompt));
                if let Err(e) = opened {
                    let _ = tx.unbounded_send(Event::Failed {
                        id,
                        message: format!("Could not start the shell: {e}"),
                    });
                }
            }
            Command::WriteTerminal { id, bytes } => {
                // a write to a session that just exited is not worth reporting
                let _ = block_in_place(|| terminals.ptys().write(id, &bytes));
            }
            Command::ResizeTerminal { id, cols, rows } => {
                if phone.resize(id, cols, rows) {
                    let _ = block_in_place(|| terminals.ptys().resize(id, cols, rows));
                }
            }
            Command::Close { id } => {
                // dropping the session kills its CLI
                structured.remove(&id);
                terminals.close(id);
            }
            Command::FindImage {
                id,
                cwd,
                conversation,
                n,
            } => {
                let events = tx.clone();
                tokio::task::spawn_blocking(move || {
                    let path =
                        images::find(&crate::util::home_dir(), &cwd, conversation.as_deref(), n);
                    let _ = events.unbounded_send(Event::ImageFound { id, n, path });
                });
            }
            Command::OpenFile { path, line, col } => {
                tokio::task::spawn_blocking(move || {
                    // the UI only offers files it saw on disk; a race with a delete is not worth
                    // a message
                    let _ = open::open_file(&path, line, col);
                });
            }
            Command::Folder(cmd) => folders.handle(cmd, tx.clone()),
            Command::Usage(cmd) => usage::handle(cmd, tx.clone()),
            Command::Skills(cmd) => skills::handle(cmd, tx.clone()),
            Command::Update(cmd) => update::handle(cmd, tx.clone()),
            Command::Phone(cmd) => phone.command(cmd),
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
        let dir = tempfile::tempdir().unwrap();
        let (engine, client, events) = Engine::start_in(dir.path().into()).unwrap();
        let events = forward(events);
        let id = SessionId(1);
        let wait = Duration::from_secs(30);
        client.send(Command::OpenTerminal {
            id,
            cwd: std::env::temp_dir(),
            cols: 80,
            rows: 24,
            run: None,
            prompt: None,
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
        let dir = tempfile::tempdir().unwrap();
        let (engine, client, events) = Engine::start_in(dir.path().into()).unwrap();
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

    #[test]
    fn state_and_journals_come_back_through_the_channel() {
        use hyprspace_proto::{AppState, Prompt};
        let dir = tempfile::tempdir().unwrap();
        let wait = Duration::from_secs(10);
        let mut state = AppState::default();
        state.take_id();
        {
            let (engine, client, events) = Engine::start_in(dir.path().into()).unwrap();
            let events = forward(events);
            client.send(Command::LoadState);
            match events.recv_timeout(wait).unwrap() {
                Event::State { state: s } => assert_eq!(s, AppState::default()),
                other => panic!("unexpected {other:?}"),
            }
            client.send(Command::SaveState {
                state: state.clone(),
            });
            let j = journal::Journal::open(&journal::path(&dir.path().join("journals"), "t-1"));
            j.record(Entry::Prompt {
                prompt: Prompt::text("hi"),
            });
            drop(j);
            client.send(Command::LoadJournal {
                id: SessionId(4),
                journal: "t-1".into(),
            });
            match events.recv_timeout(wait).unwrap() {
                Event::Journal { id, entries } => {
                    assert_eq!(id, SessionId(4));
                    assert_eq!(entries.len(), 1);
                }
                other => panic!("unexpected {other:?}"),
            }
            engine.shutdown();
        }
        let (engine, client, events) = Engine::start_in(dir.path().into()).unwrap();
        let events = forward(events);
        client.send(Command::LoadState);
        match events.recv_timeout(wait).unwrap() {
            Event::State { state: s } => assert_eq!(s, state),
            other => panic!("unexpected {other:?}"),
        }
        engine.shutdown();
    }

    #[test]
    fn folder_requests_answer_through_the_channel() {
        use hyprspace_proto::{FolderCommand, FolderEvent};
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi\n").unwrap();
        let (engine, client, events) = Engine::start_in(dir.path().join("state")).unwrap();
        let events = forward(events);
        client.send(Command::Folder(FolderCommand::ReadFile {
            path: dir.path().join("a.txt"),
        }));
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Folder(FolderEvent::File { text, .. }) => assert_eq!(text.unwrap(), "hi\n"),
            other => panic!("unexpected {other:?}"),
        }
        client.send(Command::Folder(FolderCommand::ListDir {
            path: dir.path().into(),
        }));
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Folder(FolderEvent::Dir { entries, .. }) => {
                let names: Vec<_> = entries.unwrap().into_iter().map(|e| e.name).collect();
                assert_eq!(names, ["state", "a.txt"]);
            }
            other => panic!("unexpected {other:?}"),
        }
        engine.shutdown();
    }

    #[test]
    fn skills_and_usage_answer_through_the_channel() {
        use hyprspace_proto::agents::{SkillKind, SkillScope};
        use hyprspace_proto::{SkillCommand, SkillEvent, UsageCommand, UsageEvent};
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("proj");
        let (engine, client, events) = Engine::start_in(dir.path().join("state")).unwrap();
        let events = forward(events);
        let wait = Duration::from_secs(10);
        let project = |items: Vec<hyprspace_proto::agents::SkillItem>| -> Vec<String> {
            items
                .into_iter()
                .filter(|s| s.scope == SkillScope::Project)
                .map(|s| s.command)
                .collect()
        };
        let write = |name: &str, replaces: Option<(SkillScope, String)>| SkillCommand::Write {
            cwd: cwd.clone(),
            scope: SkillScope::Project,
            kind: SkillKind::Skill,
            name: name.into(),
            content: "---
description: Fixes
---
Fix it.
"
            .into(),
            replaces,
        };
        client.send(Command::Skills(write("fix", None)));
        match events.recv_timeout(wait).unwrap() {
            Event::Skills(SkillEvent::Done { error }) => assert_eq!(error, None),
            other => panic!("unexpected {other:?}"),
        }
        match events.recv_timeout(wait).unwrap() {
            Event::Skills(SkillEvent::List { items, .. }) => assert_eq!(project(items), ["/fix"]),
            other => panic!("unexpected {other:?}"),
        }
        // a rename writes the new one and removes the old
        client.send(Command::Skills(write(
            "mend",
            Some((SkillScope::Project, "fix".into())),
        )));
        let _ = events.recv_timeout(wait).unwrap();
        match events.recv_timeout(wait).unwrap() {
            Event::Skills(SkillEvent::List { items, .. }) => assert_eq!(project(items), ["/mend"]),
            other => panic!("unexpected {other:?}"),
        }
        client.send(Command::Skills(SkillCommand::Read {
            cwd: cwd.clone(),
            scope: SkillScope::Project,
            kind: SkillKind::Skill,
            name: "mend".into(),
        }));
        match events.recv_timeout(wait).unwrap() {
            Event::Skills(SkillEvent::Read { content, .. }) => {
                assert!(content.unwrap().contains("Fix it."))
            }
            other => panic!("unexpected {other:?}"),
        }
        client.send(Command::Usage(UsageCommand::Local {
            provider: "nobody".into(),
        }));
        match events.recv_timeout(wait).unwrap() {
            Event::Usage(UsageEvent::Local { provider, usage }) => {
                assert_eq!(provider, "nobody");
                assert_eq!(usage, None);
            }
            other => panic!("unexpected {other:?}"),
        }
        engine.shutdown();
    }

    #[test]
    fn a_broken_state_file_is_kept_and_replaced_by_the_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("state.json"), "{nope").unwrap();
        let (engine, client, events) = Engine::start_in(dir.path().into()).unwrap();
        let events = forward(events);
        client.send(Command::LoadState);
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::State { state } => assert_eq!(state, hyprspace_proto::AppState::default()),
            other => panic!("unexpected {other:?}"),
        }
        engine.shutdown();
        let kept = std::fs::read_dir(dir.path()).unwrap().flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("state.corrupt-")
        });
        assert!(kept);
    }
}
