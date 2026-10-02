// The composer: where a thread starts. Pick the agent, model, effort and permission mode, type
// a task (or paste an image), press Enter. Under the box: the agent's saved conversations for
// the folder to resume, or the clone card when the text starts with a repository link. The
// product flow follows the Tauri app's ComposerPane; the root turns its events into threads.

mod card;
mod clone;
pub(crate) mod model_menu;
mod pickers;
mod repo;

use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, Entity, EventEmitter, ExternalPaths, Focusable, FontWeight, IntoElement,
    PathPromptOptions, Pixels, Point, Render, Subscription, Window, div, prelude::*, px, relative,
};
use hyprspace_proto::agents::{AgentInfo, AgentSession};
use hyprspace_proto::state::{ComposerPrefs, Pick};
use hyprspace_proto::{Agent, Client, Command, Launch, Prompt};

use crate::input::{InputEvent, TextInput};
use crate::{attach, colors};
use clone::CloneCard;
use model_menu::{Anchor, Choice, Host as _, ModelMenu, Spec};
pub use pickers::effort_label;

/// Where a new thread goes: a space, its name, and its folder (None for an open space).
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub space: u64,
    pub name: String,
    pub cwd: Option<PathBuf>,
}

pub enum ComposerEvent {
    /// Start a thread in the target space, in a terminal session when `terminal`.
    Start {
        launch: Launch,
        prompt: Option<Prompt>,
        title: String,
        terminal: bool,
    },
    /// A clone finished. `open_here` starts the thread in the target space; otherwise the
    /// folder becomes its own space, with a thread when there was a task.
    Cloned {
        path: PathBuf,
        open_here: bool,
        launch: Launch,
        prompt: Option<Prompt>,
        terminal: bool,
    },
    /// The picks changed; the root saves them.
    Prefs(ComposerPrefs),
    /// With no space yet, a folder was picked to become the first one.
    AddProject(PathBuf),
}

#[derive(Clone, Copy)]
pub enum PickFor {
    /// The folder a thread in an open space runs in.
    Folder,
    CloneParent,
    /// A first project, when there is no space at all.
    Project,
}

