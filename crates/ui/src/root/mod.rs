// The window's root: the sidebar of spaces and threads on the left, the open thread (or the
// composer, or settings) on the right. It owns the saved state, keeps one view per opened thread
// so runs go on while another thread is on screen, and routes the engine's events to them.

mod render;
mod settle;
mod threads;
pub mod titlebar;

pub(crate) use threads::folder_name;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui::{AppContext, Context, Entity, Pixels, Point, SharedString, Subscription, Task, Window};
use hyprspace_proto::agents::AgentInfo;
use hyprspace_proto::{AppState, Client, Command, Event, Events, SessionId, Thread, ThreadKind};

use crate::composer::{Composer, ComposerEvent};
use crate::input::{InputEvent, TextInput};
use crate::terminal::TerminalView;
use crate::transcript::{Status, TranscriptView};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Thread(u64),
    /// The composer for a space, or for no space at all on a first run.
    Compose(Option<u64>),
    Settings,
}

pub enum View {
    Structured(Entity<TranscriptView>),
    Terminal(Entity<TerminalView>),
}

/// A new thread, as the composer asked for it.
pub struct Start {
    pub launch: hyprspace_proto::Launch,
    pub prompt: Option<hyprspace_proto::Prompt>,
    pub title: String,
    /// Run the agent in a terminal session instead of a structured one.
    pub terminal: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rename {
    Thread(u64),
}

#[derive(Clone, Copy)]
pub enum Action {
    NewThread(u64),
    NewTerminal(u64),
    Rename(Rename),
    /// Settle every thread at work in a space.
    SettleSpace(u64),
    /// Settles a thread, or brings it back.
    Settle(u64, bool),
    /// Snoozes a thread until a time, in ms since the epoch.
    SnoozeUntil(u64, u64),
    /// Snoozes a thread until its agent finishes its turn.
    SnoozeDone(u64),
    Wake(u64),
    RemoveThread(u64),
    /// Opens the space's folder in an editor or the file manager.
    OpenIn(hyprspace_proto::Opener, u64),
    /// Narrows the sidebar to a space's threads, by its name in the search box.
    Filter(u64),
    CopyTitle(u64),
    CopyPath(u64),
    CopyConversation(u64),
}

/// One line of a right-click menu.
#[derive(Clone)]
pub enum MenuEntry {
    /// A choice: its words, a note on the right (a snooze's wake time), and what it does.
    Item {
        label: SharedString,
        hint: Option<SharedString>,
        action: Action,
    },
    /// A rule between groups.
    Divider,
    /// A row that opens more choices beside it while the pointer is on it.
    Sub {
        label: SharedString,
        entries: Vec<MenuEntry>,
    },
}

impl MenuEntry {
    pub fn item(label: impl Into<SharedString>, action: Action) -> Self {
        MenuEntry::Item {
            label: label.into(),
            hint: None,
            action,
        }
    }
}

/// A right-click menu's lines, top to bottom.
pub type MenuItems = Vec<MenuEntry>;

/// The sidebar's drag handle, carried while it is dragged.
pub struct SidebarDrag;

pub struct Root {
    pub(crate) client: Client,
    pub(crate) state: AppState,
    pub(crate) loaded: bool,
    pub(crate) agents: Vec<AgentInfo>,
    pub(crate) views: HashMap<u64, View>,
    pub(crate) sessions: HashMap<SessionId, u64>,
    pub(crate) next_session: u64,
    pub(crate) status: HashMap<u64, Status>,
    /// Threads that finished while off screen, until they are opened.
    pub(crate) unseen: HashSet<u64>,
    pub(crate) screen: Screen,
    pub(crate) composer: Entity<Composer>,
    /// A thread being renamed: its box, and what ends the rename (Enter or Esc, and focus
    /// leaving the box).
    pub(crate) rename: Option<(Rename, Entity<TextInput>, [Subscription; 2])>,
    pub(crate) menu: Option<(Point<Pixels>, MenuItems)>,
    /// The menu's line whose choices are open beside it.
    pub(crate) menu_sub: Option<usize>,
    /// A thread whose row the sidebar should scroll into view: one just opened or made, which
    /// may sit above a list scrolled down.
    pub(crate) reveal: Option<u64>,
    /// A settled or snoozed thread opened from its shelf, on screen while it stays there. A
    /// message sent to it brings it back to the list.
    pub(crate) peek: Option<u64>,
    /// An image shown over the whole window, from a Ctrl+click on it in a terminal.
    pub(crate) lightbox: Option<(Entity<crate::viewer::lightbox::Lightbox>, Subscription)>,
    /// While a row is dragged over the list: the row under the pointer, and whether the dragged
    /// one would land below it (the pointer on its lower half) rather than above.
    pub(crate) drop_at: Option<(u64, bool)>,
    /// Whether the Settled and Snoozed shelves are open.
    pub(crate) settled_open: bool,
    pub(crate) snoozed_open: bool,
    /// The sidebar row under the pointer and since when, for its hover card.
    pub(crate) hover_row: Option<(u64, Instant)>,
    pub(crate) _hover_timer: Option<Task<()>>,
    /// Where each thread's row was last drawn, for the card that shows beside it.
    pub(crate) row_bounds:
        std::rc::Rc<std::cell::RefCell<HashMap<u64, gpui::Bounds<gpui::Pixels>>>>,
    /// The snooze menu: where it opened, for which thread.
    pub(crate) snooze_menu: Option<(Point<Pixels>, u64)>,
    /// The last settle or snooze, which the toast can undo for a few seconds.
    pub(crate) undo: Option<settle::Undo>,
    _undo_timer: Option<Task<()>>,
    /// Where Settings' Back button returns to.
    pub(crate) back: Screen,
    pub(crate) settings: crate::settings::Settings,
    /// A folder named on the command line, opened once the state has loaded.
    pub(crate) open_arg: Option<PathBuf>,
    /// The main area, the dock and the viewer (`crate::workbench`).
    pub(crate) work: crate::workbench::Work,
    /// Plan limits for the ring and Settings' Usage (`crate::usage`).
    pub(crate) limits: Entity<crate::usage::Limits>,
    /// Settings' Skills view (`crate::skills`).
    pub(crate) skills: Entity<crate::skills::Skills>,
    /// The command palette while it is open (`crate::palette`).
    pub(crate) palette: Option<Entity<crate::palette::Palette>>,
    /// The in-app folder browser for opening a folder as a space (`crate::folders`).
    pub(crate) folder_picker: Option<Entity<crate::folders::FolderPicker>>,
    /// The intro while it is showing (`crate::intro`).
    pub(crate) intro: Option<crate::intro::Intro>,
    /// Takes the intro away once its exit has played.
    pub(crate) _intro_exit: Option<Task<()>>,
    /// The app updating itself (`crate::update`).
    pub(crate) updater: Entity<crate::update::Updater>,
    /// A press in the title row on macOS, until the pointer moves and the window drag starts.
    pub(crate) moving: bool,
    /// The sidebar's slide (`crate::slide`), shared by its column and its part of the title row.
    pub(crate) sidebar_flips: crate::slide::Flips,
    /// What each terminal thread's agent is doing, from its hooks.
    pub(crate) activity: HashMap<u64, Activity>,
    /// When each terminal thread's current turn began, for the sidebar's running count.
    pub(crate) turns: HashMap<u64, Instant>,
    /// The sidebar, drawn through a cached view of its own (`crate::sidebar::SidebarView`).
    pub(crate) sidebar_view: Entity<crate::sidebar::SidebarView>,
    /// The sidebar's search box, which narrows the spaces and threads to what matches.
    pub(crate) search: Entity<TextInput>,
    /// The last git status read for each folder the sidebar shows: its branch and changes.
    pub(crate) git: HashMap<PathBuf, hyprspace_proto::folder::GitStatus>,
    /// A one-second tick runs while a thread works or a subagent runs, so their counts move.
    ticking: bool,
    _ticker: Option<Task<()>>,
    _git_pump: Task<()>,
    /// When `git_poll` last read every open space, not just the one on screen.
    git_polled: Option<Instant>,
    pub(crate) _pump: Task<()>,
    pub(crate) _subs: Vec<Subscription>,
}

/// One line on what an agent does and the subagents it has running.
#[derive(Default)]
pub(crate) struct Activity {
    pub doing: Option<String>,
    pub subs: Vec<hyprspace_proto::SubAgent>,
}

impl Root {
    pub fn new(
        client: Client,
        mut events: Events,
        open: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        client.send(Command::LoadState);
        client.send(Command::LoadAgents);
        let composer = cx.new(|cx| Composer::new(client.clone(), cx));
        let mut subs = vec![
            cx.subscribe_in(&composer, window, Self::on_composer),
            cx.observe_window_appearance(window, |root, window, cx| {
                root.apply_theme(window);
                cx.notify();
            }),
            cx.observe_window_activation(window, |root, window, cx| {
                if window.is_window_active() {
                    root.updater.update(cx, |u, cx| u.focused(cx));
                }
            }),
        ];
        let pump = cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = events.next().await {
                if this
                    .update_in(cx, |root, window, cx| root.route(event, window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let work = crate::workbench::Work::new(client.clone(), window, cx);
        let search = cx.new(|cx| TextInput::new("Search", false, cx));
        let root = cx.entity();
        let sidebar_view = cx.new(|cx| crate::sidebar::SidebarView::new(&root, cx));
        subs.push(cx.subscribe(&search, |_, input, e: &InputEvent, cx| {
            if let InputEvent::Cancel = e {
                input.update(cx, |i, cx| i.set_text("", cx));
            }
            cx.notify();
        }));
        // the sidebar's branches and change counts, kept fresh while the app runs
        subs.push(cx.observe_window_activation(window, |r, window, _| {
            if window.is_window_active() {
                r.git_poll(Duration::from_secs(5));
            }
        }));
        let git_pump = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(15))
                    .await;
                let alive = this.update(cx, |r, cx| {
                    r.git_poll(Duration::from_secs(60));
                    r.tidy_threads(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let limits = cx.new(|cx| crate::usage::Limits::new(client.clone(), cx));
        let skills = cx.new(|_| crate::skills::Skills::new(client.clone()));
        let updater = cx.new(|cx| crate::update::Updater::new(client.clone(), cx));
        Self {
            client,
            state: AppState::default(),
            loaded: false,
            agents: Vec::new(),
            views: HashMap::new(),
            sessions: HashMap::new(),
            next_session: 1,
            status: HashMap::new(),
            unseen: HashSet::new(),
            screen: Screen::Compose(None),
            composer,
            rename: None,
            menu: None,
            menu_sub: None,
            reveal: None,
            peek: None,
            lightbox: None,
            drop_at: None,
            settled_open: false,
            snoozed_open: false,
            hover_row: None,
            _hover_timer: None,
            row_bounds: Default::default(),
            snooze_menu: None,
            undo: None,
            _undo_timer: None,
            back: Screen::Compose(None),
            settings: crate::settings::Settings::new(cx),
            open_arg: open,
            work,
            limits,
            skills,
            palette: None,
            folder_picker: None,
            intro: None,
            _intro_exit: None,
            updater,
            moving: false,
            sidebar_flips: Default::default(),
            activity: HashMap::new(),
            turns: HashMap::new(),
            search,
            sidebar_view,
            git: HashMap::new(),
            ticking: false,
            _ticker: None,
            _git_pump: git_pump,
            git_polled: None,
            _pump: pump,
            _subs: subs,
        }
    }

    fn route(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::State { state } => self.loaded(state, window, cx),
            Event::Agents { agents } => {
                for view in self.views.values() {
                    if let View::Structured(v) = view {
                        let agent = v.read(cx).agent();
                        if let Some(info) = agents.iter().find(|a| a.agent == agent) {
                            let catalog = info.catalog.clone();
                            v.update(cx, |v, cx| v.set_catalog(catalog, cx));
                        }
                    }
                }
                self.composer
                    .update(cx, |c, cx| c.set_agents(agents.clone(), cx));
                self.agents = agents;
            }
            Event::Resumable {
                agent,
                cwd,
                sessions,
            } => self
                .composer
                .update(cx, |c, cx| c.resumable(agent, cwd, sessions, cx)),
            Event::CloneProgress { request, line } => self
                .composer
                .update(cx, |c, cx| c.clone_progress(request, line, cx)),
            Event::Cloned { request, result } => self
                .composer
                .update(cx, |c, cx| c.cloned(request, result, cx)),
            Event::Run { id, event } => {
                if let Some(View::Structured(v)) = self.view_of(id) {
                    v.update(cx, |v, cx| v.apply(event, cx));
                }
            }
            Event::Journal { id, entries } => {
                if let Some(View::Structured(v)) = self.view_of(id) {
                    v.update(cx, |v, cx| v.replay(entries, cx));
                }
            }
            Event::Failed { id, message } => match self.view_of(id) {
                Some(View::Structured(v)) => v.update(cx, |v, cx| v.fail(message, cx)),
                Some(View::Terminal(v)) => v.update(cx, |v, cx| v.fail(message, cx)),
                None => {}
            },
            Event::TerminalOutput { id, bytes } => {
                if let Some(View::Terminal(v)) = self.view_of(id) {
                    v.update(cx, |v, cx| v.output(&bytes, cx));
                }
            }
            Event::TerminalExit { id, code } => {
                if let Some(View::Terminal(v)) = self.view_of(id) {
                    v.update(cx, |v, cx| v.exit(code, cx));
                }
                if let Some(&thread) = self.sessions.get(&id) {
                    self.status.remove(&thread);
                    self.activity.remove(&thread);
                    self.turns.remove(&thread);
                    self.session_ended(thread);
                    cx.notify();
                }
            }
            Event::ImageFound { id, n, path } => {
                if let Some(View::Terminal(v)) = self.view_of(id) {
                    v.update(cx, |v, cx| v.image_found(n, path, cx));
                }
            }
            Event::TerminalConversation { id, resume } => {
                let Some(&thread) = self.sessions.get(&id) else {
                    return;
                };
                if let Some(Thread {
                    kind: ThreadKind::Terminal { run: Some(run), .. },
                    ..
                }) = self.state.thread_mut(thread)
                {
                    run.resume = Some(resume);
                    self.save();
                }
            }
            Event::TerminalAgent { id, agent, model } => {
                if let Some(&thread) = self.sessions.get(&id) {
                    self.terminal_agent(thread, agent, model);
                    cx.notify();
                }
            }
            Event::Folder(e) => self.folder_event(e, cx),
            Event::Usage(e) => self.limits.update(cx, |l, cx| l.event(e, cx)),
            Event::Skills(e) => self.skills.update(cx, |s, cx| s.event(e, window, cx)),
            Event::Update(e) => {
                let quit = e == hyprspace_proto::UpdateEvent::Quit;
                self.updater.update(cx, |u, cx| u.event(e, cx));
                // the installer waits for this process to exit; quitting also ends every session
                if quit {
                    cx.quit();
                }
            }
            Event::AgentState { id, state } => {
                if let Some(&thread) = self.sessions.get(&id) {
                    let status = Status::from(state);
                    // a turn passes through working and waiting, and neither restarts its count
                    if matches!(status, Status::Working | Status::Waiting) {
                        self.turns.entry(thread).or_insert_with(Instant::now);
                    } else {
                        self.turns.remove(&thread);
                    }
                    self.set_status(thread, status);
                    self.tick(cx);
                    cx.notify();
                }
            }
            Event::AgentActivity { id, doing, subs } => {
                if let Some(&thread) = self.sessions.get(&id) {
                    self.activity.insert(thread, Activity { doing, subs });
                    self.tick(cx);
                    cx.notify();
                }
            }
        }
    }

    /// Asks for the git status of the folders the sidebar shows open: each open space's, and a
    /// thread's own when it runs somewhere else. Each folder costs a handful of git processes,
    /// so the space on screen is read every time and the rest once `every` has passed.
    pub(crate) fn git_poll(&mut self, every: Duration) {
        let all = self.git_polled.is_none_or(|t| t.elapsed() >= every);
        if all {
            self.git_polled = Some(Instant::now());
        }
        let here = self.current_space();
        let mut folders: Vec<PathBuf> = Vec::new();
        for s in self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived && !s.folded && (all || Some(s.id) == here))
        {
            folders.extend(s.cwd.clone());
            for t in s.threads.iter().filter(|t| t.active()) {
                if let hyprspace_proto::ThreadKind::Terminal { cwd, .. } = &t.kind {
                    folders.push(cwd.clone());
                }
            }
        }
        folders.sort();
        folders.dedup();
        for cwd in folders {
            self.client
                .send(Command::Folder(hyprspace_proto::FolderCommand::GitStatus {
                    cwd,
                }));
        }
    }

    /// Whether anything in the sidebar is counting.
    fn live(&self) -> bool {
        self.status
            .values()
            .any(|s| matches!(s, Status::Working | Status::Waiting))
            || self.activity.values().any(|a| !a.subs.is_empty())
    }

    /// Starts the one-second tick if something counts and it isn't running. It stops itself.
    pub(crate) fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking || !self.live() {
            return;
        }
        self.ticking = true;
        self._ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let live = this
                    .update(cx, |r, cx| {
                        r.ticking = r.live();
                        if r.ticking {
                            cx.notify();
                        }
                        r.ticking
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
            }
        }));
    }

