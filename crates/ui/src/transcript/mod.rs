// A structured thread: its transcript and the box under it. The view opens the session on the
// first prompt (resuming the CLI's conversation when the thread has one), sends later prompts
// as runs or steers, answers approvals, and tells the root when its status changes.

mod approval;
mod ask;
mod branch;
mod model;
mod motion;
mod render;
mod rows;
mod subagent;
mod tool;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Context, Entity, EventEmitter, ExternalPaths, Focusable, IntoElement,
    ListAlignment, ListState, PathPromptOptions, Render, Subscription, Task, Window, px,
};
use hyprspace_proto::agents::AgentCatalog;
use hyprspace_proto::{Answer, Client, Command, Entry, Launch, Prompt, RunEvent, SessionId};

use crate::attach;
use crate::composer::model_menu::{self, Anchor, Choice, ModelMenu, Spec};
use crate::input::{InputEvent, TextInput};
pub use model::Status;
use model::{Item, Transcript};

pub enum TranscriptEvent {
    Status(Status),
    /// The CLI is up: the id that resumes its conversation, and the folder it runs in.
    Started {
        thread: String,
        cwd: PathBuf,
    },
    /// The user picked another model for this thread.
    Launch(Launch),
    Queue(Vec<Prompt>),
    Limited(Option<u64>),
    ResumeAt(Option<u64>),
}

pub struct TranscriptView {
    id: SessionId,
    client: Client,
    launch: Launch,
    journal: String,
    delegate: bool,
    model: Transcript,
    /// A session is open in the engine.
    open: bool,
    /// Waiting for the journal; a prompt sent meanwhile waits in `queued`.
    loading: bool,
    queued: Option<Prompt>,
    queue: Vec<Prompt>,
    resume_at: Option<u64>,
    input: Entity<TextInput>,
    images: Vec<PathBuf>,
    saving: usize,
    send_saved: bool,
    list: ListState,
    rows: Vec<rows::Row>,
    remeasure: bool,
    rail: std::cell::RefCell<Vec<crate::slide::Glide>>,
    hover_tick: std::cell::Cell<Option<usize>>,
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
    stopping: bool,
    pub(super) suggest: crate::suggest::Suggest,
    pub(super) picks: std::collections::HashMap<String, Vec<Vec<usize>>>,
    pub(super) expanded: HashSet<usize>,
    pub(super) answered: std::collections::HashMap<String, Vec<(String, String)>>,
    pub(super) perm_menu: Option<gpui::Point<gpui::Pixels>>,
    follow: motion::Follow,
    reveal: Option<motion::Reveal>,
    partial: Option<(usize, Vec<crate::markdown::Block>)>,
    fresh: usize,
    seen: usize,
    born: HashMap<usize, Instant>,
    working: Option<Instant>,
    drag_step: Option<Instant>,
    ticker: Option<Task<()>>,
    _subs: Vec<Subscription>,
    _focus: Vec<Subscription>,
    remote: bool,
    echoed: VecDeque<String>,
}

impl EventEmitter<TranscriptEvent> for TranscriptView {}