pub struct Composer {
    client: Client,
    input: Entity<TextInput>,
    target: Option<Target>,
    /// The folder picked for a thread in an open space.
    folder: Option<PathBuf>,
    agents: Vec<AgentInfo>,
    prefs: ComposerPrefs,
    images: Vec<PathBuf>,
    /// The permission menu, open where it was clicked.
    menu: Option<Point<Pixels>>,
    models: Option<ModelMenu>,
    /// Where the model chip sits, for the model menu to open from.
    anchor: Anchor,
    effort_anchor: Anchor,
    resumable: Vec<AgentSession>,
    /// The (agent, folder) the resume list is for.
    asked: Option<(Agent, PathBuf)>,
    clone: CloneCard,
    next_request: u64,
    error: Option<String>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Composer {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(
                "Describe the task, paste an image, or paste a repository link to clone it",
                true,
                cx,
            )
        });
        let name = cx.new(|cx| TextInput::new("folder name", false, cx));
        let subs = vec![
            cx.subscribe(&input, |this, _, e: &InputEvent, cx| match e {
                InputEvent::Submit => this.submit(cx),
                InputEvent::Changed => this.text_changed(cx),
                InputEvent::Images(images) => {
                    this.images
                        .extend(images.iter().filter_map(|i| attach::save(i).ok()));
                    cx.notify();
                }
                InputEvent::Cancel => {
                    this.menu = None;
                    cx.notify();
                }
            }),
            cx.subscribe(&name, |this, _, e: &InputEvent, cx| {
                if let InputEvent::Submit = e {
                    this.start_clone(cx);
                }
                cx.notify();
            }),
        ];
        Self {
            client,
            input,
            target: None,
            folder: None,
            agents: Vec::new(),
            prefs: ComposerPrefs::default(),
            images: Vec::new(),
            menu: None,
            models: None,
            anchor: Anchor::default(),
            effort_anchor: Anchor::default(),
            resumable: Vec::new(),
            asked: None,
            clone: CloneCard {
                url: String::new(),
                parent: PathBuf::new(),
                name,
                here: false,
                open_here: false,
                request: None,
                line: None,
                error: None,
                parent_empty: None,
            },
            next_request: 1,
            error: None,
            _subs: subs,
        }
    }

    /// Shows the composer for `target`. The resume list is read again every time, since the
    /// agent may have saved a conversation since the last look.
    pub fn set_target(&mut self, target: Option<Target>, cx: &mut Context<Self>) {
        if self.target != target {
            self.folder = None;
            self.error = None;
            self.target = target;
            self.clone.open_here = false;
        }
        self.asked = None;
        self.ask_resumable();
        cx.notify();
    }

    pub fn set_agents(&mut self, agents: Vec<AgentInfo>, cx: &mut Context<Self>) {
        self.agents = agents;
        self.ask_resumable();
        cx.notify();
    }

    pub fn set_prefs(&mut self, prefs: ComposerPrefs, cx: &mut Context<Self>) {
        self.prefs = prefs;
        self.ask_resumable();
        cx.notify();
    }

    fn cwd(&self) -> Option<PathBuf> {
        self.target
            .as_ref()
            .and_then(|t| t.cwd.clone())
            .or_else(|| self.folder.clone())
    }

    fn installed(&self) -> impl Iterator<Item = &AgentInfo> {
        self.agents.iter().filter(|a| a.status.installed)
    }

    /// The agent the next thread runs: the last pick if it is installed, else the first one.
    fn agent(&self) -> Option<&AgentInfo> {
        let picked = self.prefs.agent;
        self.installed()
            .find(|a| Some(a.agent) == picked)
            .or_else(|| self.installed().next())
    }

    /// Whether the next thread runs in a terminal: picked, or the only way the agent runs.
    fn terminal(&self) -> bool {
        self.prefs.terminal || self.agent().is_some_and(|a| !a.agent.structured())
    }

    fn toggle_terminal(&mut self, cx: &mut Context<Self>) {
        self.prefs.terminal = !self.prefs.terminal;
        cx.emit(ComposerEvent::Prefs(self.prefs.clone()));
        cx.notify();
    }

    fn pick(&self) -> Option<Pick> {
        self.agent().map(|a| self.prefs.pick(a.agent))
    }

    fn launch(&self, cwd: PathBuf) -> Option<Launch> {
        let agent = self.agent()?.agent;
        let pick = self.prefs.pick(agent);
        Some(Launch {
            agent,
            cwd,
            model: Some(pick.model).filter(|m| !m.is_empty()),
            effort: Some(pick.effort).filter(|e| !e.is_empty()),
            permission: self.prefs.permission,
            resume: None,
        })
    }

    fn set_pick(&mut self, pick: Pick, cx: &mut Context<Self>) {
        self.prefs.agent = Some(pick.agent);
        self.prefs.set_pick(pick);
        self.ask_resumable();
        cx.emit(ComposerEvent::Prefs(self.prefs.clone()));
        cx.notify();
    }

    fn ask_resumable(&mut self) {
        let (Some(agent), Some(cwd)) = (self.agent().map(|a| a.agent), self.cwd()) else {
            self.resumable.clear();
            self.asked = None;
            return;
        };
        let key = (agent, cwd.clone());
        if self.asked.as_ref() != Some(&key) {
            self.resumable.clear();
            self.asked = Some(key);
            self.client.send(Command::ListResumable { agent, cwd });
        }
    }

    pub fn resumable(
        &mut self,
        agent: Agent,
        cwd: PathBuf,
        sessions: Vec<AgentSession>,
        cx: &mut Context<Self>,
    ) {
        if self.asked == Some((agent, cwd)) {
            self.resumable = sessions;
            cx.notify();
        }
    }

    pub fn clone_progress(&mut self, request: u64, line: String, cx: &mut Context<Self>) {
        if self.clone.request == Some(request) {
            self.clone.line = Some(line);
            cx.notify();
        }
    }

    pub fn cloned(
        &mut self,
        request: u64,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        if self.clone.request != Some(request) {
            return;
        }
        self.clone.request = None;
        self.clone.line = None;
        match result {
            Ok(path) => {
                let rest = repo::split(self.input.read(cx).text())
                    .map(|(_, rest)| rest)
                    .unwrap_or_default();
                let prompt = self.prompt(rest);
                if let Some(launch) = self.launch(path.clone()) {
                    cx.emit(ComposerEvent::Cloned {
                        path,
                        open_here: self.clone.open_here && self.target.is_some(),
                        launch,
                        prompt,
                        terminal: self.terminal(),
                    });
                }
                self.reset(cx);
            }
            Err(e) => self.clone.error = Some(e),
        }
        cx.notify();
    }

    fn text_changed(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let Some((repo, _)) = repo::split(self.input.read(cx).text()) else {
            return;
        };
        if repo.url != self.clone.url {
            self.clone.url = repo.url.clone();
            self.clone.error = None;
            self.clone
                .name
                .update(cx, |n, cx| n.set_text(repo.name, cx));
            if self.clone.parent.as_os_str().is_empty() {
                let parent = self.cwd().unwrap_or_else(home);
                self.clone.set_parent(parent);
            }
        }
    }

    /// The text and attached images as a prompt; None when both are empty.
    fn prompt(&mut self, text: String) -> Option<Prompt> {
        let images = std::mem::take(&mut self.images);
        (!text.is_empty() || !images.is_empty()).then_some(Prompt { text, images })
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.images.clear();
        self.clone.url.clear();
        self.clone.error = None;
        self.input.update(cx, |i, cx| i.set_text("", cx));
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        self.models = None;
        let text = self.input.read(cx).text().trim().to_string();
        if repo::split(&text).is_some() {
            self.start_clone(cx);
            return;
        }
        if self.target.is_none() {
            self.pick_folder(PickFor::Project, cx);
            return;
        }
        let Some(cwd) = self.cwd() else {
            self.pick_folder(PickFor::Folder, cx);
            return;
        };
        let Some(launch) = self.launch(cwd) else {
            self.error = Some(no_agent());
            cx.notify();
            return;
        };
        let title = title_of(&text);
        let prompt = self.prompt(text);
        cx.emit(ComposerEvent::Start {
            launch,
            prompt,
            title,
            terminal: self.terminal(),
        });
        self.reset(cx);
    }

    fn resume(&mut self, session: AgentSession, cx: &mut Context<Self>) {
        let Some(mut launch) = self.cwd().and_then(|c| self.launch(c)) else {
            return;
        };
        launch.resume = Some(session.id);
        let text = self.input.read(cx).text().trim().to_string();
        let prompt = self.prompt(text);
        cx.emit(ComposerEvent::Start {
            launch,
            prompt,
            title: session.title,
            terminal: self.terminal(),
        });
        self.reset(cx);
    }

    fn start_clone(&mut self, cx: &mut Context<Self>) {
        if self.clone.busy() {
            return;
        }
        let Some((repo, _)) = repo::split(self.input.read(cx).text()) else {
            return;
        };
        let name = self.clone.name.read(cx).text().trim().to_string();
        if self.clone.parent.as_os_str().is_empty() || (!self.clone.here && name.is_empty()) {
            self.clone.error = Some("Pick a folder and a name for the clone.".into());
            cx.notify();
            return;
        }
        if self.agent().is_none() {
            self.error = Some(no_agent());
            cx.notify();
            return;
        }
        let request = self.next_request;
        self.next_request += 1;
        self.clone.request = Some(request);
        self.clone.error = None;
        self.client.send(Command::Clone {
            request,
            url: repo.url,
            parent: self.clone.parent.clone(),
            name,
            here: self.clone.here,
        });
        cx.notify();
    }

    pub fn pick_folder(&mut self, why: PickFor, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |c, cx| {
                match why {
                    PickFor::Folder => {
                        c.folder = Some(path);
                        c.ask_resumable();
                    }
                    PickFor::CloneParent => c.clone.set_parent(path),
                    PickFor::Project => cx.emit(ComposerEvent::AddProject(path)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks for image files to attach.
    fn pick_images(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let _ = this.update(cx, |c, cx| {
                c.images
                    .extend(paths.into_iter().filter(|p| attach::is_image(p)));
                cx.notify();
            });
        })
        .detach();
    }

    fn open_permission(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.menu = Some(e.position());
        cx.notify();
    }

    fn open_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(spec) = self.model_spec() {
            self.menu = None;
            self.models = Some(ModelMenu::open(&spec, window, cx));
            cx.notify();
        }
    }

    fn open_effort(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(spec) = self.model_spec() {
            self.menu = None;
            self.models = Some(ModelMenu::open_effort(&spec, window, cx));
            cx.notify();
        }
    }

    fn drop_paths(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        self.images.extend(
            paths
                .paths()
                .iter()
                .filter(|p| attach::is_image(p))
                .cloned(),
        );
        cx.notify();
    }
}