    /// The view for a session. An event for a session whose view is gone is stale, so it is
    /// dropped.
    fn view_of(&self, id: SessionId) -> Option<&View> {
        self.sessions.get(&id).and_then(|t| self.views.get(t))
    }

    fn loaded(&mut self, state: AppState, window: &mut Window, cx: &mut Context<Self>) {
        self.state = state;
        self.loaded = true;
        if self.state.settle_archived() {
            self.save();
        }
        // whatever came due while the app was shut wakes now, not at the first sweep
        let woke = self.state.wake_due(crate::time::now_ms(), true);
        if !woke.is_empty() {
            self.unseen.extend(woke);
            self.save();
        }
        self.git_poll(Duration::ZERO);
        self.apply_theme(window);
        let prefs = self.state.composer.clone();
        self.composer.update(cx, |c, cx| c.set_prefs(prefs, cx));
        let seen = self.state.seen_version.clone();
        if self.updater.update(cx, |u, cx| u.launched_after(&seen, cx)) {
            self.state.seen_version = crate::update::VERSION.into();
            self.save();
        }
        self.first_run(window, cx);
        if let Some(path) = self.open_arg.take() {
            let space = self.add_project(path, cx);
            self.compose(Some(space), window, cx);
        } else if let Some(id) = self
            .state
            .active
            .filter(|id| self.state.thread(*id).is_some())
        {
            self.open_thread(id, window, cx);
        } else {
            let first = self.state.spaces.iter().find(|s| !s.archived).map(|s| s.id);
            self.compose(first, window, cx);
        }
        cx.notify();
    }

