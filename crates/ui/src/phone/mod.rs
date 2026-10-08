// The desktop's side of the phone bridge (engine/src/phone): the thread list a paired phone
// sees, built from what the sidebar shows, and what a phone asks for, done the way the click
// it stands for would do it. Settings' Phone view is settings/phone.rs.

pub(crate) mod qr;

use gpui::{App, Context, Window};
use hyprspace_proto::phone::{
    Ask, Board, BoardKind, BoardSpace, BoardStatus, BoardTheme, BoardThread, Pairing, Palette,
    PhoneCommand, PhoneEvent, PhoneStatus, Shelf, StartPrefs,
};
use hyprspace_proto::{Command, Launch, SessionId, ThreadKind};
use hyprspace_theme::Theme;

use crate::root::{Root, Start, View};
use crate::time::now_ms;
use crate::transcript::Status;

#[derive(Default)]
pub struct PhoneState {
    pub status: PhoneStatus,
    pub pairing: Option<Pairing>,
    /// The board and session list the engine has, so an unchanged one isn't sent again.
    sent: Option<Board>,
    sessions: Vec<(u64, SessionId)>,
}

impl PhoneState {
    /// A paired phone is connected now.
    fn anyone(&self) -> bool {
        self.status.devices.iter().any(|d| d.online)
    }
}

impl Root {
    /// Tells the engine whether to run the bridge, from the saved setting.
    pub(crate) fn phone_enable(&self) {
        let p = self.state.phone;
        self.client.send(Command::Phone(PhoneCommand::Enable {
            on: p.on,
            network: p.network,
        }));
    }

    pub(crate) fn phone_event(
        &mut self,
        e: PhoneEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match e {
            PhoneEvent::Status { status } => {
                let joined = status.devices.iter().any(|d| d.online) && !self.phone.anyone();
                self.phone.status = status;
                if joined {
                    // a phone that just came in gets the list without waiting for the tick
                    self.phone.sent = None;
                    self.phone_publish(cx);
                }
            }
            PhoneEvent::Pairing { pairing } => self.phone.pairing = pairing,
            PhoneEvent::Ask { ask } => self.phone_ask(ask, window, cx),
            PhoneEvent::Watching { threads } => {
                // a terminal a phone watches needs its session running
                for id in threads {
                    if self.views.contains_key(&id) {
                        continue;
                    }
                    if let Some((_, t)) = self.state.thread(id) {
                        let t = t.clone();
                        self.make_view(&t, None, true, cx);
                    }
                }
                self.phone_publish(cx);
            }
            PhoneEvent::Fit { id, phone } => {
                if let Some(View::Terminal(v)) =
                    self.sessions.get(&id).and_then(|t| self.views.get(t))
                {
                    v.update(cx, |v, cx| v.set_phone(phone, cx));
                }
            }
        }
        cx.notify();
    }

    /// Sends the board and the session list when a phone is there to see them and they changed.
    pub(crate) fn phone_publish(&mut self, cx: &App) {
        if !self.phone.anyone() || !self.loaded {
            return;
        }
        let mut sessions: Vec<(u64, SessionId)> =
            self.sessions.iter().map(|(s, t)| (*t, *s)).collect();
        sessions.sort();
        if sessions != self.phone.sessions {
            self.phone.sessions = sessions.clone();
            self.client
                .send(Command::Phone(PhoneCommand::Sessions { sessions }));
        }
        let board = self.board(cx);
        if self.phone.sent.as_ref() != Some(&board) {
            self.phone.sent = Some(board.clone());
            self.client.send(Command::Phone(PhoneCommand::Board {
                board: Box::new(board),
            }));
        }
    }

