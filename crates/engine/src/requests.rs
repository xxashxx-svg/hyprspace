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
        // before the state was read, a file that names Gemini would not parse and start clean
        let state = match self.store.load(STATE) {
            Ok(Some(raw)) => match serde_json::from_str::<AppState>(&without_gemini(&raw)) {
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
                // a first run, maybe right after the Tauri app updated into this one: bring its
                // spaces and settings over, and save them so it happens only once
                let legacy = self.store.dir().parent().map(|p| p.join("v2"));
                match legacy.and_then(|dir| crate::legacy::import(&dir)) {
                    Some(state) => {
                        self.save_state(&state);
                        state
                    }
                    None => AppState::default(),
                }
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
                    status: crate::providers::status(agent.cli()),
                    catalog: hyprspace_harness::catalog::catalog(agent, &home),
                })
                .collect();
            Self::send(&tx, Event::Agents { agents });
        });
    }

    pub fn list_resumable(&self, agent: Agent, cwd: PathBuf) {
        let tx = self.tx.clone();
        tokio::task::spawn_blocking(move || {
            let sessions = crate::sessions::list(agent.cli(), &cwd);
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

/// Gemini was removed in October 2026. A state saved before then can name it: a terminal thread that ran
/// it, the composer's last agent, its model pick. Those go, a thread keeping its shell, so the
/// rest loads instead of the whole file failing to parse and the app starting clean.
fn without_gemini(raw: &str) -> String {
    use serde_json::Value;
    if !raw.contains("\"gemini\"") {
        return raw.to_string();
    }
    let Ok(mut v) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    fn names_gemini(v: &Value) -> bool {
        v.get("agent").and_then(Value::as_str) == Some("gemini")
    }
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, child) in map.iter_mut() {
                    if (k == "run" && names_gemini(child))
                        || (k == "agent" && child.as_str() == Some("gemini"))
                    {
                        *child = Value::Null;
                    } else {
                        walk(child);
                    }
                }
            }
            Value::Array(items) => {
                items.retain(|i| !names_gemini(i));
                items.iter_mut().for_each(walk);
            }
            _ => {}
        }
    }
    walk(&mut v);
    serde_json::to_string(&v).unwrap_or_else(|_| raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_that_names_gemini_still_loads() {
        use hyprspace_proto::Launch;
        use hyprspace_proto::state::{Pick, Space, Thread, ThreadKind};
        let mut s = AppState::default();
        s.spaces.push(Space {
            id: 1,
            threads: vec![Thread {
                id: 2,
                kind: ThreadKind::Terminal {
                    cwd: "/w".into(),
                    run: Some(Launch::new(Agent::Codex, "/w")),
                },
                ..Default::default()
            }],
            ..Default::default()
        });
        s.composer.agent = Some(Agent::Codex);
        s.composer.picks = vec![Pick {
            agent: Agent::Codex,
            model: String::new(),
            effort: String::new(),
        }];
        // the same state, saved while Gemini was there
        let raw = serde_json::to_string(&s)
            .unwrap()
            .replace("\"codex\"", "\"gemini\"");
        assert!(serde_json::from_str::<AppState>(&raw).is_err());
        let back: AppState = serde_json::from_str(&without_gemini(&raw)).unwrap();
        assert_eq!(back.composer.agent, None);
        assert!(back.composer.picks.is_empty());
        let (_, t) = back.thread(2).unwrap();
        assert_eq!(t.agent(), None);
        assert_eq!(t.cwd(), &std::path::PathBuf::from("/w"));
    }
    use futures::StreamExt as _;

    #[test]
    fn a_first_run_brings_over_the_tauri_apps_state_once() {
        let home = tempfile::tempdir().unwrap();
        let v2 = home.path().join("v2");
        std::fs::create_dir(&v2).unwrap();
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tauri-v2");
        for f in std::fs::read_dir(fixture).unwrap() {
            let f = f.unwrap().path();
            std::fs::copy(&f, v2.join(f.file_name().unwrap())).unwrap();
        }
        let store = Store::open(home.path().join("native")).unwrap();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let requests = Requests::new(store.clone(), tx);

        requests.load_state();
        let Some(Event::State { state }) = futures::executor::block_on(rx.next()) else {
            panic!("no state")
        };
        assert_eq!(state.spaces.len(), 4);
        assert!(store.load(STATE).unwrap().is_some(), "saved right away");

        // the Tauri app goes on changing its own store; ours no longer follows it
        std::fs::write(v2.join("workspaces.json"), r#"{"workspaces":[]}"#).unwrap();
        requests.load_state();
        let Some(Event::State { state: again }) = futures::executor::block_on(rx.next()) else {
            panic!("no state")
        };
        assert_eq!(again, state);
    }
}
