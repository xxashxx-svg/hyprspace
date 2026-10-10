// Other computers this one is paired with (docs/internals/machines.md): their threads mirrored
// into the sidebar from each host's board, and what is done to them sent back as asks. Settings'
// Computers part is settings/machines.rs.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{AppContext, Context, Focusable, Window};
use hyprspace_proto::agents::AgentInfo;
use hyprspace_proto::peer::{Found, Peer, PeerCommand, PeerEvent};
use hyprspace_proto::phone::{Ask, Board, BoardKind, BoardStatus, Down, NewThread, Shelf, Up};
use hyprspace_proto::{Agent, Command, Launch, Snooze, Space, Thread, ThreadKind};

use crate::composer::machine::Machine;
use crate::folders::{FolderPicker, PickerEvent};
use crate::root::{Root, Screen, Start, folder_name, same_folder};
use crate::time::now_ms;
use crate::transcript::Status;

pub(crate) const FIRST: u64 = 1 << 52;

#[derive(Default)]
pub struct Machines {
    pub peers: Vec<Peer>,
    pub found: Vec<Found>,
    pub error: Option<String>,
    ids: HashMap<(String, bool, u64), u64>,
    threads: HashMap<u64, (String, u64)>,
    spaces: HashMap<u64, (String, Option<u64>)>,
    places: HashMap<u64, (String, bool)>,
    agents: HashMap<String, Vec<AgentInfo>>,
    pending: Vec<(String, u64, PathBuf)>,
    next: u64,
    next_request: u64,
    asked: HashSet<u64>,
    open_next: Option<(String, u64)>,
    browse: Option<String>,
    pub(crate) input: Option<gpui::Entity<crate::input::TextInput>>,
    pub(crate) chosen: Option<Found>,
    looking: bool,
}

impl Machines {
    fn take(&mut self) -> u64 {
        self.next += 1;
        FIRST + self.next
    }

    fn local(&mut self, peer: &str, thread: bool, remote: u64) -> u64 {
        let key = (peer.to_string(), thread, remote);
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = self.take();
        self.ids.insert(key, id);
        if thread {
            self.threads.insert(id, (peer.to_string(), remote));
        } else {
            self.spaces.insert(id, (peer.to_string(), Some(remote)));
        }
        id
    }

    pub fn remote(&self, thread: u64) -> Option<(String, u64)> {
        self.threads.get(&thread).cloned()
    }

    pub fn space(&self, space: u64) -> Option<(String, Option<u64>)> {
        self.spaces.get(&space).cloned()
    }

    pub fn name(&self, peer: &str) -> String {
        self.peers
            .iter()
            .find(|p| p.id == peer)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }

    pub fn online(&self, peer: &str) -> bool {
        self.peers.iter().any(|p| p.id == peer && p.online)
    }

    pub fn place(&self, thread: u64) -> Option<&(String, bool)> {
        self.places.get(&thread)
    }

    pub fn agents(&self, peer: &str) -> &[AgentInfo] {
        self.agents.get(peer).map_or(&[], Vec::as_slice)
    }

    fn request(&mut self) -> u64 {
        self.next_request += 1;
        self.asked.insert(self.next_request);
        self.next_request
    }
}

fn status(s: BoardStatus) -> Status {
    match s {
        BoardStatus::Idle => Status::Idle,
        BoardStatus::Working => Status::Working,
        BoardStatus::Waiting => Status::Waiting,
        BoardStatus::Done => Status::Done,
        BoardStatus::Failed => Status::Failed,
    }
}

impl Root {
    pub(crate) fn look_for_hosts(&mut self) {
        let want = self.screen == Screen::Settings
            && self.settings.tab() == crate::settings::Tab::Computers;
        if want != self.machines.looking {
            self.machines.looking = want;
            if !want {
                self.machines.found.clear();
            }
            self.client
                .send(Command::Peer(PeerCommand::Look { on: want }));
        }
    }

