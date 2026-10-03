// A structured thread: its transcript and the box under it. The view opens the session on the
// first prompt (resuming the CLI's conversation when the thread has one), sends later prompts
// as runs or steers, answers approvals, and tells the root when its status changes.

mod approval;
mod branch;
mod model;
mod render;
mod subagent;
mod tool;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, EventEmitter, ExternalPaths, Focusable, IntoElement,
    PathPromptOptions, Render, ScrollHandle, Subscription, Task, Window, px,
};
use hyprspace_proto::agents::AgentCatalog;
use hyprspace_proto::{Answer, Client, Command, Entry, Launch, Prompt, RunEvent, SessionId};

use crate::attach;
use crate::composer::model_menu::{self, Anchor, Choice, ModelMenu, Spec};
use crate::input::{InputEvent, TextInput};
pub use model::Status;
use model::Transcript;

pub enum TranscriptEvent {
    Status(Status),
    /// The CLI is up: the id that resumes its conversation, and the folder it runs in.
    Started {
        thread: String,
        cwd: PathBuf,
    },
    /// The user picked another model for this thread.
    Launch(Launch),
}

pub struct TranscriptView {
    id: SessionId,
    client: Client,
    launch: Launch,
    journal: String,
    model: Transcript,
    /// A session is open in the engine.
    open: bool,
    /// Waiting for the journal; a prompt sent meanwhile waits in `queued`.
    loading: bool,
    queued: Option<Prompt>,
    input: Entity<TextInput>,
    images: Vec<PathBuf>,
    scroll: ScrollHandle,
    catalog: Option<AgentCatalog>,
    menu: Option<ModelMenu>,
    /// Where the model chip sits, for its menu to open from.
    anchor: Anchor,
    effort_anchor: Anchor,
    /// Runs of tool calls opened to their single calls, by the index of their first call.
    open_runs: HashSet<usize>,
    /// The reply box is empty, so a live run shows Stop instead of Send.
    empty: bool,
    branch: Option<String>,
    /// The model was picked again since the CLI last said which one it runs, so that report no
    /// longer tells what the next run uses.
    repicked: bool,
    status: Status,
    ticker: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<TranscriptEvent> for TranscriptView {}

impl TranscriptView {
    /// `history` reads the thread's journal first; `first` is sent once that is done.
    pub fn new(
        id: SessionId,
        launch: Launch,
        journal: String,
        history: bool,
        first: Option<Prompt>,
        client: Client,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(IDLE_HINT, true, cx));
        let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Submit => this.submit(cx),
            InputEvent::Cancel => this.interrupt(cx),
            InputEvent::Images(images) => {
                this.images
                    .extend(images.iter().filter_map(|i| attach::save(i).ok()));
                cx.notify();
            }
            InputEvent::Changed => {
                let empty = this.input.read(cx).text().is_empty();
                if empty != this.empty {
                    this.empty = empty;
                    cx.notify();
                }
            }
        });
        let branch = branch::of(&launch.cwd);
        let mut view = Self {
            id,
            client,
            launch,
            journal,
            model: Transcript::default(),
            open: false,
            loading: history,
            queued: None,
            input,
            images: Vec::new(),
            scroll: ScrollHandle::new(),
            catalog: None,
            menu: None,
            anchor: Anchor::default(),
            effort_anchor: Anchor::default(),
            open_runs: HashSet::new(),
            empty: true,
            branch,
            repicked: false,
            status: Status::Idle,
            ticker: None,
            _subs: vec![sub],
        };
        if history {
            view.client.send(Command::LoadJournal {
                id,
                journal: view.journal.clone(),
            });
            view.queued = first;
        } else if let Some(prompt) = first {
            view.send(prompt, cx);
        }
        view
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Seconds the live run has taken, while one is live.
    pub fn elapsed(&self) -> Option<u64> {
        self.model.elapsed()
    }

    /// The tool the run is in, while it runs one.
    pub fn doing(&self) -> Option<String> {
        self.model.running_tool().map(tool::label)
    }

    /// The subagents still running: what each was asked, and seconds since it started.
    pub fn subagents(&self) -> Vec<(String, String, u64)> {
        self.model.live_agents()
    }

    pub fn agent(&self) -> hyprspace_proto::Agent {
        self.launch.agent
    }

    pub fn set_catalog(&mut self, catalog: AgentCatalog, cx: &mut Context<Self>) {
        self.catalog = Some(catalog);
        cx.notify();
    }

    pub fn note(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.model.note(text);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        if text.is_empty() && self.images.is_empty() {
            return;
        }
        let prompt = Prompt {
            text,
            images: std::mem::take(&mut self.images),
        };
        self.input.update(cx, |i, cx| i.set_text("", cx));
        if self.loading {
            self.queued = Some(prompt);
            return;
        }
        self.send(prompt, cx);
    }

    fn send(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        self.model.prompt(&prompt);
        if self.open {
            self.client.send(Command::Send {
                id: self.id,
                prompt,
            });
        } else {
            self.open = true;
            self.client.send(Command::OpenStructured {
                id: self.id,
                launch: self.launch.clone(),
                prompt: Some(prompt),
                journal: Some(self.journal.clone()),
            });
        }
        self.scroll.scroll_to_bottom();
        self.changed(cx);
    }

    fn interrupt(&mut self, _: &mut Context<Self>) {
        if self.model.running() {
            self.client.send(Command::Interrupt { id: self.id });
        }
    }

    fn answer(&mut self, request: String, answer: Answer, cx: &mut Context<Self>) {
        self.model.answered(&request, answer);
        self.client.send(Command::Approve {
            id: self.id,
            request,
            answer,
        });
        self.changed(cx);
    }

    fn pick_model(&mut self, model: String, cx: &mut Context<Self>) {
        // the 1M window carries over to a model that has one
        let current = self.launch.model.as_deref().unwrap_or_default();
        let long =
            crate::models::is_long(current) && crate::models::takes_long(self.launch.agent, &model);
        let model = Some(crate::models::windowed(&model, long)).filter(|m| !m.is_empty());
        if model == self.launch.model {
            return;
        }
        self.launch.model = model;
        self.repicked = true;
        // an effort the new model does not take would be refused at start
        if let (Some(cat), Some(effort)) = (&self.catalog, &self.launch.effort) {
            let id = self.launch.model.clone().unwrap_or_default();
            if !cat.efforts_for(&id).contains(effort) {
                self.launch.effort = None;
            }
        }
        let name = self.model_label();
        self.relaunch(format!("Model set to {name}."), cx);
    }

    fn pick_long(&mut self, long: bool, cx: &mut Context<Self>) {
        let Some(model) = self.launch.model.clone() else {
            return;
        };
        let model = crate::models::windowed(&model, long);
        if Some(&model) == self.launch.model.as_ref() {
            return;
        }
        self.launch.model = Some(model);
        let size = if long { "1M" } else { "standard" };
        self.relaunch(format!("Context window set to {size}."), cx);
    }

    fn pick_effort(&mut self, effort: String, cx: &mut Context<Self>) {
        let effort = Some(effort).filter(|e| !e.is_empty());
        if effort == self.launch.effort {
            return;
        }
        self.launch.effort = effort;
        let name = crate::composer::effort_label(self.launch.effort.as_deref().unwrap_or_default());
        self.relaunch(format!("Effort set to {name}."), cx);
    }

    /// After a model or effort change: the next prompt starts a session with it, resuming this
    /// conversation.
    fn relaunch(&mut self, what: String, cx: &mut Context<Self>) {
        if self.open && !self.model.running() {
            self.client.send(Command::Close { id: self.id });
            self.open = false;
        }
        self.model
            .note(format!("{what} It applies from your next message."));
        cx.emit(TranscriptEvent::Launch(self.launch.clone()));
        cx.notify();
    }

    /// What the model chip says: the picked model, or the one the CLI says it runs, by its
    /// name in the catalog.
    fn model_label(&self) -> String {
        let picked = self.launch.model.as_deref().filter(|m| !m.is_empty());
        let reported = self.model.model.as_deref().filter(|_| !self.repicked);
        match picked.or(reported) {
            Some(id) => crate::models::name(self.catalog.as_ref(), id),
            None => "Default".into(),
        }
    }

    pub fn apply(&mut self, event: RunEvent, cx: &mut Context<Self>) {
        if let RunEvent::Started { thread, cwd, .. } = &event {
            self.repicked = false;
            self.launch.resume = Some(thread.clone());
            self.launch.cwd = cwd.clone();
            cx.emit(TranscriptEvent::Started {
                thread: thread.clone(),
                cwd: cwd.clone(),
            });
        }
        // the agent may have switched branches during the run
        if matches!(event, RunEvent::Started { .. } | RunEvent::Finished { .. }) {
            self.branch = branch::of(&self.launch.cwd);
        }
        if matches!(event, RunEvent::Failed { .. }) {
            self.open = false;
        }
        let follow = self.at_bottom();
        self.model.apply(event);
        if follow {
            self.scroll.scroll_to_bottom();
        }
        self.changed(cx);
    }

    /// The engine could not start or reach the session.
    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.open = false;
        self.model.fail(message);
        self.scroll.scroll_to_bottom();
        self.changed(cx);
    }

    pub fn replay(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        if !self.loading {
            return;
        }
        for entry in entries {
            match entry {
                Entry::Prompt { prompt } => self.model.prompt(&prompt),
                Entry::Answer { request, answer } => self.model.answered(&request, answer),
                Entry::Run { event } => self.model.apply(event),
            }
        }
        self.model.settle();
        self.loading = false;
        self.scroll.scroll_to_bottom();
        if let Some(prompt) = self.queued.take() {
            self.send(prompt, cx);
        }
        self.changed(cx);
    }

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
            let _ = this.update(cx, |v, cx| {
                v.images
                    .extend(paths.into_iter().filter(|p| attach::is_image(p)));
                cx.notify();
            });
        })
        .detach();
    }

    fn at_bottom(&self) -> bool {
        let max = self.scroll.max_offset().y;
        -self.scroll.offset().y >= max - px(48.)
    }

    /// After anything that can move the status: tell the root, and keep a one-second tick
    /// going while a run or a subagent is live so their timers count.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let hint = if self.model.running() {
            RUNNING_HINT
        } else {
            IDLE_HINT
        };
        self.input.update(cx, |i, cx| i.set_placeholder(hint, cx));
        let status = self.model.status();
        if status != self.status {
            self.status = status;
            cx.emit(TranscriptEvent::Status(status));
        }
        if self.model.ticking() && self.ticker.is_none() {
            self.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let live = this
                        .update(cx, |v, cx| {
                            cx.notify();
                            v.model.ticking()
                        })
                        .unwrap_or(false);
                    if !live {
                        break;
                    }
                }
            }));
        } else if !self.model.ticking() {
            self.ticker = None;
        }
        cx.notify();
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

