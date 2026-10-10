//! The computers this one is paired with, as their client: a connection to each one's bridge,
//! and the sessions the UI routes to their threads. The host side is the phone bridge
//! (`crate::phone`). Shape and reasons: docs/internals/machines.md.

mod link;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use hyprspace_proto::peer::{Found, Peer, PeerCommand, PeerEvent};
use hyprspace_proto::phone::{Ask, Down, INPUT, Up, frame};
use hyprspace_proto::{Command, Event, Prompt, SessionId};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
pub struct Peers(Arc<Mutex<Inner>>);

struct Inner {
    ui: UnboundedSender<Event>,
    path: PathBuf,
    saved: Vec<Saved>,
    links: HashMap<String, Link>,
    routes: HashMap<SessionId, Route>,
    uploads: HashMap<u64, oneshot::Sender<Result<String, String>>>,
    next_upload: u64,
    look: Option<JoinHandle<()>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    /// The host's certificate fingerprint, which the connection pins.
    id: String,
    name: String,
    hosts: Vec<String>,
    port: u16,
    token: String,
}

#[derive(Default)]
struct Link {
    tx: Option<UnboundedSender<Message>>,
    error: Option<String>,
    version: String,
    task: Option<JoinHandle<()>>,
}

struct Route {
    peer: String,
    thread: u64,
    kind: Kind,
    size: Option<(u16, u16)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Unknown,
    Journal,
    Terminal,
}

impl Peers {
    pub fn new(ui: UnboundedSender<Event>, dir: &Path) -> Self {
        let path = dir.join("peers.json");
        let saved: Vec<Saved> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self(Arc::new(Mutex::new(Inner {
            ui,
            path,
            saved,
            links: HashMap::new(),
            routes: HashMap::new(),
            uploads: HashMap::new(),
            next_upload: 1,
            look: None,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Connects to every saved host. Inside the engine's runtime.
    pub fn start(&self) {
        let ids: Vec<String> = self.lock().saved.iter().map(|s| s.id.clone()).collect();
        if ids.is_empty() {
            return;
        }
        for id in ids {
            self.connect(id);
        }
        self.lock().send_peers();
    }

    fn connect(&self, id: String) {
        let task = tokio::spawn(link::run(self.clone(), id.clone()));
        let mut inner = self.lock();
        let link = inner.links.entry(id).or_default();
        if let Some(old) = link.task.replace(task) {
            old.abort();
        }
    }

    pub fn command(&self, cmd: PeerCommand) {
        match cmd {
            PeerCommand::Pair { link } => {
                let me = self.clone();
                tokio::spawn(async move {
                    match link::pair(&link).await {
                        Ok((saved, version)) => me.paired(saved, version),
                        Err(message) => me.lock().to_ui(PeerEvent::PairFailed { message }),
                    }
                });
            }
            PeerCommand::Forget { peer } => {
                self.up(&peer, Up::Leave);
                // the leave goes out before the connection drops
                let me = self.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    me.forget(&peer);
                });
            }
            PeerCommand::Look { on } => {
                let mut inner = self.lock();
                if let Some(t) = inner.look.take() {
                    t.abort();
                }
                if on {
                    inner.look = Some(tokio::spawn(link::look(self.clone())));
                }
            }
            PeerCommand::Bind { id, peer, thread } => {
                self.lock().routes.insert(
                    id,
                    Route {
                        peer,
                        thread,
                        kind: Kind::Unknown,
                        size: None,
                    },
                );
            }
            PeerCommand::Unbind { id } => {
                self.lock().routes.remove(&id);
            }
            PeerCommand::Up { peer, up } => self.up(&peer, up),
            PeerCommand::Folder { peer, cmd } => self.up(&peer, Up::Folder { cmd }),
            PeerCommand::New {
                peer,
                mut ask,
                images,
            } => {
                let me = self.clone();
                tokio::spawn(async move {
                    if let Ask::New { start, .. } = &mut ask {
                        start.images = me.upload(&peer, images).await;
                    }
                    me.up(&peer, Up::Ask { ask });
                });
            }
        }
    }

    /// What the UI sends about a session bound to another computer goes there; anything else
    /// comes back to run here.
    pub fn route(&self, cmd: Command) -> Option<Command> {
        let id = match &cmd {
            Command::OpenStructured { id, .. }
            | Command::Send { id, .. }
            | Command::Interrupt { id }
            | Command::Approve { id, .. }
            | Command::LoadJournal { id, .. }
            | Command::OpenTerminal { id, .. }
            | Command::WriteTerminal { id, .. }
            | Command::ResizeTerminal { id, .. }
            | Command::Close { id }
            | Command::FindImage { id, .. } => *id,
            _ => return Some(cmd),
        };
        let mut inner = self.lock();
        let Some(route) = inner.routes.get_mut(&id) else {
            return Some(cmd);
        };
        let (peer, thread) = (route.peer.clone(), route.thread);
        let mut ups = Vec::new();
        match cmd {
            Command::LoadJournal { .. } => {
                route.kind = Kind::Journal;
                ups.push(Up::Watch { thread });
            }
            Command::OpenTerminal { cols, rows, .. } => {
                route.kind = Kind::Terminal;
                route.size = Some((cols, rows));
                ups.push(Up::Attach { thread });
                ups.push(Up::Size { thread, cols, rows });
            }
            Command::ResizeTerminal { cols, rows, .. } => {
                route.size = Some((cols, rows));
                ups.push(Up::Size { thread, cols, rows });
            }
            Command::WriteTerminal { bytes, .. } => {
                inner.send(&peer, Message::Binary(frame(INPUT, thread, &bytes).into()));
            }
            Command::OpenStructured {
                prompt: Some(prompt),
                ..
            }
            | Command::Send { prompt, .. } => {
                drop(inner);
                self.say(peer, thread, prompt);
                return None;
            }
            Command::Interrupt { .. } => ups.push(Up::Ask {
                ask: Ask::Interrupt { thread },
            }),
            Command::Approve {
                request,
                answer,
                answers,
                ..
            } => ups.push(Up::Ask {
                ask: Ask::Approve {
                    thread,
                    request,
                    answer,
                    answers,
                },
            }),
            Command::Close { .. } => {
                let kind = route.kind;
                inner.routes.remove(&id);
                match kind {
                    Kind::Journal => ups.push(Up::Unwatch { thread }),
                    Kind::Terminal => ups.push(Up::Detach { thread }),
                    Kind::Unknown => {}
                }
            }
            _ => {}
        }
        for up in ups {
            inner.send_up(&peer, &up);
        }
        None
    }

    fn up(&self, peer: &str, up: Up) {
        self.lock().send_up(peer, &up);
    }

    /// A message for a structured thread: attached files go up first, and the message names
    /// them by where the host saved them.
    fn say(&self, peer: String, thread: u64, prompt: Prompt) {
        let Prompt { text, images } = prompt;
        if images.is_empty() {
            self.up(
                &peer,
                Up::Ask {
                    ask: Ask::Send {
                        thread,
                        text,
                        images: Vec::new(),
                    },
                },
            );
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            let saved = me.upload(&peer, images).await;
            me.up(
                &peer,
                Up::Ask {
                    ask: Ask::Send {
                        thread,
                        text,
                        images: saved,
                    },
                },
            );
        });
    }

    /// Sends files to a host, which saves them and answers with where.
    async fn upload(&self, peer: &str, files: Vec<PathBuf>) -> Vec<String> {
        let mut saved = Vec::new();
        for path in files {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string());
            let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || std::fs::read(path)).await
            else {
                continue;
            };
            let rx = {
                let mut inner = self.lock();
                let id = inner.next_upload;
                inner.next_upload += 1;
                let (tx, rx) = oneshot::channel();
                inner.uploads.insert(id, tx);
                inner.send_up(
                    peer,
                    &Up::Upload {
                        id,
                        data: STANDARD.encode(bytes),
                        name,
                    },
                );
                rx
            };
            if let Ok(Ok(Ok(path))) = tokio::time::timeout(Duration::from_secs(120), rx).await {
                saved.push(path);
            }
        }
        saved
    }