impl TranscriptView {
    /// `history` reads the thread's journal first; `first` is sent once that is done.
    pub fn new(
        id: SessionId,
        launch: Launch,
        thread: &hyprspace_proto::Thread,
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
                this.saving += 1;
                attach::save_later(images.clone(), cx, |this, saved, cx| {
                    this.saving -= 1;
                    this.images.extend(saved);
                    if this.saving == 0 && std::mem::take(&mut this.send_saved) {
                        this.submit(cx);
                    }
                    cx.notify();
                })
            }
            InputEvent::Changed => {
                this.refresh_suggest(cx);
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
            journal: thread.journal(),
            delegate: thread.parent.is_none(),
            model: Transcript::default(),
            open: false,
            loading: history,
            queued: None,
            queue: thread.queue.clone(),
            resume_at: thread.resume_at,
            input,
            images: Vec::new(),
            saving: 0,
            send_saved: false,
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            rows: Vec::new(),
            remeasure: false,
            rail: Default::default(),
            hover_tick: Default::default(),
            catalog: None,
            menu: None,
            anchor: Anchor::default(),
            effort_anchor: Anchor::default(),
            open_runs: HashSet::new(),
            empty: true,
            branch,
            repicked: false,
            status: Status::Idle,
            stopping: false,
            suggest: Default::default(),
            picks: Default::default(),
            expanded: HashSet::new(),
            answered: Default::default(),
            perm_menu: None,
            follow: Default::default(),
            reveal: None,
            partial: None,
            fresh: 0,
            seen: 0,
            born: HashMap::new(),
            working: None,
            drag_step: None,
            ticker: None,
            _subs: vec![sub],
            _focus: Vec::new(),
            remote: false,
            echoed: VecDeque::new(),
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

    pub fn last_reply(&self) -> String {
        self.model.last_reply()
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

    pub fn set_resume(&mut self, at: Option<u64>, cx: &mut Context<Self>) {
        self.resume_at = at;
        cx.notify();
    }

    pub fn continue_now(&mut self, cx: &mut Context<Self>) {
        self.cancel_resume(cx);
        self.offer(Prompt::text(CONTINUE), cx);
    }

    pub(super) fn cancel_resume(&mut self, cx: &mut Context<Self>) {
        if self.resume_at.take().is_some() {
            cx.emit(TranscriptEvent::ResumeAt(None));
            cx.notify();
        }
    }

    pub fn set_catalog(&mut self, catalog: AgentCatalog, cx: &mut Context<Self>) {
        self.catalog = Some(catalog);
        cx.notify();
    }

    pub fn note(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.model.note(text);
        cx.notify();
    }

    /// A message from the phone, sent the way the reply box sends one.
    pub fn send_prompt(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        self.offer(prompt, cx);
    }

    /// An approval answered from the phone.
    pub fn approve(
        &mut self,
        request: String,
        answer: Answer,
        answers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.answer_with(request, answer, answers, cx);
    }

    /// The thread runs on another computer, which keeps its journal and sends what it records.
    pub fn set_remote(&mut self) {
        self.remote = true;
    }

    pub fn set_permission(
        &mut self,
        permission: hyprspace_proto::Permission,
        cx: &mut Context<Self>,
    ) {
        self.pick_permission(permission, cx);
    }

    /// Stop pressed on the phone.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.interrupt(cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.saving > 0 {
            self.send_saved = true;
            return;
        }
        let text = self.input.read(cx).text().trim().to_string();
        if text.is_empty() && self.images.is_empty() {
            return;
        }
        let prompt = Prompt {
            text,
            images: std::mem::take(&mut self.images),
        };
        self.input.update(cx, |i, cx| i.set_text("", cx));
        self.offer(prompt, cx);
    }

    fn offer(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        if self.loading {
            self.queued = Some(prompt);
        } else if self.model.running() {
            self.queue.push(prompt);
            self.queue_changed(cx);
        } else {
            self.send(prompt, cx);
        }
    }

    pub(super) fn steer(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.queue.len() {
            let prompt = self.queue.remove(ix);
            self.queue_changed(cx);
            self.send(prompt, cx);
        }
    }

    pub(super) fn unqueue(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.queue.len() {
            self.queue.remove(ix);
            self.queue_changed(cx);
        }
    }

    fn next_queued(&mut self, cx: &mut Context<Self>) {
        if !self.model.running() && !self.queue.is_empty() {
            let prompt = self.queue.remove(0);
            self.queue_changed(cx);
            self.send(prompt, cx);
        }
    }

