//! The phone bridge. Off until the user switches it on in Settings; then the engine listens for
//! paired phones on a TLS WebSocket (`server`) and relays what they need: the UI's thread list
//! (`Board`), a structured thread's journal as it is written, and a terminal thread's screen
//! from an emulator of its own (`mirror`). What a phone asks the app to do goes to the UI,
//! which does it the way a click would. Shape and reasons: docs/internals/phone.md.

mod idle;
mod mirror;
mod net;
mod server;
pub(crate) mod squeeze;
mod store;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::phone::{
    Ask, Board, BoardKind, Down, Network, PROTOCOL, Pairing, PhoneCommand, PhoneEvent, PhoneStatus,
    Up,
};
use hyprspace_proto::{Entry, Event, FolderCommand, SessionId};
use tokio::task::JoinHandle;

use crate::folder::Folders;
use crate::journal::{self, Journal};
use crate::pty::PtyManager;
use mirror::{Mirror, Ring, Sent};
pub(crate) use net::SERVICE;
use store::Store;

/// How long a pairing code works.
const PAIR_FOR: Duration = Duration::from_secs(5 * 60);
/// Wrong codes a pairing takes before it stops working.
const PAIR_TRIES: u8 = 5;
/// How long a terminal keeps a phone's width after the phone stops showing it.
const LEAVE: Duration = Duration::from_secs(20);
/// How often a watched terminal's screen goes out.
const FRAME: Duration = Duration::from_millis(50);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The engine's handle on the bridge. Cheap to clone.
#[derive(Clone)]
pub struct Phone(Arc<Mutex<Hub>>);

struct Hub {
    ui: UnboundedSender<Event>,
    ptys: PtyManager,
    folders: Folders,
    store: Store,
    journals_dir: PathBuf,
    on: bool,
    network: Network,
    port: u16,
    fingerprint: String,
    error: Option<String>,
    tasks: Vec<JoinHandle<()>>,
    /// The bridge's announcement on the local network, while it listens.
    mdns: Option<mdns_sd::ServiceDaemon>,
    pairing: Option<Secret>,
    /// When recent pairings and hellos failed, so a phone guessing codes slows to a stop.
    failures: VecDeque<Instant>,
    next_conn: u64,
    conns: HashMap<u64, Conn>,
    board: Board,
    /// Frames sent since the last look at whether someone is at the computer.
    ticks: u32,
    sessions: HashMap<u64, SessionId>,
    threads: HashMap<SessionId, u64>,
    terms: HashMap<SessionId, Term>,
    journals: HashMap<String, Weak<Journal>>,
}

struct Secret {
    long: String,
    code: String,
    expires: u64,
    tries: u8,
}

struct Conn {
    device: String,
    name: String,
    computer: bool,
    tx: UnboundedSender<Down>,
    watching: HashMap<u64, Watch>,
    attached: HashSet<u64>,
    /// The size a computer last asked for each terminal, which it gets back when it types.
    sizes: HashMap<u64, (u16, u16)>,
}

#[derive(Default)]
struct Watch {
    sent: Sent,
    /// Cleared when the phone stops watching, which ends its journal watcher.
    live: Arc<AtomicBool>,
    /// A structured thread's journal is coming or came; entries follow it.
    journal: bool,
}

/// A terminal session, as the bridge keeps it.
struct Term {
    ring: Ring,
    /// The size the desktop asked for last.
    desktop: (u16, u16),
    /// The size the PTY has.
    size: (u16, u16),
    mirror: Option<Mirror>,
    /// The connection the PTY is sized for, when a phone or another computer is.
    fit: Option<u64>,
    /// A computer has both sides of the size; a phone only the width.
    full: bool,
}

impl Phone {
    pub fn new(
        ui: UnboundedSender<Event>,
        ptys: PtyManager,
        folders: Folders,
        dir: PathBuf,
    ) -> Self {
        Self(Arc::new(Mutex::new(Hub {
            ui,
            ptys,
            folders,
            store: Store::load(&dir),
            journals_dir: dir.join("journals"),
            on: false,
            network: Network::Everywhere,
            port: 0,
            fingerprint: String::new(),
            error: None,
            tasks: Vec::new(),
            mdns: None,
            pairing: None,
            failures: VecDeque::new(),
            next_conn: 1,
            conns: HashMap::new(),
            board: Board::default(),
            ticks: 0,
            sessions: HashMap::new(),
            threads: HashMap::new(),
            terms: HashMap::new(),
            journals: HashMap::new(),
        })))
    }