    fn paired(&self, saved: Saved, version: String) {
        let id = saved.id.clone();
        {
            let mut inner = self.lock();
            inner.saved.retain(|s| s.id != id);
            inner.saved.push(saved);
            inner.save();
            inner.links.entry(id.clone()).or_default().version = version;
        }
        self.connect(id);
        self.lock().send_peers();
    }

    fn forget(&self, peer: &str) {
        let mut inner = self.lock();
        inner.saved.retain(|s| s.id != peer);
        inner.save();
        if let Some(task) = inner.links.remove(peer).and_then(|l| l.task) {
            task.abort();
        }
        inner.drop_routes(peer, "That computer was unpaired.");
        inner.send_peers();
    }

    fn saved(&self, peer: &str) -> Option<Saved> {
        self.lock().saved.iter().find(|s| s.id == peer).cloned()
    }

    /// The host is found somewhere else on the network now.
    fn moved(&self, peer: &str, hosts: Vec<String>, port: u16) {
        let mut inner = self.lock();
        if let Some(s) = inner.saved.iter_mut().find(|s| s.id == peer)
            && (s.hosts != hosts || s.port != port)
        {
            s.hosts = hosts;
            s.port = port;
            inner.save();
        }
    }

    /// Connected: what was routed to the host before comes back.
    fn online(&self, peer: &str, tx: UnboundedSender<Message>, version: String) {
        let mut inner = self.lock();
        let link = inner.links.entry(peer.to_string()).or_default();
        link.tx = Some(tx);
        link.error = None;
        link.version = version;
        let ups: Vec<Up> = inner
            .routes
            .values()
            .filter(|r| r.peer == peer)
            .flat_map(|r| match (r.kind, r.size) {
                (Kind::Journal, _) => vec![Up::Watch { thread: r.thread }],
                (Kind::Terminal, Some((cols, rows))) => vec![
                    Up::Attach { thread: r.thread },
                    Up::Size {
                        thread: r.thread,
                        cols,
                        rows,
                    },
                ],
                _ => Vec::new(),
            })
            .collect();
        for up in ups {
            inner.send_up(peer, &up);
        }
        inner.send_peers();
    }