    pub(crate) fn peer_event(&mut self, e: PeerEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e {
            PeerEvent::Peers { peers } => self.peers(peers, window, cx),
            PeerEvent::PairFailed { message } => self.machines.error = Some(message),
            PeerEvent::Found { hosts } => self.machines.found = hosts,
            PeerEvent::Board { peer, board } => self.mirror(&peer, *board, window, cx),
            PeerEvent::Down { peer, down } => match down {
                Down::Started { request, thread } if self.machines.asked.remove(&request) => {
                    self.machines.open_next = Some((peer, thread));
                    self.open_started(window, cx);
                }
                Down::Folders { path, .. } if self.machines.browse.as_ref() == Some(&peer) => {
                    self.machines.browse = None;
                    self.browse_at(peer, PathBuf::from(path), window, cx);
                }
                _ => {}
            },
            PeerEvent::Folder { peer, event } => self.peer_folder(peer, event, cx),
        }
        self.sidebar_view.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn peers(&mut self, peers: Vec<Peer>, window: &mut Window, cx: &mut Context<Self>) {
        let known: HashSet<String> = peers.iter().map(|p| p.id.clone()).collect();
        let gone: Vec<u64> = self
            .state
            .spaces
            .iter()
            .filter(|s| s.machine.as_ref().is_some_and(|m| !known.contains(m)))
            .flat_map(|s| s.threads.iter().map(|t| t.id))
            .collect();
        self.state
            .spaces
            .retain(|s| s.machine.as_ref().is_none_or(|m| known.contains(m)));
        self.machines.pending.retain(|p| known.contains(&p.0));
        self.lose(&gone, window, cx);
        // a host out of reach can't say how its threads are doing
        let away: Vec<u64> = self
            .state
            .spaces
            .iter()
            .filter(|s| {
                s.machine
                    .as_ref()
                    .is_some_and(|m| !peers.iter().any(|p| &p.id == m && p.online))
            })
            .flat_map(|s| s.threads.iter().map(|t| t.id))
            .collect();
        for id in away {
            self.status.remove(&id);
            self.turns.remove(&id);
            self.activity.remove(&id);
        }
        let machines = peers
            .iter()
            .map(|p| Machine {
                id: p.id.clone(),
                name: p.name.clone(),
                online: p.online,
            })
            .collect();
        self.composer
            .update(cx, |c, cx| c.set_machines(machines, cx));
        self.machines.peers = peers;
        self.machines.error = None;
    }

    fn lose(&mut self, threads: &[u64], window: &mut Window, cx: &mut Context<Self>) {
        for id in threads {
            self.drop_view(*id);
            self.status.remove(id);
            self.unseen.remove(id);
            self.turns.remove(id);
            self.activity.remove(id);
        }
        if !threads.is_empty() {
            self.leave(window, cx);
        }
    }

    fn mirror(&mut self, peer: &str, board: Board, window: &mut Window, cx: &mut Context<Self>) {
        let mine = |s: &Space| s.machine.as_deref() == Some(peer);
        let folded: HashMap<u64, bool> = self
            .state
            .spaces
            .iter()
            .filter(|s| mine(s))
            .map(|s| (s.id, s.folded))
            .collect();
        let before: HashSet<u64> = self
            .state
            .spaces
            .iter()
            .filter(|s| mine(s))
            .flat_map(|s| s.threads.iter().map(|t| t.id))
            .collect();
        self.state.spaces.retain(|s| !mine(s));
        self.machines
            .agents
            .insert(peer.to_string(), board.agents.clone());
        let now = now_ms();
        let mut kept = HashSet::new();
        for bs in &board.spaces {
            let sid = self.machines.local(peer, false, bs.id);
            let cwd = PathBuf::from(&bs.path);
            let mut space = Space {
                id: sid,
                name: bs.name.clone(),
                cwd: Some(cwd.clone()),
                folded: folded.get(&sid).copied().unwrap_or(false),
                machine: Some(peer.to_string()),
                ..Space::default()
            };
            for bt in board.threads.iter().filter(|t| t.space == bs.id) {
                let tid = self.machines.local(peer, true, bt.id);
                kept.insert(tid);
                let launch = |agent: Agent| {
                    let mut l = Launch::new(agent, cwd.clone());
                    l.model = Some(bt.model_id.clone()).filter(|m| !m.is_empty());
                    l.effort = Some(bt.effort.clone()).filter(|e| !e.is_empty());
                    l.permission = bt.permission;
                    l
                };
                let kind = match bt.kind {
                    BoardKind::Structured => ThreadKind::Structured {
                        launch: launch(bt.agent.unwrap_or(Agent::Claude)),
                    },
                    BoardKind::Terminal => ThreadKind::Terminal {
                        cwd: cwd.clone(),
                        run: bt.agent.map(launch),
                    },
                };
                space.threads.push(Thread {
                    id: tid,
                    title: bt.title.clone(),
                    kind,
                    settled: bt.shelf == Shelf::Settled,
                    snooze: (bt.shelf == Shelf::Snoozed).then_some(Snooze::Done),
                    created: bt.touched,
                    touched: bt.touched,
                    order: Some(bt.rank),
                    pinned: bt.pinned.then(|| (i64::MAX - bt.rank).max(0) as u64),
                    ..Thread::default()
                });
                // a structured view reads its status off its own transcript
                if !self.views.contains_key(&tid) || bt.kind == BoardKind::Terminal {
                    self.status.insert(tid, status(bt.status));
                }
                if bt.unseen && self.screen != Screen::Thread(tid) {
                    self.unseen.insert(tid);
                } else {
                    self.unseen.remove(&tid);
                }
                self.activity.entry(tid).or_default().doing = bt.doing.clone();
                match bt.since {
                    Some(since) => {
                        let ago = Duration::from_millis(now.saturating_sub(since));
                        let at = Instant::now().checked_sub(ago).unwrap_or_else(Instant::now);
                        self.turns.insert(tid, at);
                    }
                    None => {
                        self.turns.remove(&tid);
                    }
                }
                self.machines
                    .places
                    .insert(tid, (bt.place.clone(), bt.branch));
            }
            self.state.spaces.push(space);
        }
        let pending: Vec<_> = self
            .machines
            .pending
            .iter()
            .filter(|p| p.0 == peer)
            .cloned()
            .collect();
        for (_, id, path) in pending {
            let real = self
                .state
                .spaces
                .iter()
                .find(|s| mine(s) && s.cwd.as_deref().is_some_and(|c| same_folder(c, &path)))
                .map(|s| s.id);
            match real {
                Some(real) => {
                    self.machines.pending.retain(|p| p.1 != id);
                    self.machines.spaces.remove(&id);
                    if self.screen == Screen::Compose(Some(id)) {
                        self.compose(Some(real), window, cx);
                    }
                }
                None => self.state.spaces.push(Space {
                    id,
                    name: folder_name(&path),
                    cwd: Some(path),
                    machine: Some(peer.to_string()),
                    ..Space::default()
                }),
            }
        }
        let gone: Vec<u64> = before.difference(&kept).copied().collect();
        self.lose(&gone, window, cx);
        if let Screen::Compose(Some(space)) = self.screen
            && self.machines.space(space).is_some_and(|(p, _)| p == peer)
        {
            self.refresh_composer(cx);
        }
        self.open_started(window, cx);
        self.tick(cx);
    }

    fn open_started(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((peer, thread)) = self.machines.open_next.clone() else {
            return;
        };
        if let Some(&id) = self.machines.ids.get(&(peer, true, thread))
            && self.state.thread(id).is_some()
        {
            self.machines.open_next = None;
            self.open_thread(id, window, cx);
        }
    }

    pub(crate) fn ask_host(&mut self, thread: u64, ask: impl FnOnce(u64) -> Ask) -> bool {
        let Some((peer, remote)) = self.machines.remote(thread) else {
            return false;
        };
        self.client.send(Command::Peer(PeerCommand::Up {
            peer,
            up: Up::Ask { ask: ask(remote) },
        }));
        true
    }

    pub(crate) fn start_remote(
        &mut self,
        space: u64,
        start: Start,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((peer, remote)) = self.machines.space(space) else {
            return false;
        };
        if !self.machines.online(&peer) {
            let message = format!("{} is offline.", self.machines.name(&peer));
            self.composer.update(cx, |c, cx| c.fail(message, cx));
            return true;
        }
        let Start {
            launch,
            prompt,
            terminal,
            ..
        } = start;
        let (text, images) = prompt.map(|p| (p.text, p.images)).unwrap_or_default();
        let request = self.machines.request();
        let ask = Ask::New {
            space: remote.unwrap_or(0),
            folder: remote.is_none().then(|| launch.cwd.display().to_string()),
            start: NewThread {
                agent: Some(launch.agent),
                model: launch.model.unwrap_or_default(),
                effort: launch.effort.unwrap_or_default(),
                permission: launch.permission,
                terminal,
                prompt: text,
                images: Vec::new(),
            },
            request: Some(request),
        };
        self.client
            .send(Command::Peer(PeerCommand::New { peer, ask, images }));
        true
    }

    pub(crate) fn remote_terminal(&mut self, space: u64, cx: &mut Context<Self>) {
        let Some((peer, remote)) = self.machines.space(space) else {
            return;
        };
        if !self.machines.online(&peer) {
            return;
        }
        let cwd = self.state.space(space).and_then(|s| s.cwd.clone());
        let request = self.machines.request();
        let ask = Ask::New {
            space: remote.unwrap_or(0),
            folder: remote
                .is_none()
                .then(|| cwd.map(|c| c.display().to_string()))
                .flatten(),
            start: NewThread {
                agent: None,
                model: String::new(),
                effort: String::new(),
                permission: Default::default(),
                terminal: true,
                prompt: String::new(),
                images: Vec::new(),
            },
            request: Some(request),
        };
        self.client.send(Command::Peer(PeerCommand::Up {
            peer,
            up: Up::Ask { ask },
        }));
        cx.notify();
    }

    pub(crate) fn pick_machine(
        &mut self,
        machine: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = match self.screen {
            Screen::Compose(Some(s)) => self.state.space(s).map(|s| s.name.clone()),
            _ => None,
        };
        let on = |s: &&Space| s.machine == machine && !s.archived;
        let same = self
            .state
            .spaces
            .iter()
            .filter(on)
            .find(|s| Some(&s.name) == name.as_ref())
            .map(|s| s.id);
        if let Some(id) = same {
            self.compose(Some(id), window, cx);
            return;
        }
        let Some(peer) = machine.clone() else {
            let first = self.state.spaces.iter().find(on).map(|s| s.id);
            self.compose(first, window, cx);
            return;
        };
        let near = self
            .state
            .spaces
            .iter()
            .filter(on)
            .find_map(|s| s.cwd.as_deref()?.parent().map(PathBuf::from));
        match near {
            Some(start) => self.browse_at(peer, start, window, cx),
            None => {
                self.machines.browse = Some(peer.clone());
                self.client.send(Command::Peer(PeerCommand::Up {
                    peer,
                    up: Up::Folders {
                        path: String::new(),
                    },
                }));
            }
        }
    }

    pub(crate) fn browse_at(
        &mut self,
        peer: String,
        start: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.folder_picker.is_some() {
            return;
        }
        let back = window.focused(cx);
        let client = self.client.clone();
        let machine = Some((peer.clone(), self.machines.name(&peer)));
        let picker =
            cx.new(|cx| FolderPicker::new(client, machine, start.clone(), start, back, cx));
        cx.subscribe_in(
            &picker,
            window,
            move |r, _, e: &PickerEvent, window, cx| match e {
                PickerEvent::Open(path) => {
                    let path = path.clone();
                    r.close_folder_picker(window, cx);
                    r.open_on(peer.clone(), path, window, cx);
                }
                PickerEvent::System | PickerEvent::Close => r.close_folder_picker(window, cx),
            },
        )
        .detach();
        let focus = picker.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.folder_picker = Some(picker);
        self.menu = None;
        cx.notify();
    }

    fn open_on(
        &mut self,
        peer: String,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let known = self
            .state
            .spaces
            .iter()
            .find(|s| {
                s.machine.as_ref() == Some(&peer)
                    && s.cwd.as_deref().is_some_and(|c| same_folder(c, &path))
            })
            .map(|s| s.id);
        let id = match known {
            Some(id) => id,
            None => {
                let id = self.machines.take();
                self.machines.spaces.insert(id, (peer.clone(), None));
                self.machines.pending.push((peer.clone(), id, path.clone()));
                self.state.spaces.push(Space {
                    id,
                    name: folder_name(&path),
                    cwd: Some(path),
                    machine: Some(peer),
                    ..Space::default()
                });
                id
            }
        };
        self.compose(Some(id), window, cx);
    }
}
