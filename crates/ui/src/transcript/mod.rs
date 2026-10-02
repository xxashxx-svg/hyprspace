// A structured thread: its transcript and the box under it. The view opens the session on the
// first prompt (resuming the CLI's conversation when the thread has one), sends later prompts
// as runs or steers, answers approvals, and tells the root when its status changes.

mod approval;
mod model;
mod render;
mod tool;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, EventEmitter, ExternalPaths, Focusable, IntoElement, Pixels,
    Point, Render, ScrollHandle, Subscription, Task, Window, px,
};
use hyprspace_proto::agents::AgentCatalog;
use hyprspace_proto::{Answer, Client, Command, Entry, Launch, Prompt, RunEvent, SessionId};

use crate::attach;
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
    /// The model picker, open at this point.
    menu: Option<Point<Pixels>>,
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
        let input = cx.new(|cx| TextInput::new("Reply, or steer the run while it works", true, cx));
        let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Submit => this.submit(cx),
            InputEvent::Cancel => this.interrupt(cx),
            InputEvent::Images(images) => {
                this.images
                    .extend(images.iter().filter_map(|i| attach::save(i).ok()));
                cx.notify();
            }
            InputEvent::Changed => {}
        });
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
        self.menu = None;
        let model = Some(model).filter(|m| !m.is_empty());
        if model == self.launch.model {
            cx.notify();
            return;
        }
        self.launch.model = model;
        // an effort the new model does not take would be refused at start
        if let (Some(cat), Some(effort)) = (&self.catalog, &self.launch.effort) {
            let id = self.launch.model.clone().unwrap_or_default();
            if !cat.efforts_for(&id).contains(effort) {
                self.launch.effort = None;
            }
        }
        // the next prompt starts a session on the new model, resuming this conversation
        if self.open && !self.model.running() {
            self.client.send(Command::Close { id: self.id });
            self.open = false;
        }
        let name = self.model_label();
        self.model.note(format!(
            "Model set to {name}. It applies from your next message."
        ));
        cx.emit(TranscriptEvent::Launch(self.launch.clone()));
        cx.notify();
    }

    /// What the model chip says: the picked model's label, or the CLI's own name for it.
    fn model_label(&self) -> String {
        let id = self.launch.model.clone().unwrap_or_default();
        let from_catalog = self
            .catalog
            .as_ref()
            .and_then(|c| c.models.iter().find(|m| m.id == id))
            .filter(|m| !m.id.is_empty())
            .map(|m| m.label.clone());
        from_catalog
            .or_else(|| self.model.model.clone())
            .unwrap_or_else(|| format!("{} default", self.launch.agent.name()))
    }

    pub fn apply(&mut self, event: RunEvent, cx: &mut Context<Self>) {
        if let RunEvent::Started { thread, cwd, .. } = &event {
            self.launch.resume = Some(thread.clone());
            self.launch.cwd = cwd.clone();
            cx.emit(TranscriptEvent::Started {
                thread: thread.clone(),
                cwd: cwd.clone(),
            });
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

    fn at_bottom(&self) -> bool {
        let max = self.scroll.max_offset().y;
        -self.scroll.offset().y >= max - px(48.)
    }

    /// After anything that can move the status: tell the root, and keep a one-second tick
    /// going while a run is live so its timer counts.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let status = self.model.status();
        if status != self.status {
            self.status = status;
            cx.emit(TranscriptEvent::Status(status));
        }
        if self.model.running() && self.ticker.is_none() {
            self.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let live = this
                        .update(cx, |v, cx| {
                            cx.notify();
                            v.model.running()
                        })
                        .unwrap_or(false);
                    if !live {
                        break;
                    }
                }
            }));
        } else if !self.model.running() {
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