fn no_agent() -> String {
    "Neither claude nor codex is installed. Install one and sign in, then reopen HyprSpace.".into()
}

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// A thread's first name: the first line of its task, cut to fit the sidebar.
pub fn title_of(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.is_empty() {
        return "New thread".into();
    }
    if line.chars().count() > 60 {
        format!(
            "{}...",
            line.chars().take(57).collect::<String>().trim_end()
        )
    } else {
        line.to_string()
    }
}

impl Focusable for Composer {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.input.read(cx).text().to_string();
        let repo = repo::split(&text).map(|(r, _)| r);
        // only a clone says what it will do; otherwise the box speaks for itself, as zeron's does
        let heading = repo.as_ref().map(|r| {
            div()
                .flex()
                .justify_center()
                .mb(px(16.))
                .text_size(px(22.))
                .font_weight(FontWeight::MEDIUM)
                .child("Clone ")
                .child(bold(r.label.clone()))
        });
        let agent_name = self.agent().map(|a| a.agent.name()).unwrap_or("The agent");
        let below = match &repo {
            Some(r) => Some(clone::render(
                &self.clone,
                r,
                agent_name,
                self.target.is_some(),
                cx,
            )),
            None => card::resume_list(self, cx),
        };
        let menu = self
            .menu
            .map(|at| pickers::permission_menu(self, at, window, cx));
        let models = self
            .models
            .as_ref()
            .zip(self.model_spec())
            .and_then(|(m, spec)| {
                let anchor = if m.is_effort() {
                    &self.effort_anchor
                } else {
                    &self.anchor
                };
                model_menu::render(m, &spec, anchor, window, cx)
            });
        div()
            .id("composer")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .bg(colors::bg())
            .text_color(colors::text1())
            .on_drop(cx.listener(|c, paths: &ExternalPaths, _, cx| c.drop_paths(paths, cx)))
            .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(colors::accent_dim()))
            .overflow_y_scroll()
            // the box sits a third of the way down, like zeron's
            .child(div().flex_none().h(relative(0.3)))
            .child(
                div()
                    .w_full()
                    .max_w(px(760.))
                    .px(px(24.))
                    .pb(px(24.))
                    .flex()
                    .flex_col()
                    .children(heading)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .mb(px(6.))
                            .child(card::folder_picker(self, cx)),
                    )
                    .child(card::card(self, window, cx))
                    .children(self.error.clone().map(|e| {
                        div()
                            .mt(px(10.))
                            .text_size(px(12.))
                            .text_color(colors::error())
                            .child(e)
                    }))
                    .children(below),
            )
            .children(menu)
            .children(models)
    }
}

