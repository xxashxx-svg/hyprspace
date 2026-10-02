// The window's root: the sidebar of spaces and threads on the left, the open thread (or the
// composer) on the right. It owns the saved state, keeps one view per opened thread so runs go
// on while another thread is on screen, and routes the engine's events to them.

mod render;
mod threads;

use std::collections::HashMap;
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
}

pub enum View {
    Structured(Entity<TranscriptView>),
    Terminal(Entity<TerminalView>),
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
    AddProject,
    NewOpenSpace,
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
    pub(crate) screen: Screen,
    pub(crate) composer: Entity<Composer>,
    pub(crate) search: Entity<TextInput>,
    pub(crate) rename: Option<(Rename, Entity<TextInput>, Subscription)>,
    pub(crate) menu: Option<(Point<Pixels>, MenuItems)>,
    pub(crate) archived_open: bool,
    /// The Appearance menu, open at this point.
    pub(crate) appearance_at: Option<Point<Pixels>>,
    /// A folder named on the command line, opened once the state has loaded.
    pub(crate) open_arg: Option<PathBuf>,
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
        Self {
            client,
            state: AppState::default(),
            loaded: false,
            agents: Vec::new(),
            views: HashMap::new(),
            sessions: HashMap::new(),
            next_session: 1,
            status: HashMap::new(),
            screen: Screen::Compose(None),
            composer,
            search,
            rename: None,
            menu: None,
            archived_open: false,
            appearance_at: None,
            open_arg: open,
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
            } => {
                if let Screen::Compose(Some(space)) = self.screen {
                    self.start_thread(
                        space,
                        launch.clone(),
                        prompt.clone(),
                        title.clone(),
                        window,
                        cx,
                    );
                }
            }
            ComposerEvent::Cloned {
                path,
                open_here,
                launch,
                prompt,
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
                    self.start_thread(space, launch.clone(), prompt.clone(), title, window, cx);
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

    /// A model's label from the agent's catalog, or the agent's name for its default.
    pub(crate) fn model_label(&self, launch: &hyprspace_proto::Launch) -> String {
        let Some(id) = launch.model.as_deref() else {
            return launch.agent.name().to_string();
        };
        self.agents
            .iter()
            .find(|a| a.agent == launch.agent)
            .and_then(|a| a.catalog.models.iter().find(|m| m.id == id))
            .map_or_else(|| id.to_string(), |m| m.label.clone())
    }

    pub(crate) fn save(&self) {
        if self.loaded {
            self.client.send(Command::SaveState {
                state: self.state.clone(),
            });
        }
    }
}