    fn queue_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TranscriptEvent::Queue(self.queue.clone()));
        cx.notify();
    }

    fn send(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        self.cancel_resume(cx);
        self.model.prompt(&prompt);
        if self.remote {
            self.echoed.push_back(prompt.text.clone());
        }
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
                delegate: self.delegate,
            });
        }
        self.follow.down(&self.list);
        self.changed(cx);
    }

    fn interrupt(&mut self, cx: &mut Context<Self>) {
        if self.model.running() {
            self.client.send(Command::Interrupt { id: self.id });
            self.stopping = true;
            cx.notify();
        }
    }

    fn answer(&mut self, request: String, answer: Answer, cx: &mut Context<Self>) {
        self.answer_with(request, answer, Vec::new(), cx);
    }

    pub(super) fn answer_with(
        &mut self,
        request: String,
        answer: Answer,
        answers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.model.answered(&request, answer);
        if !answers.is_empty() {
            self.answered.insert(request.clone(), answers.clone());
        }
        self.client.send(Command::Approve {
            id: self.id,
            request,
            answer,
            answers,
        });
        self.changed(cx);
    }

    pub(super) fn pick_option(
        &mut self,
        request: &str,
        question: usize,
        option: usize,
        multi: bool,
        cx: &mut Context<Self>,
    ) {
        let picks = self.picks.entry(request.to_string()).or_default();
        if picks.len() <= question {
            picks.resize(question + 1, Vec::new());
        }
        let chosen = &mut picks[question];
        match (multi, chosen.contains(&option)) {
            (true, true) => chosen.retain(|&o| o != option),
            (true, false) => chosen.push(option),
            (false, _) => *chosen = vec![option],
        }
        cx.notify();
    }

    pub(super) fn approve_plan(&mut self, request: String, cx: &mut Context<Self>) {
        self.answer_with(request, Answer::Allow, Vec::new(), cx);
        if self.launch.permission == hyprspace_proto::Permission::Plan {
            self.launch.permission = hyprspace_proto::Permission::Ask;
            cx.emit(TranscriptEvent::Launch(self.launch.clone()));
        }
    }

    pub fn repick(&mut self, model: String, effort: String, cx: &mut Context<Self>) {
        if self.model.running() {
            return;
        }
        self.pick_model(model, cx);
        self.pick_effort(effort, cx);
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
    pub(super) fn pick_permission(
        &mut self,
        permission: hyprspace_proto::Permission,
        cx: &mut Context<Self>,
    ) {
        self.perm_menu = None;
        if permission == self.launch.permission || self.model.running() {
            cx.notify();
            return;
        }
        self.launch.permission = permission;
        let name = crate::composer::pickers::permission_label(permission);
        self.relaunch(format!("Permission set to {name}."), cx);
    }

    fn relaunch(&mut self, what: String, cx: &mut Context<Self>) {
        if self.open && !self.model.running() && !self.remote {
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
        if let RunEvent::Commands { names } = &event {
            crate::suggest::set_commands(self.launch.agent, names.clone(), cx);
        }
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
        if let RunEvent::Limited { resets } = &event {
            cx.emit(TranscriptEvent::Limited(*resets));
        }
        let done = matches!(
            event,
            RunEvent::Finished {
                status: hyprspace_proto::RunStatus::Done,
                ..
            }
        );
        let n = self.model.items.len();
        let had = match self.model.items.last() {
            Some(Item::Text { source, .. }) => source.len(),
            _ => 0,
        };
        self.model.apply(event);
        let text = self.model.items[n.saturating_sub(1)..]
            .iter()
            .rposition(|i| matches!(i, Item::Text { .. }))
            .map(|i| i + n.saturating_sub(1));
        if let Some(ix) = text
            && self.reveal.as_ref().is_none_or(|r| r.ix != ix)
        {
            let from = if ix + 1 == n { had } else { 0 };
            self.reveal = Some(motion::Reveal::new(ix, from));
        }
        self.changed(cx);
        if done {
            self.next_queued(cx);
        }
    }

    /// The engine could not start or reach the session.
    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.open = false;
        self.model.fail(message);
        self.follow.down(&self.list);
        self.changed(cx);
    }

    pub fn replay(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        // a host sends its whole journal again after the connection comes back
        if !self.loading && !self.remote {
            return;
        }
        if !self.loading {
            self.model = Transcript::default();
            self.rows.clear();
            self.list.reset(0);
            self.reveal = None;
            self.partial = None;
            self.seen = 0;
            self.born.clear();
            self.echoed.clear();
        }
        for entry in entries {
            match entry {
                Entry::Prompt { prompt } => self.model.prompt(&prompt),
                Entry::Answer {
                    request,
                    answer,
                    answers,
                } => {
                    self.model.answered(&request, answer);
                    if !answers.is_empty() {
                        self.answered.insert(request, answers);
                    }
                }
                Entry::Run { event } => self.model.apply(event),
            }
        }
        if !self.remote {
            self.model.settle();
        }
        self.loading = false;
        self.fresh = self.model.items.len();
        self.follow.snap(&self.list);
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
                v.images.extend(paths.into_iter().filter(|p| p.is_file()));
                cx.notify();
            });
        })
        .detach();
    }

    /// What a host recorded after the journal it sent: its own copy of a prompt sent from here
    /// is already showing.
    pub fn recorded(&mut self, entries: Vec<Entry>, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        for entry in entries {
            match entry {
                Entry::Prompt { prompt } => {
                    if self.echoed.front() == Some(&prompt.text) {
                        self.echoed.pop_front();
                    } else {
                        self.model.prompt(&prompt);
                        self.changed(cx);
                    }
                }
                Entry::Answer {
                    request,
                    answer,
                    answers,
                } => {
                    self.model.answered(&request, answer);
                    if !answers.is_empty() {
                        self.answered.insert(request, answers);
                    }
                    self.changed(cx);
                }
                Entry::Run { event } => self.apply(event, cx),
            }
        }
    }

    pub fn refresh_suggest(&mut self, cx: &mut Context<Self>) {
        let cwd = self
            .model
            .cwd
            .clone()
            .unwrap_or_else(|| self.launch.cwd.clone());
        self.suggest
            .refresh(&self.input, self.launch.agent, &cwd, &self.client, cx);
        cx.notify();
    }

    pub(super) fn take_suggestion(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.suggest.accept(Some(ix), &self.input, cx);
    }

    pub(super) fn suggest_prev(
        &mut self,
        _: &crate::suggest::Prev,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suggest.step(-1);
        cx.notify();
    }

    pub(super) fn suggest_next(
        &mut self,
        _: &crate::suggest::Next,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suggest.step(1);
        cx.notify();
    }

    pub(super) fn suggest_accept(
        &mut self,
        _: &crate::suggest::Accept,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suggest.accept(None, &self.input, cx);
    }

    pub(super) fn suggest_dismiss(
        &mut self,
        _: &crate::suggest::Dismiss,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.suggest.dismiss();
        cx.notify();
    }

    /// After anything that can move the status: tell the root, and keep a one-second tick
    /// going while a run or a subagent is live so their timers count.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let hint = if self.model.running() {
            RUNNING_HINT
        } else {
            IDLE_HINT
        };
        let prompts: Vec<String> = self
            .model
            .items
            .iter()
            .filter_map(|i| match i {
                Item::User { text, .. } if !text.trim().is_empty() => Some(text.clone()),
                _ => None,
            })
            .collect();
        self.input.update(cx, |i, cx| {
            i.set_placeholder(hint, cx);
            i.set_history(prompts);
        });
        if !self.model.running() {
            self.stopping = false;
        }
        self.remeasure = true;
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

    fn sync_rows(&mut self, rows: Vec<rows::Row>) {
        let same = self
            .rows
            .iter()
            .zip(&rows)
            .take_while(|(a, b)| a == b)
            .count();
        if same < self.rows.len() || rows.len() > same {
            self.list.splice(same..self.rows.len(), rows.len() - same);
        }
        if std::mem::take(&mut self.remeasure) && same > 0 {
            self.list.remeasure_items(0..same);
        }
        self.rows = rows;
    }

    /// Files dropped on the thread: images go in with the next message, anything else has its
    /// path typed in at the cursor, as a terminal does, and the box takes the keyboard.
    fn drop_paths(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let (images, text) = attach::split_drop(paths.paths());
        self.images.extend(images);
        if !text.is_empty() {
            self.input.update(cx, |i, cx| i.insert(&text, cx));
        }
        window.focus(&self.input.focus_handle(cx), cx);
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
const CONTINUE: &str = "The usage limit has reset. Continue where you left off.";
const RUNNING_HINT: &str = "Queue a message for when it finishes";

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
