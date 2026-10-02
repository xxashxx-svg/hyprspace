// App-level requests: the saved state, which agents can start, the resume list, and clones.
// Each one that shells out or reads a lot runs on the blocking pool and answers with its own
// event, so a slow `codex --version` never holds up keystrokes queued behind it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::agents::AgentInfo;
use hyprspace_proto::{Agent, AppState, Event};

use crate::persist::Store;

const STATE: &str = "state";

pub struct Requests {
    store: Store,
    /// False after a load hit a real IO error: saving then would overwrite a file that may be
    /// fine, so saves are dropped until a load succeeds.
    can_save: Arc<AtomicBool>,
    tx: UnboundedSender<Event>,
}

impl Requests {
    pub fn new(store: Store, tx: UnboundedSender<Event>) -> Self {
        Self {
            store,
            can_save: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    fn send(tx: &UnboundedSender<Event>, event: Event) {
        let _ = tx.unbounded_send(event);
    }

    pub fn load_state(&self) {
        let state = match self.store.load(STATE) {
            Ok(Some(raw)) => match serde_json::from_str::<AppState>(&raw) {
                Ok(state) => {
                    self.can_save.store(true, Ordering::Relaxed);
                    state
                }
                Err(_) => {
                    // keep the broken file for a human, start clean
                    let moved = self.store.backup(STATE).is_ok();
                    self.can_save.store(moved, Ordering::Relaxed);
                    AppState::default()
                }
            },
            Ok(None) => {
                self.can_save.store(true, Ordering::Relaxed);
                AppState::default()
            }
            Err(_) => AppState::default(),
        };
        Self::send(&self.tx, Event::State { state });
    }

    /// Runs on the command loop so saves land in the order they were sent.
    pub fn save_state(&self, state: &AppState) {
        if !self.can_save.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(raw) = serde_json::to_string_pretty(state) {
            let _ = self.store.save(STATE, &raw);
        }
    }

    pub fn load_agents(&self) {
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let home = crate::home_dir();
            let agents = [Agent::Claude, Agent::Codex]
                .into_iter()
                .map(|agent| AgentInfo {
                    agent,
                    status: crate::providers::status(id(agent)),
                    catalog: hyprspace_harness::catalog::catalog(agent, &home),
                })
                .collect();
            Self::send(&tx, Event::Agents { agents });
        });
    }

    pub fn list_resumable(&self, agent: Agent, cwd: PathBuf) {
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let sessions = crate::sessions::list(id(agent), &cwd);
            Self::send(
                &tx,
                Event::Resumable {
                    agent,
                    cwd,
                    sessions,
                },
            );
        });
    }

    pub fn clone_repo(&self, request: u64, url: String, parent: PathBuf, name: String, here: bool) {
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            // git redraws its counters many times a second; a few updates a second is plenty
            let mut last: Option<Instant> = None;
            let result = crate::git::clone(&url, &parent, &name, here, |line| {
                if last.is_none_or(|t| t.elapsed() >= Duration::from_millis(120)) {
                    last = Some(Instant::now());
                    Self::send(&tx, Event::CloneProgress { request, line });
                }
            });
            Self::send(&tx, Event::Cloned { request, result });
        });
    }
}

/// The CLI name the provider and session modules key on.
fn id(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude",
        Agent::Codex => "codex",
    }
}