impl model_menu::Host for TranscriptView {
    fn model_spec(&self) -> Option<Spec> {
        let catalog = self.catalog.as_ref()?;
        let agent = self.launch.agent;
        let model = self.launch.model.clone().unwrap_or_default();
        // on Default, the CLI's own choice is the model it reported running
        let resolved = self
            .model
            .model
            .as_deref()
            .filter(|_| model.is_empty() && !self.repicked)
            .map(|id| crate::models::name(Some(catalog), id));
        let models = catalog
            .models
            .iter()
            .map(|m| {
                let (label, note) = match &resolved {
                    Some(r) if m.id.is_empty() => (format!("{} · {r}", m.label), None),
                    _ => (m.label.clone(), m.note.clone()),
                };
                model_menu::Model {
                    agent,
                    id: m.id.clone(),
                    label,
                    note,
                }
            })
            .collect();
        Some(Spec {
            models,
            efforts: catalog.efforts_for(&model).to_vec(),
            default_effort: catalog.model(&model).and_then(|m| m.default_effort.clone()),
            long: crate::models::takes_long(agent, &model).then(|| crate::models::is_long(&model)),
            agent,
            model,
            effort: self.launch.effort.clone().unwrap_or_default(),
            foot: self
                .launch
                .resume
                .is_some()
                .then_some("Switching keeps this conversation."),
        })
    }

    fn model_menu(&mut self) -> &mut Option<ModelMenu> {
        &mut self.menu
    }

    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
            Choice::Model(_, model) => self.pick_model(model, cx),
            Choice::Effort(effort) => self.pick_effort(effort, cx),
            Choice::Long(long) => self.pick_long(long, cx),
        }
    }
}

const IDLE_HINT: &str = "Ask for anything";
const RUNNING_HINT: &str = "Reply, or steer while it works";

impl Focusable for TranscriptView {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for TranscriptView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.model.parse();
        render::view(self, window, cx)
    }
}
