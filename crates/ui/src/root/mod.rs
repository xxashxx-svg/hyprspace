// The window's root: the sidebar of spaces and threads on the left, the open thread (or the
// composer, or settings) on the right. It owns the saved state, keeps one view per opened thread
// so runs go on while another thread is on screen, and routes the engine's events to them.

mod render;
mod threads;

pub(crate) use threads::folder_name;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use futures::StreamExt;
use gpui::{AppContext, Context, Entity, Pixels, Point, SharedString, Subscription, Task, Window};
use hyprspace_proto::agents::AgentInfo;
use hyprspace_proto::{AppState, Client, Command, Event, Events, SessionId};

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
    Space(u64),
    Thread(u64),
}

#[derive(Clone, Copy)]
pub enum Action {
    NewThread(u64),
    NewTerminal(u64),
    Rename(Rename),
    ArchiveSpace(u64, bool),
    ArchiveThread(u64, bool),
    RemoveSpace(u64),
    RemoveThread(u64),
    /// Opens the thread as a new pane beside the ones on screen.
    OpenBeside(u64),
    /// Opens the space's folder in an editor or the file manager.
    OpenIn(hyprspace_proto::Opener, u64),
}

/// A context menu's rows: what each says and does.
pub type MenuItems = Vec<(SharedString, Action)>;

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
    pub(crate) search: Entity<TextInput>,
    pub(crate) rename: Option<(Rename, Entity<TextInput>, Subscription)>,
    pub(crate) menu: Option<(Point<Pixels>, MenuItems)>,
    pub(crate) archived_open: bool,
    /// Where Settings' Back button returns to.
    pub(crate) back: Screen,
    pub(crate) settings: crate::settings::Settings,
    /// A folder named on the command line, opened once the state has loaded.
    pub(crate) open_arg: Option<PathBuf>,
    /// Panes, the dock and the viewers (`crate::panes`).
    pub(crate) work: crate::panes::Work,
    /// Plan limits for the ring and Settings' Usage (`crate::usage`).
    pub(crate) limits: Entity<crate::usage::Limits>,
    /// Settings' Skills view (`crate::skills`).
    pub(crate) skills: Entity<crate::skills::Skills>,
    /// The command palette while it is open (`crate::palette`).
    pub(crate) palette: Option<Entity<crate::palette::Palette>>,
    /// The intro while it is showing (`crate::intro`).
    pub(crate) intro: Option<crate::intro::Intro>,
    /// The app updating itself (`crate::update`).
    pub(crate) updater: Entity<crate::update::Updater>,
    pub(crate) _pump: Task<()>,
    pub(crate) _subs: Vec<Subscription>,
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
        let search = cx.new(|cx| TextInput::new("Search", false, cx));
        let subs = vec![
            cx.subscribe_in(&composer, window, Self::on_composer),
            cx.subscribe(&search, |this, input, e: &InputEvent, cx| {
                if let InputEvent::Cancel = e {
                    input.update(cx, |i, cx| i.set_text("", cx));
                }
                this.menu = None;
                cx.notify();
            }),
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
        let work = crate::panes::Work::new(client.clone(), window, cx);
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
            search,
            rename: None,
            menu: None,
            archived_open: false,
            back: Screen::Compose(None),
            settings: crate::settings::Settings::new(cx),
            open_arg: open,
            work,
            limits,
            skills,
            palette: None,
            intro: None,
            updater,
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
                    self.set_status(thread, Status::from(state));
                    cx.notify();
                }
            }
        }
    }

    /// The view for a session. An event for a session whose view is gone is stale, so it is
    /// dropped.
    fn view_of(&self, id: SessionId) -> Option<&View> {
        self.sessions.get(&id).and_then(|t| self.views.get(t))
    }

    fn loaded(&mut self, state: AppState, window: &mut Window, cx: &mut Context<Self>) {
        self.state = state;
        self.loaded = true;
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
        crate::colors::set(&a.theme, crate::colors::dark(a.scheme, window.appearance()));
        crate::settings::set_terminal_font(a);
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