    /// The sidebar as data: each space and its threads, with what each row shows.
    fn board(&self, cx: &App) -> Board {
        let now = now_ms();
        let mut spaces = Vec::new();
        let mut threads = Vec::new();
        for s in self.state.spaces.iter().filter(|s| !s.archived) {
            let (lf, li) = hyprspace_theme::tag(&s.name, false);
            let (df, di) = hyprspace_theme::tag(&s.name, true);
            spaces.push(BoardSpace {
                id: s.id,
                tag: vec![lf.0, li.0, df.0, di.0],
                name: s.name.clone(),
                path: s
                    .cwd
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            });
            let mut list: Vec<_> = s.threads.iter().collect();
            list.sort_by_key(|t| std::cmp::Reverse(t.rank()));
            for t in list {
                threads.push(self.board_thread(s.id, t, now, cx));
            }
        }
        let a = &self.state.appearance;
        let prefs = &self.state.composer;
        Board {
            spaces,
            threads,
            agents: self.agents.clone(),
            start: StartPrefs {
                agent: prefs.agent,
                permission: prefs.permission,
                picks: prefs
                    .picks
                    .iter()
                    .map(|p| (p.agent, p.model.clone(), p.effort.clone()))
                    .collect(),
                structured: prefs.structured,
            },
            theme: BoardTheme {
                scheme: a.scheme,
                light: palette(&theme(&a.theme, false, a)),
                dark: palette(&theme(&a.theme, true, a)),
            },
            // the engine knows whether someone is at the computer, and fills it in
            present: false,
        }
    }

    fn board_thread(
        &self,
        space: u64,
        t: &hyprspace_proto::Thread,
        now: u64,
        cx: &App,
    ) -> BoardThread {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let busy = matches!(status, Status::Working | Status::Waiting);
        let background = if busy {
            0
        } else {
            self.subagents(id, cx).len()
        };
        let (running, doing) = match self.views.get(&id) {
            Some(View::Structured(v)) => {
                let v = v.read(cx);
                (v.elapsed(), v.doing())
            }
            _ => (
                self.turns
                    .get(&id)
                    .filter(|_| busy)
                    .map(|t| t.elapsed().as_secs()),
                self.activity.get(&id).and_then(|a| a.doing.clone()),
            ),
        };
        let doing = match background {
            0 => doing.filter(|_| status != Status::Idle),
            1 => Some("1 subagent running".into()),
            n => Some(format!("{n} subagents running")),
        };
        let cwd = t.cwd();
        let (place, branch) = match self.git.get(cwd).map(|g| &g.branch) {
            Some(b) if b.is_repo && !b.branch.is_empty() => (b.branch.clone(), true),
            _ => (
                cwd.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                false,
            ),
        };
        BoardThread {
            id,
            space,
            title: line(&t.title, 200),
            kind: match t.kind {
                ThreadKind::Structured { .. } => BoardKind::Structured,
                ThreadKind::Terminal { .. } => BoardKind::Terminal,
            },
            agent: t.agent().map(|l| l.agent),
            model: t.agent().map(|l| self.model_label(l)),
            status: match status {
                Status::Idle => BoardStatus::Idle,
                Status::Working => BoardStatus::Working,
                Status::Waiting => BoardStatus::Waiting,
                Status::Done => BoardStatus::Done,
                Status::Failed => BoardStatus::Failed,
            },
            doing: doing.map(|d| line(&d, 160)),
            since: running.map(|secs| now.saturating_sub(secs * 1000)),
            unseen: self.unseen.contains(&id),
            place,
            branch,
            touched: t.last_touch(),
            shelf: if t.settled {
                Shelf::Settled
            } else if t.snooze.is_some() {
                Shelf::Snoozed
            } else {
                Shelf::Active
            },
            rank: t.rank(),
            live: self.views.contains_key(&id),
        }
    }