    fn on_composer(
        &mut self,
        _: &Entity<Composer>,
        event: &ComposerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ComposerEvent::Start {
                launch,
                prompt,
                title,
                terminal,
            } => {
                if let Screen::Compose(Some(space)) = self.screen {
                    let start = Start {
                        launch: launch.clone(),
                        prompt: prompt.clone(),
                        title: title.clone(),
                        terminal: *terminal,
                    };
                    self.start_thread(space, start, window, cx);
                }
            }
            ComposerEvent::Cloned {
                path,
                open_here,
                launch,
                prompt,
                terminal,
            } => {
                let here = match self.screen {
                    Screen::Compose(Some(space)) if *open_here => Some(space),
                    _ => None,
                };
                let space = here.unwrap_or_else(|| self.add_project(path.clone(), cx));
                let title = crate::composer::title_of(
                    prompt.as_ref().map(|p| p.text.as_str()).unwrap_or_default(),
                );
                if here.is_some() || prompt.is_some() {
                    let start = Start {
                        launch: launch.clone(),
                        prompt: prompt.clone(),
                        title,
                        terminal: *terminal,
                    };
                    self.start_thread(space, start, window, cx);
                } else {
                    self.compose(Some(space), window, cx);
                }
            }
            ComposerEvent::Prefs(prefs) => {
                self.state.composer = prefs.clone();
                self.save();
            }
            ComposerEvent::AddProject(path) => {
                let space = self.add_project(path.clone(), cx);
                self.compose(Some(space), window, cx);
            }
        }
    }

    /// Paints the window in the saved theme, on the side the scheme and the system ask for, and
    /// hands terminals the saved font.
    pub(crate) fn apply_theme(&self, window: &mut Window) {
        let a = &self.state.appearance;
        crate::colors::set(
            &a.theme,
            crate::colors::dark(a.scheme, window.appearance()),
            a.diff_colors == hyprspace_proto::state::DiffColors::BlueOrange,
        );
        crate::settings::set_terminal_font(a);
        crate::slide::set_animations(a.animations);
        window.refresh();
    }

    /// A model's label from the agent's catalog, or the agent's name for its default.
    pub(crate) fn model_label(&self, launch: &hyprspace_proto::Launch) -> String {
        let Some(id) = launch.model.as_deref() else {
            return launch.agent.name().to_string();
        };
        let catalog = self
            .agents
            .iter()
            .find(|a| a.agent == launch.agent)
            .map(|a| &a.catalog);
        crate::models::name(catalog, id)
    }

    pub(crate) fn save(&self) {
        if self.loaded {
            self.client.send(Command::SaveState {
                state: self.state.clone(),
            });
        }
    }
}