    fn offline(&self, peer: &str, error: Option<String>) {
        let mut inner = self.lock();
        let Some(link) = inner.links.get_mut(peer) else {
            return;
        };
        let changed = link.tx.take().is_some() || link.error != error;
        link.error = error;
        if changed {
            inner.send_peers();
        }
    }

    fn heard(&self, peer: &str, down: Down) {
        let mut inner = self.lock();
        match down {
            Down::Board { board } => inner.to_ui(PeerEvent::Board {
                peer: peer.to_string(),
                board,
            }),
            Down::Transcript {
                thread,
                entries,
                reset,
            } => {
                for id in inner.routed(peer, thread, Kind::Journal) {
                    let entries = entries.clone();
                    let _ = inner.ui.unbounded_send(if reset {
                        Event::Journal { id, entries }
                    } else {
                        Event::Recorded { id, entries }
                    });
                }
            }
            Down::Exited { thread, code } => {
                for id in inner.routed(peer, thread, Kind::Terminal) {
                    let _ = inner.ui.unbounded_send(Event::TerminalExit { id, code });
                }
            }
            Down::Uploaded { id, path, error } => {
                if let Some(tx) = inner.uploads.remove(&id) {
                    let _ = tx.send(path.ok_or_else(|| error.unwrap_or_default()));
                }
            }
            Down::Folder { event } => inner.to_ui(PeerEvent::Folder {
                peer: peer.to_string(),
                event: *event,
            }),
            Down::Denied { forget: true, .. } => {
                drop(inner);
                self.forget(peer);
            }
            Down::Pong | Down::Welcome { .. } => {}
            down => inner.to_ui(PeerEvent::Down {
                peer: peer.to_string(),
                down,
            }),
        }
    }

    fn output(&self, peer: &str, thread: u64, bytes: Vec<u8>) {
        let inner = self.lock();
        for id in inner.routed(peer, thread, Kind::Terminal) {
            let _ = inner.ui.unbounded_send(Event::TerminalOutput {
                id,
                bytes: bytes.clone(),
            });
        }
    }

    fn found(&self, hosts: Vec<Found>) {
        let inner = self.lock();
        let hosts = hosts
            .into_iter()
            .filter(|f| {
                f.name != crate::phone::host_name()
                    && !inner.saved.iter().any(|s| s.id == f.fingerprint)
            })
            .collect();
        inner.to_ui(PeerEvent::Found { hosts });
    }
}

impl Inner {
    fn to_ui(&self, e: PeerEvent) {
        let _ = self.ui.unbounded_send(Event::Peer(e));
    }

    fn send(&self, peer: &str, msg: Message) {
        if let Some(tx) = self.links.get(peer).and_then(|l| l.tx.as_ref()) {
            let _ = tx.unbounded_send(msg);
        }
    }

    fn send_up(&self, peer: &str, up: &Up) {
        if let Ok(json) = serde_json::to_string(up) {
            self.send(peer, Message::Text(json.into()));
        }
    }

    fn routed(&self, peer: &str, thread: u64, kind: Kind) -> Vec<SessionId> {
        self.routes
            .iter()
            .filter(|(_, r)| r.peer == peer && r.thread == thread && r.kind == kind)
            .map(|(id, _)| *id)
            .collect()
    }

    fn drop_routes(&mut self, peer: &str, message: &str) {
        let gone: Vec<SessionId> = self
            .routes
            .iter()
            .filter(|(_, r)| r.peer == peer)
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.routes.remove(&id);
            let _ = self.ui.unbounded_send(Event::Failed {
                id,
                message: message.into(),
            });
        }
    }

    fn send_peers(&self) {
        let peers = self
            .saved
            .iter()
            .map(|s| {
                let link = self.links.get(&s.id);
                Peer {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    online: link.is_some_and(|l| l.tx.is_some()),
                    error: link.and_then(|l| l.error.clone()),
                    version: link.map(|l| l.version.clone()).unwrap_or_default(),
                }
            })
            .collect();
        self.to_ui(PeerEvent::Peers { peers });
    }

    fn save(&self) {
        let Ok(json) = serde_json::to_vec_pretty(&self.saved) else {
            return;
        };
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}