    fn phone_ask(&mut self, ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        match ask {
            Ask::Send { thread, text } => {
                if let Some(v) = self.structured_view(thread, cx) {
                    v.update(cx, |v, cx| v.send_text(text, cx));
                }
            }
            Ask::Approve {
                thread,
                request,
                answer,
            } => {
                if let Some(v) = self.structured_view(thread, cx) {
                    v.update(cx, |v, cx| v.approve(request, answer, cx));
                }
            }
            Ask::Interrupt { thread } => {
                if let Some(View::Structured(v)) = self.views.get(&thread) {
                    v.update(cx, |v, cx| v.stop(cx));
                }
            }
            Ask::Settle { thread, on } => {
                let snoozed = self
                    .state
                    .thread(thread)
                    .is_some_and(|(_, t)| t.snooze.is_some());
                if !on && snoozed {
                    self.snooze(thread, None, window, cx);
                } else {
                    self.settle(thread, on, window, cx);
                }
            }
            Ask::New {
                space,
                folder,
                start,
            } => {
                let space = match folder.map(std::path::PathBuf::from) {
                    Some(f) if f.is_dir() => self.add_project(f, cx),
                    Some(_) => return,
                    None => space,
                };
                let Some(cwd) = self.state.space(space).and_then(|s| s.cwd.clone()) else {
                    return;
                };
                let Some(agent) = start.agent else {
                    self.new_terminal(space, false, window, cx);
                    return;
                };
                let mut launch = Launch::new(agent, cwd);
                launch.model = Some(start.model).filter(|m| !m.is_empty());
                launch.effort = Some(start.effort).filter(|e| !e.is_empty());
                launch.permission = start.permission;
                let prompt = Some(start.prompt.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .map(hyprspace_proto::Prompt::text);
                let title = crate::composer::title_of(
                    prompt.as_ref().map(|p| p.text.as_str()).unwrap_or_default(),
                );
                let terminal = start.terminal;
                self.start_thread(
                    space,
                    Start {
                        launch,
                        prompt,
                        title,
                        terminal,
                    },
                    false,
                    window,
                    cx,
                );
            }
        }
        self.phone_publish(cx);
    }

    /// A structured thread's view, made (with its history) if it has none yet.
    fn structured_view(
        &mut self,
        thread: u64,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<crate::transcript::TranscriptView>> {
        if !self.views.contains_key(&thread) {
            let (_, t) = self.state.thread(thread)?;
            let t = t.clone();
            self.make_view(&t, None, true, cx);
        }
        match self.views.get(&thread) {
            Some(View::Structured(v)) => Some(v.clone()),
            _ => None,
        }
    }
}

/// The first line of `text`, at most `max` characters: the phone shows one line of a title or
/// of what an agent last said, and an agent's whole last message can run to pages.
fn line(text: &str, max: usize) -> String {
    let first = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .trim();
    if first.chars().count() <= max {
        return first.to_string();
    }
    let cut: String = first.chars().take(max).collect();
    format!("{}...", cut.trim_end())
}

fn theme(id: &str, dark: bool, a: &hyprspace_proto::state::Appearance) -> Theme {
    let mut t = hyprspace_theme::build(id, dark);
    if a.diff_colors == hyprspace_proto::state::DiffColors::BlueOrange {
        (t.diff_add, t.diff_del) = hyprspace_theme::blue_orange(dark);
    }
    t
}

fn palette(t: &Theme) -> Palette {
    Palette {
        bg: t.bg.0,
        surface1: t.surface1.0,
        surface2: t.surface2.0,
        surface3: t.surface3.0,
        accent: t.accent.0,
        on_accent: t.on_accent.0,
        link: t.link.0,
        text1: t.text1.0,
        text2: t.text2.0,
        text3: t.text3.0,
        border0: t.border0.0,
        border1: t.border1.0,
        border2: t.border2.0,
        ink: t.ink.0,
        busy: t.busy.0,
        waiting: t.waiting.0,
        ok: t.ok.0,
        error: t.error.0,
        diff_add: t.diff_add.0,
        diff_del: t.diff_del.0,
        term_bg: t.term_bg.0,
        term_fg: t.term_fg.0,
        cursor: t.cursor.0,
        ansi: t.ansi.iter().map(|c| c.0).collect(),
    }
}