impl model_menu::Host for Composer {
    /// Every installed agent's models, grouped by agent.
    fn model_spec(&self) -> Option<Spec> {
        let pick = self.pick()?;
        let models = self
            .installed()
            .flat_map(|a| {
                a.catalog.models.iter().map(|m| model_menu::Model {
                    agent: a.agent,
                    id: m.id.clone(),
                    label: m.label.clone(),
                    note: m.note.clone(),
                })
            })
            .collect();
        let catalog = &self.agent()?.catalog;
        Some(Spec {
            models,
            efforts: catalog.efforts_for(&pick.model).to_vec(),
            default_effort: catalog
                .model(&pick.model)
                .and_then(|m| m.default_effort.clone()),
            long: crate::models::takes_long(pick.agent, &pick.model)
                .then(|| crate::models::is_long(&pick.model)),
            agent: pick.agent,
            model: pick.model,
            effort: pick.effort,
            foot: None,
        })
    }

    fn model_menu(&mut self) -> &mut Option<ModelMenu> {
        &mut self.models
    }

    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
            Choice::Model(agent, model) => {
                let old = self.prefs.pick(agent);
                // keep the effort when the new model takes it
                let takes = self
                    .agents
                    .iter()
                    .find(|a| a.agent == agent)
                    .is_some_and(|a| a.catalog.efforts_for(&model).contains(&old.effort));
                let effort = if takes { old.effort } else { String::new() };
                // the 1M window carries over to a model that has one
                let long =
                    crate::models::is_long(&old.model) && crate::models::takes_long(agent, &model);
                self.set_pick(
                    Pick {
                        agent,
                        model: crate::models::windowed(&model, long),
                        effort,
                    },
                    cx,
                );
            }
            Choice::Long(long) => {
                if let Some(pick) = self.pick() {
                    let model = crate::models::windowed(&pick.model, long);
                    self.set_pick(Pick { model, ..pick }, cx);
                }
            }
            Choice::Effort(effort) => {
                if let Some(pick) = self.pick() {
                    self.set_pick(Pick { effort, ..pick }, cx);
                }
            }
        }
    }
}

fn bold(text: String) -> gpui::Div {
    div().font_weight(FontWeight::BOLD).child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_the_first_line() {
        assert_eq!(title_of(""), "New thread");
        assert_eq!(title_of("  fix it\nmore"), "fix it");
        let long = "a".repeat(80);
        assert_eq!(title_of(&long).chars().count(), 60);
    }
}