    fn hub(&self) -> MutexGuard<'_, Hub> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn command(&self, cmd: PhoneCommand) {
        match cmd {
            PhoneCommand::Enable { on, network } => self.enable(on, network),
            PhoneCommand::Board { mut board } => {
                let mut hub = self.hub();
                board.present = hub.board.present;
                if hub.board != *board {
                    hub.board = (*board).clone();
                    hub.broadcast(Down::Board { board });
                }
            }
            PhoneCommand::Sessions { sessions } => {
                let mut hub = self.hub();
                hub.threads = sessions.iter().map(|(t, s)| (*s, *t)).collect();
                hub.sessions = sessions.into_iter().collect();
            }
            PhoneCommand::Started {
                to,
                request,
                thread,
            } => {
                if let Some(c) = self.hub().conns.get(&to) {
                    let _ = c.tx.unbounded_send(Down::Started { request, thread });
                }
            }
            PhoneCommand::Pair => {
                // a code lives five minutes; while the screen shows it, a fresh one takes over
                let mut hub = self.hub();
                hub.pairing = Some(Secret::new());
                hub.send_pairing();
                let first = hub.pairing.as_ref().map(|p| p.long.clone());
                drop(hub);
                let me = self.clone();
                tokio::spawn(async move {
                    let mut current = first;
                    loop {
                        tokio::time::sleep(PAIR_FOR).await;
                        let mut hub = me.hub();
                        if hub.pairing.as_ref().map(|p| &p.long) != current.as_ref() {
                            return;
                        }
                        hub.pairing = Some(Secret::new());
                        current = hub.pairing.as_ref().map(|p| p.long.clone());
                        hub.send_pairing();
                    }
                });
            }
            PhoneCommand::StopPairing => {
                let mut hub = self.hub();
                hub.pairing = None;
                hub.send_pairing();
            }
            PhoneCommand::Forget { device } => {
                let mut hub = self.hub();
                hub.store.forget(&device);
                let gone: Vec<u64> = hub
                    .conns
                    .iter()
                    .filter(|(_, c)| c.device == device)
                    .map(|(id, _)| *id)
                    .collect();
                for id in gone {
                    if let Some(c) = hub.conns.get(&id) {
                        let _ = c.tx.unbounded_send(Down::Denied {
                            message:
                                "This computer revoked this phone's access. Pair again to connect."
                                    .into(),
                            forget: true,
                        });
                    }
                    hub.drop_conn(id);
                }
                hub.send_status();
            }
            PhoneCommand::Take { id } => self.hub().unfit(id),
        }
    }

    fn enable(&self, on: bool, network: Network) {
        let mut hub = self.hub();
        if hub.on == on && hub.network == network && (on || hub.tasks.is_empty()) {
            return;
        }
        hub.stop();
        hub.network = network;
        if on {
            match self.start(&mut hub) {
                Ok(()) => hub.on = true,
                Err(e) => {
                    hub.stop();
                    hub.error = Some(e);
                }
            }
        }
        hub.send_status();
    }

    /// Binds the listeners and starts the frame ticker. A port someone else holds gives way
    /// to one the OS picks, which phones learn from the next QR code.
    fn start(&self, hub: &mut Hub) -> Result<(), String> {
        let id = hub
            .store
            .identity()
            .map_err(|e| format!("Couldn't make the certificate: {e}"))?;
        let tls = server::acceptor(&id).map_err(|e| format!("Couldn't set up TLS: {e}"))?;
        hub.fingerprint = id.fingerprint;
        let only_ts = hub.network == Network::Tailscale;
        let binds = net::binds(only_ts);
        if binds.is_empty() {
            return Err("This computer isn't on Tailscale.".into());
        }
        let mut port = hub.store.port();
        let mut listeners = Vec::new();
        for ip in &binds {
            let std = match std::net::TcpListener::bind((*ip, port)) {
                Ok(l) => l,
                Err(_) if listeners.is_empty() => std::net::TcpListener::bind((*ip, 0))
                    .map_err(|e| format!("Couldn't listen on {ip}: {e}"))?,
                Err(e) => return Err(format!("Couldn't listen on {ip}: {e}")),
            };
            port = std.local_addr().map_err(|e| e.to_string())?.port();
            std.set_nonblocking(true).map_err(|e| e.to_string())?;
            let l = tokio::net::TcpListener::from_std(std).map_err(|e| e.to_string())?;
            listeners.push(l);
        }
        hub.store.set_port(port);
        hub.port = port;
        if !only_ts {
            hub.mdns = net::announce(port, &hub.fingerprint, &host_name());
        }
        for l in listeners {
            hub.tasks
                .push(tokio::spawn(server::listen(self.clone(), l, tls.clone())));
        }
        let me = self.clone();
        hub.tasks.push(tokio::spawn(async move {
            let mut every = tokio::time::interval(FRAME);
            loop {
                every.tick().await;
                me.frames();
            }
        }));
        Ok(())
    }

    /// The first message on a connection: a hello from a paired phone or a pairing. Returns
    /// the connection's id and what to tell the phone, or why it isn't let in.
    fn admit(&self, up: Up, tx: UnboundedSender<Down>) -> Result<(u64, Down), Down> {
        let denied = |message: &str| Down::Denied {
            message: message.into(),
            forget: false,
        };
        let mut hub = self.hub();
        let now = now_ms();
        let cutoff = Instant::now() - Duration::from_secs(60);
        while hub.failures.front().is_some_and(|t| *t < cutoff) {
            hub.failures.pop_front();
        }
        if hub.failures.len() >= 10 {
            return Err(denied("Too many tries. Wait a minute and try again."));
        }
        let (name, computer) = match &up {
            Up::Hello {
                device, computer, ..
            }
            | Up::Pair {
                device, computer, ..
            } => (clip(device), *computer),
            _ => (String::new(), false),
        };
        let (protocol, device, token, proof) = match up {
            Up::Hello {
                token,
                device,
                protocol,
                app,
                ..
            } => {
                if protocol != PROTOCOL {
                    return Err(denied(&outdated(protocol)));
                }
                let device = clip(&device);
                match hub.store.check(&token, &device, &clip(&app), now) {
                    Some(id) => (protocol, id, None, None),
                    None => {
                        hub.failures.push_back(Instant::now());
                        return Err(Down::Denied {
                            message:
                                "This computer revoked this phone's access. Pair again to connect."
                                    .into(),
                            forget: true,
                        });
                    }
                }
            }
            Up::Pair {
                proof,
                device,
                protocol,
                app,
                ..
            } => {
                if protocol != PROTOCOL {
                    return Err(denied(&outdated(protocol)));
                }
                let fingerprint = hub.fingerprint.clone();
                let key = match &mut hub.pairing {
                    Some(p) if p.expires > now => {
                        let key = [&p.long, &p.code]
                            .into_iter()
                            .find(|key| same(&prove(key, &fingerprint), &proof))
                            .cloned();
                        if key.is_none() {
                            p.tries += 1;
                        }
                        key
                    }
                    _ => None,
                };
                let Some(key) = key else {
                    hub.failures.push_back(Instant::now());
                    if hub.pairing.as_ref().is_some_and(|p| p.tries >= PAIR_TRIES) {
                        hub.pairing = None;
                        hub.send_pairing();
                    }
                    return Err(denied(
                        "That code didn't work. Check it, or show a new one in Settings, Phone.",
                    ));
                };
                hub.pairing = None;
                hub.send_pairing();
                let (id, token) = hub.store.add(&clip(&device), &clip(&app), now, computer);
                let ours = prove(&key, &format!("desktop {fingerprint}"));
                (protocol, id, Some(token), Some(ours))
            }
            _ => return Err(denied("Say hello first.")),
        };
        let _ = protocol;
        let conn = hub.next_conn;
        hub.next_conn += 1;
        hub.board.present = idle::present();
        let board = Box::new(hub.board.clone());
        let _ = tx.unbounded_send(Down::Board { board });
        hub.conns.insert(
            conn,
            Conn {
                device,
                name,
                computer,
                tx,
                watching: HashMap::new(),
                attached: HashSet::new(),
                sizes: HashMap::new(),
            },
        );
        hub.send_status();
        let desktop = host_name();
        Ok((
            conn,
            Down::Welcome {
                desktop,
                version: env!("CARGO_PKG_VERSION").into(),
                token,
                proof,
            },
        ))
    }

    fn left(&self, conn: u64) {
        let mut hub = self.hub();
        hub.drop_conn(conn);
        hub.send_status();
    }

    /// Everything a phone sends after it is in.
    fn handle(&self, conn: u64, up: Up) {
        match up {
            Up::Watch { thread } => self.watch(conn, thread),
            Up::Unwatch { thread } => {
                let mut hub = self.hub();
                hub.unwatch(conn, thread, true);
                hub.send_watching();
                drop(hub);
                self.release_later(conn, thread);
            }
            Up::Attach { thread } => self.attach(conn, thread),
            Up::Detach { thread } => {
                let mut hub = self.hub();
                if let Some(c) = hub.conns.get_mut(&conn) {
                    c.attached.remove(&thread);
                }
                hub.send_watching();
                drop(hub);
                self.release_later(conn, thread);
            }
            Up::Size { thread, cols, rows } => self.hub().size(conn, thread, cols, rows),
            Up::Folder { cmd } => self.folder(conn, cmd),
            Up::Fit { thread, cols, rows } => self.hub().fit(conn, thread, cols, rows),
            Up::Unfit { thread } => {
                let mut hub = self.hub();
                if let Some(&id) = hub.sessions.get(&thread)
                    && hub.terms.get(&id).is_some_and(|t| t.fit == Some(conn))
                {
                    hub.unfit(id);
                }
            }
            Up::Keys { thread, text } => self.keys(thread, text.into_bytes()),
            Up::Paste { thread, text } => {
                let paste = {
                    let hub = self.hub();
                    hub.sessions
                        .get(&thread)
                        .and_then(|id| hub.terms.get(id))
                        .and_then(|t| t.mirror.as_ref())
                        .is_some_and(|m| m.bracketed_paste())
                };
                let mut bytes = Vec::new();
                if paste {
                    bytes.extend_from_slice(b"\x1b[200~");
                    // a paste can't end the paste early
                    bytes.extend(text.replace("\x1b[201~", "").into_bytes());
                    bytes.extend_from_slice(b"\x1b[201~");
                } else {
                    bytes.extend(text.replace('\n', "\r").into_bytes());
                }
                self.keys(thread, bytes);
            }
            Up::Ask { ask } => {
                let hub = self.hub();
                let known = |t: u64| hub.board.threads.iter().any(|b| b.id == t);
                let ok = match &ask {
                    Ask::New {
                        folder: Some(_), ..
                    } => true,
                    Ask::New { space, .. } => hub.board.spaces.iter().any(|s| s.id == *space),
                    Ask::Send { thread, images, .. } => {
                        known(*thread) && images.iter().all(|p| is_upload(Path::new(p)))
                    }
                    Ask::Approve { thread, .. }
                    | Ask::Interrupt { thread }
                    | Ask::Snooze { thread, .. }
                    | Ask::Settle { thread, .. }
                    | Ask::Model { thread, .. }
                    | Ask::Permission { thread, .. }
                    | Ask::Rename { thread, .. }
                    | Ask::Pin { thread, .. } => known(*thread),
                };
                if ok {
                    let _ = hub
                        .ui
                        .unbounded_send(Event::Phone(PhoneEvent::Ask { ask, from: conn }));
                } else if let Some(c) = hub.conns.get(&conn) {
                    let _ = c.tx.unbounded_send(Down::Failed {
                        thread: None,
                        message: "That thread or space is gone.".into(),
                    });
                }
            }
            Up::Ping => {
                if let Some(c) = self.hub().conns.get(&conn) {
                    let _ = c.tx.unbounded_send(Down::Pong);
                }
            }
            Up::Upload { id, data, name } => {
                let me = self.clone();
                tokio::task::spawn_blocking(move || {
                    let (path, error) = match save_upload(&data, name.as_deref()) {
                        Ok(p) => (Some(p.display().to_string()), None),
                        Err(e) => (None, Some(e.to_string())),
                    };
                    if let Some(c) = me.hub().conns.get(&conn) {
                        let _ = c.tx.unbounded_send(Down::Uploaded { id, path, error });
                    }
                });
            }
            Up::Folders { path } => {
                let me = self.clone();
                tokio::task::spawn_blocking(move || {
                    let path = match path.trim() {
                        "" => crate::util::home_dir(),
                        p => PathBuf::from(p),
                    };
                    let dirs = crate::folder::list_dir(&path)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|e| e.dir && !e.name.starts_with('.'))
                        .map(|e| path.join(e.name).display().to_string())
                        .collect();
                    let down = Down::Folders {
                        path: path.display().to_string(),
                        parent: path.parent().map(|p| p.display().to_string()),
                        dirs,
                    };
                    if let Some(c) = me.hub().conns.get(&conn) {
                        let _ = c.tx.unbounded_send(down);
                    }
                });
            }
            Up::Leave => {
                let mut hub = self.hub();
                if let Some(device) = hub.conns.get(&conn).map(|c| c.device.clone()) {
                    hub.store.forget(&device);
                    hub.drop_conn(conn);
                    hub.send_status();
                }
            }
            Up::Hello { .. } | Up::Pair { .. } => {}
        }
    }

    /// A phone hopping between threads mustn't resize the terminal each time, so the size it
    /// asked for holds a little while after it leaves.
    fn release_later(&self, conn: u64, thread: u64) {
        let me = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(LEAVE).await;
            let mut hub = me.hub();
            let back = hub
                .conns
                .get(&conn)
                .is_some_and(|c| c.watching.contains_key(&thread) || c.attached.contains(&thread));
            if let Some(&id) = hub.sessions.get(&thread)
                && !back
                && hub.terms.get(&id).is_some_and(|t| t.fit == Some(conn))
            {
                hub.unfit(id);
            }
        });
    }

    /// Keystrokes from a computer attached to the thread. It takes the terminal's size back if
    /// someone at the host took it.
    pub(super) fn input(&self, conn: u64, thread: u64, bytes: &[u8]) {
        let (ptys, id) = {
            let mut hub = self.hub();
            if !hub
                .conns
                .get(&conn)
                .is_some_and(|c| c.computer && c.attached.contains(&thread))
            {
                return;
            }
            let Some(&id) = hub.sessions.get(&thread) else {
                return;
            };
            if hub.terms.get(&id).is_some_and(|t| t.fit != Some(conn)) {
                hub.take_size(conn, thread);
            }
            (hub.ptys.clone(), id)
        };
        ptys.write(id, bytes);
    }

    fn attach(&self, conn: u64, thread: u64) {
        let mut hub = self.hub();
        let terminal = hub
            .board
            .threads
            .iter()
            .any(|t| t.id == thread && t.kind == BoardKind::Terminal);
        let Hub {
            conns,
            sessions,
            terms,
            ..
        } = &mut *hub;
        let Some(c) = conns.get_mut(&conn).filter(|c| c.computer) else {
            return;
        };
        if !terminal {
            let _ = c.tx.unbounded_send(Down::Failed {
                thread: Some(thread),
                message: "That thread is gone.".into(),
            });
            return;
        }
        // what it printed so far and what it prints from now on, under one lock, so nothing is
        // missed or sent twice
        c.attached.insert(thread);
        if let Some(t) = sessions.get(&thread).and_then(|id| terms.get(id)) {
            let data = t.ring.bytes();
            if !data.is_empty() {
                let _ = c.tx.unbounded_send(Down::Bytes { thread, data });
            }
        }
        hub.send_watching();
    }

    fn folder(&self, conn: u64, cmd: FolderCommand) {
        if matches!(cmd, FolderCommand::Openers | FolderCommand::OpenIn { .. }) {
            return;
        }
        let hub = self.hub();
        let Some(down) = hub
            .conns
            .get(&conn)
            .filter(|c| c.computer)
            .map(|c| c.tx.clone())
        else {
            return;
        };
        let folders = hub.folders.clone();
        drop(hub);
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        folders.handle(cmd, tx);
        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(e) = rx.next().await {
                if let Event::Folder(event) = e
                    && down
                        .unbounded_send(Down::Folder {
                            event: Box::new(event),
                        })
                        .is_err()
                {
                    break;
                }
            }
        });
    }

    fn keys(&self, thread: u64, bytes: Vec<u8>) {
        let (ptys, id) = {
            let hub = self.hub();
            let Some(&id) = hub.sessions.get(&thread) else {
                return;
            };
            (hub.ptys.clone(), id)
        };
        ptys.write(id, &bytes);
    }

    fn watch(&self, conn: u64, thread: u64) {
        let mut hub = self.hub();
        let kind = hub
            .board
            .threads
            .iter()
            .find(|t| t.id == thread)
            .map(|t| t.kind);
        let Some(kind) = kind else {
            if let Some(c) = hub.conns.get(&conn) {
                let _ = c.tx.unbounded_send(Down::Failed {
                    thread: Some(thread),
                    message: "That thread is gone.".into(),
                });
            }
            return;
        };
        let structured = kind == BoardKind::Structured;
        let live = Arc::new(AtomicBool::new(true));
        let Some(c) = hub.conns.get_mut(&conn) else {
            return;
        };
        // a second watch starts the thread over, the way the first did
        let old = c.watching.insert(
            thread,
            Watch {
                live: live.clone(),
                journal: structured,
                ..Watch::default()
            },
        );
        if let Some(old) = old {
            old.live.store(false, Ordering::Relaxed);
        }
        let tx = c.tx.clone();
        if !structured {
            hub.send_watching();
            return;
        }
        let name = format!("thread-{thread}");
        let journal = hub.journals.get(&name).and_then(Weak::upgrade);
        let file = journal::path(&hub.journals_dir, &name);
        drop(hub);
        tokio::task::spawn_blocking(move || {
            let send = |entries| {
                let _ = tx.unbounded_send(Down::Transcript {
                    thread,
                    entries,
                    reset: true,
                });
            };
            match journal {
                Some(j) => j.watch(Some(Box::new(send)), watcher(thread, live, tx.clone())),
                None => send(journal::load(&file)),
            }
        });
    }

    /// A structured session opened with a journal. Phones already watching its thread hear
    /// what it records from now on.
    pub fn journal_opened(&self, name: &str, journal: &Arc<Journal>) {
        let mut hub = self.hub();
        hub.journals.retain(|_, j| j.strong_count() > 0);
        hub.journals
            .insert(name.to_string(), Arc::downgrade(journal));
        let Some(thread) = name
            .strip_prefix("thread-")
            .and_then(|n| n.parse::<u64>().ok())
        else {
            return;
        };
        for c in hub.conns.values() {
            if let Some(w) = c.watching.get(&thread).filter(|w| w.journal) {
                journal.watch(None, watcher(thread, w.live.clone(), c.tx.clone()));
            }
        }
    }

    pub fn opened_terminal(&self, id: SessionId, cols: u16, rows: u16) {
        self.hub().terms.insert(
            id,
            Term {
                ring: Ring::default(),
                desktop: (cols, rows),
                size: (cols, rows),
                mirror: None,
                fit: None,
                full: false,
            },
        );
    }

    /// The desktop resized a terminal. Returns whether the PTY should follow as asked. While a
    /// phone has the width, the height still follows the desktop, and this does it.
    pub fn resize(&self, id: SessionId, cols: u16, rows: u16) -> bool {
        let mut hub = self.hub();
        let ptys = hub.ptys.clone();
        let Some(t) = hub.terms.get_mut(&id) else {
            return true;
        };
        t.desktop = (cols, rows);
        if t.fit.is_some() && t.full {
            return false;
        }
        if t.fit.is_some() {
            if t.size.1 != rows {
                t.size.1 = rows;
                if let Some(m) = &mut t.mirror {
                    m.resize(t.size.0, rows);
                }
                let _ = ptys.resize(id, t.size.0, rows);
            }
            return false;
        }
        t.size = (cols, rows);
        if let Some(m) = &mut t.mirror {
            m.resize(cols, rows);
        }
        true
    }

    /// Every event on its way to the UI passes here: terminal output feeds the mirrors.
    pub fn tap(&self, event: &Event) {
        match event {
            Event::TerminalOutput { id, bytes } => {
                let mut hub = self.hub();
                if let Some(t) = hub.terms.get_mut(id) {
                    t.ring.push(bytes);
                    if let Some(m) = &mut t.mirror {
                        m.feed(bytes);
                    }
                }
                if let Some(&thread) = hub.threads.get(id) {
                    for c in hub.conns.values().filter(|c| c.attached.contains(&thread)) {
                        let _ = c.tx.unbounded_send(Down::Bytes {
                            thread,
                            data: bytes.clone(),
                        });
                    }
                }
            }
            Event::TerminalExit { id, code } => {
                let mut hub = self.hub();
                hub.terms.remove(id);
                if let Some(&thread) = hub.threads.get(id) {
                    for c in hub.conns.values().filter(|c| c.attached.contains(&thread)) {
                        let _ = c.tx.unbounded_send(Down::Exited {
                            thread,
                            code: *code,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// Sends each watched terminal's changes, and every couple of seconds whether someone is at
    /// the computer, when that changed.
    fn frames(&self) {
        let mut hub = self.hub();
        hub.ticks += 1;
        if hub.ticks >= 40 {
            hub.ticks = 0;
            let present = idle::present();
            if present != hub.board.present && !hub.conns.is_empty() {
                hub.board.present = present;
                let board = Box::new(hub.board.clone());
                hub.broadcast(Down::Board { board });
            }
        }
        let Hub {
            conns,
            sessions,
            terms,
            ..
        } = &mut *hub;
        for (conn, c) in conns.iter_mut() {
            for (thread, w) in c.watching.iter_mut().filter(|(_, w)| !w.journal) {
                let Some(t) = sessions.get(thread).and_then(|id| terms.get_mut(id)) else {
                    continue;
                };
                let (cols, rows) = t.size;
                let fit = t.fit == Some(*conn);
                let mirror = t
                    .mirror
                    .get_or_insert_with(|| Mirror::new(cols, rows, &t.ring));
                if let Some(frame) = mirror.frame(*thread, &mut w.sent, fit) {
                    let _ = c.tx.unbounded_send(Down::Term { frame });
                }
            }
        }
    }
}

impl Hub {
    fn broadcast(&self, down: Down) {
        for c in self.conns.values() {
            let _ = c.tx.unbounded_send(down.clone());
        }
    }

    fn to_ui(&self, e: PhoneEvent) {
        let _ = self.ui.unbounded_send(Event::Phone(e));
    }

    fn send_status(&self) {
        let online: HashSet<&str> = self.conns.values().map(|c| c.device.as_str()).collect();
        let status = PhoneStatus {
            on: self.on,
            port: self.port,
            addresses: if self.on {
                net::addresses(self.network == Network::Tailscale)
                    .iter()
                    .map(|ip| ip.to_string())
                    .collect()
            } else {
                Vec::new()
            },
            error: self.error.clone(),
            devices: self.store.devices(|id| online.contains(id)),
        };
        self.to_ui(PhoneEvent::Status { status });
    }

    fn send_pairing(&self) {
        let pairing = self.pairing.as_ref().filter(|_| self.on).map(|p| {
            let hosts: Vec<String> = net::addresses(self.network == Network::Tailscale)
                .iter()
                .map(|ip| ip.to_string())
                .collect();
            Pairing {
                link: format!(
                    "hyprspace://pair?n={}&h={}&p={}&f={}&c={}",
                    encode(&host_name()),
                    hosts.join(","),
                    self.port,
                    self.fingerprint,
                    p.long
                ),
                code: format!("{}-{}-{}", &p.code[..4], &p.code[4..8], &p.code[8..]),
                expires: p.expires,
            }
        });
        self.to_ui(PhoneEvent::Pairing { pairing });
    }

    /// The terminal threads phones watch, for the UI to start any that isn't running.
    fn send_watching(&self) {
        let mut threads: Vec<u64> = self
            .conns
            .values()
            .flat_map(|c| {
                c.watching
                    .iter()
                    .filter(|(_, w)| !w.journal)
                    .map(|(t, _)| *t)
                    .chain(c.attached.iter().copied())
            })
            .collect();
        threads.sort();
        threads.dedup();
        self.to_ui(PhoneEvent::Watching { threads });
    }

    /// Stops a phone watching a thread. A terminal sized for it goes back to the desktop's size,
    /// now or, with `later`, when the caller says.
    fn unwatch(&mut self, conn: u64, thread: u64, later: bool) {
        let Some(c) = self.conns.get_mut(&conn) else {
            return;
        };
        if let Some(w) = c.watching.remove(&thread) {
            w.live.store(false, Ordering::Relaxed);
        }
        if let Some(&id) = self.sessions.get(&thread)
            && !later
            && self.terms.get(&id).is_some_and(|t| t.fit == Some(conn))
        {
            self.unfit(id);
        }
    }

    fn drop_conn(&mut self, conn: u64) {
        let threads: Vec<u64> = self
            .conns
            .get(&conn)
            .map(|c| c.watching.keys().chain(&c.attached).copied().collect())
            .unwrap_or_default();
        for t in threads {
            self.unwatch(conn, t, false);
        }
        // dropping the sender ends the connection's task
        self.conns.remove(&conn);
        self.send_watching();
    }

    fn fit(&mut self, conn: u64, thread: u64, cols: u16, rows: u16) {
        let Some(&id) = self.sessions.get(&thread) else {
            return;
        };
        let Some(t) = self.terms.get_mut(&id) else {
            return;
        };
        // Only the width follows the phone; the rows stay the desktop's. ConPTY repaints its
        // whole screen when the height changes and overwrites the lines the emulator just pulled
        // down from scrollback, so every height change lost lines. The phone scrolls instead.
        let _ = rows;
        let (cols, rows) = (cols.clamp(20, 500), t.desktop.1.max(5));
        let first = t.fit != Some(conn);
        t.fit = Some(conn);
        t.full = false;
        if t.size != (cols, rows) {
            t.size = (cols, rows);
            if let Some(m) = &mut t.mirror {
                m.resize(cols, rows);
            }
            let _ = self.ptys.resize(id, cols, rows);
        }
        if first {
            let by = self.name(conn);
            self.to_ui(PhoneEvent::Fit {
                id,
                phone: true,
                by,
            });
        }
    }

    fn name(&self, conn: u64) -> String {
        self.conns
            .get(&conn)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    fn size(&mut self, conn: u64, thread: u64, cols: u16, rows: u16) {
        let Some(c) = self.conns.get_mut(&conn).filter(|c| c.computer) else {
            return;
        };
        c.sizes
            .insert(thread, (cols.clamp(20, 500), rows.clamp(5, 300)));
        self.take_size(conn, thread);
    }

    /// The terminal at the size `conn` last asked for, both ways.
    fn take_size(&mut self, conn: u64, thread: u64) {
        let Some(&(cols, rows)) = self.conns.get(&conn).and_then(|c| c.sizes.get(&thread)) else {
            return;
        };
        let by = self.name(conn);
        let Some(&id) = self.sessions.get(&thread) else {
            return;
        };
        let Some(t) = self.terms.get_mut(&id) else {
            return;
        };
        let first = t.fit != Some(conn);
        t.fit = Some(conn);
        t.full = true;
        if t.size != (cols, rows) {
            t.size = (cols, rows);
            if let Some(m) = &mut t.mirror {
                m.resize(cols, rows);
            }
            let _ = self.ptys.resize(id, cols, rows);
        }
        if first {
            self.to_ui(PhoneEvent::Fit {
                id,
                phone: true,
                by,
            });
        }
    }

    /// The terminal goes back to the size the desktop asked for.
    fn unfit(&mut self, id: SessionId) {
        let Some(t) = self.terms.get_mut(&id) else {
            return;
        };
        if t.fit.take().is_none() {
            return;
        }
        let (cols, rows) = t.desktop;
        t.size = (cols, rows);
        if let Some(m) = &mut t.mirror {
            m.resize(cols, rows);
        }
        t.full = false;
        let _ = self.ptys.resize(id, cols, rows);
        self.to_ui(PhoneEvent::Fit {
            id,
            phone: false,
            by: String::new(),
        });
    }

    /// Drops every connection and listener. Paired phones stay paired.
    fn stop(&mut self) {
        for t in self.tasks.drain(..) {
            t.abort();
        }
        if let Some(d) = self.mdns.take() {
            let _ = d.shutdown();
        }
        let conns: Vec<u64> = self.conns.keys().copied().collect();
        for c in conns {
            self.drop_conn(c);
        }
        for t in self.terms.values_mut() {
            t.mirror = None;
        }
        self.on = false;
        self.port = 0;
        self.error = None;
        self.pairing = None;
        self.send_pairing();
    }
}

fn watcher(thread: u64, live: Arc<AtomicBool>, tx: UnboundedSender<Down>) -> journal::Watcher {
    Box::new(move |e: &Entry| {
        live.load(Ordering::Relaxed)
            && tx
                .unbounded_send(Down::Transcript {
                    thread,
                    entries: vec![e.clone()],
                    reset: false,
                })
                .is_ok()
    })
}

fn photos() -> PathBuf {
    std::env::temp_dir().join("hyprspace-images")
}

fn is_upload(path: &Path) -> bool {
    path.parent() == Some(photos().as_path())
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("phone-") && !n.contains(".."))
}

fn save_upload(data: &str, name: Option<&str>) -> Result<PathBuf, &'static str> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| "That file didn't arrive whole.")?;
    let ext = match bytes.as_slice() {
        _ if name.is_some() => "",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [0x89, b'P', b'N', b'G', ..] => "png",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "webp",
        _ => return Err("Only photos can be sent."),
    };
    let dir = photos();
    std::fs::create_dir_all(&dir).map_err(|_| "Couldn't save the file.")?;
    let tag: String = store::random(8)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let path = match name {
        Some(n) => dir.join(format!("phone-{tag}-{}", clean(n))),
        None => dir.join(format!("phone-{tag}.{ext}")),
    };
    std::fs::write(&path, bytes).map_err(|_| "Couldn't save the file.")?;
    Ok(path)
}

fn clean(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let safe: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    match safe.trim_matches('.') {
        "" => "file".into(),
        s => s.replace("..", "_"),
    }
}

/// What a phone sends for `key` over the certificate it saw: HMAC-SHA256, base64url.
pub(crate) fn prove(key: &str, fingerprint: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut m = Hmac::<sha2::Sha256>::new_from_slice(key.as_bytes()).expect("any key fits");
    m.update(fingerprint.as_bytes());
    URL_SAFE_NO_PAD.encode(m.finalize().into_bytes())
}

/// Equal in time that doesn't depend on where they differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

impl Secret {
    fn new() -> Self {
        Self {
            long: URL_SAFE_NO_PAD.encode(store::random(18)),
            code: short_code(),
            expires: now_ms() + PAIR_FOR.as_millis() as u64,
            tries: 0,
        }
    }
}

/// Twelve characters from an alphabet with no look-alikes (no 0 and O, no 1, I and L): about
/// 60 bits, past cracking a captured proof in the minutes a code lives.
fn short_code() -> String {
    const ABC: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    store::random(12)
        .iter()
        .map(|b| ABC[*b as usize % ABC.len()] as char)
        .collect()
}

fn clip(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(60).collect();
    match name.trim() {
        "" => "Phone".into(),
        n => n.to_string(),
    }
}

fn outdated(protocol: u32) -> String {
    if protocol < PROTOCOL {
        "Update HyprSpace on your phone to connect to this computer.".into()
    } else {
        "Update HyprSpace on this computer to connect your phone.".into()
    }
}

pub(crate) fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "This computer".into())
}

/// Percent-encodes everything but letters, digits and `-._~`.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests;
