// A structured thread: its transcript and the box under it. The view opens the session on the
// first prompt (resuming the CLI's conversation when the thread has one), sends later prompts
// as runs or steers, answers approvals, and tells the root when its status changes.

mod approval;
mod aside;
mod ask;
mod autoscroll;
mod background;
mod banner;
mod branch;
mod changes;
mod cites;
mod commands;
mod doing;
mod find;
mod links;
mod model;
mod motion;
mod render;
mod ring;
mod rows;
mod subagent;
mod tool;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Context, Entity, EventEmitter, ExternalPaths, Focusable, IntoElement, KeyBinding,
    ListAlignment, ListState, PathPromptOptions, Render, Subscription, Task, Window, actions,
};
use hyprspace_proto::agents::AgentCatalog;
use hyprspace_proto::{Answer, Cite, Client, Command, Entry, Launch, Prompt, RunEvent, SessionId};

use crate::attach;
use crate::composer::model_menu::{self, Anchor, Choice, ModelMenu, Spec};
use crate::input::{InputEvent, TextInput};
use crate::zoom::px;
pub use model::Status;
use model::{Item, Transcript};

pub(crate) use commands::for_phone as phone_commands;

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
    Draft(String, Vec<PathBuf>, Vec<Cite>),
    SwitchFailed,
    ShowMedia(PathBuf),
    Commands(
        hyprspace_proto::Agent,
        Vec<String>,
        Vec<hyprspace_proto::CommandInfo>,
    ),
    Open(hyprspace_proto::Pane),
    ViewChanges,
    NewThread,
    /// `/login`: sign the agent in again in a shell under the thread.
    SignIn,
    Rename(String),
    Settings(crate::settings::Tab),
    Reference(u64),
    Fork {
        launch: Launch,
        keep: usize,
        text: String,
        images: Vec<PathBuf>,
    },
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
    /// Background work to name when a run cut off by an update picks back up.
    updated: Option<Vec<String>>,
    pub(super) cut: Option<usize>,
    draft: Option<Task<()>>,
    probing: bool,
    after: Option<String>,
    input: Entity<TextInput>,
    images: Vec<PathBuf>,
    cites: Vec<Cite>,
    noting: Option<cites::Noting>,
    saving: usize,
    peek: Option<(usize, gpui::Point<gpui::Pixels>)>,
    /// A sent message's image whose marker the pointer is on: the row, and the image's number.
    lit: Option<(usize, usize)>,
    send_saved: bool,
    list: ListState,
    rows: Vec<rows::Row>,
    remeasure: bool,
    rail: std::cell::RefCell<Vec<crate::slide::Glide>>,
    hover_tick: std::cell::Cell<Option<usize>>,
    rail_near: std::cell::Cell<bool>,
    rail_fade: std::cell::Cell<(f32, Option<Instant>)>,
    rail_stir: (gpui::Pixels, Option<Instant>),
    rail_until: Option<Instant>,
    rail_timer: Option<Task<()>>,
    requeue_at: Option<usize>,
    task_open: Option<String>,
    task_log: Option<String>,
    aside: Option<aside::Card>,
    /// The agent CLI's version, for `/status`.
    cli_version: Option<String>,
    provider: Option<hyprspace_proto::agents::ProviderStatus>,
    task_poll: Option<Task<()>>,
    catalog: Option<AgentCatalog>,
    menu: Option<ModelMenu>,
    want_menu: Option<bool>,
    pending: bool,
    /// Where the model chip sits, for its menu to open from.
    anchor: Anchor,
    effort_anchor: Anchor,
    /// Runs of tool calls opened to their single calls, by the index of their first call.
    open_runs: HashSet<usize>,
    /// The reply box is empty, so a live run shows Stop instead of Send.
    empty: bool,
    ultra: bool,
    branch: Option<String>,
    /// The model was picked again since the CLI last said which one it runs, so that report no
    /// longer tells what the next run uses.
    repicked: bool,
    status: Status,
    stopping: bool,
    pub(super) suggest: crate::suggest::Suggest,
    pub(super) picks: std::collections::HashMap<String, Vec<Vec<usize>>>,
    pub(super) steps: std::collections::HashMap<String, usize>,
    pub(super) expanded: HashSet<usize>,
    pub(super) unfolded: HashSet<usize>,
    typed_at: Option<Instant>,
    /// What each message in the box's Up history had attached, in the history's order.
    recall_images: Vec<Vec<PathBuf>>,
    pub(super) held: HashSet<String>,
    pub(super) turn_flip: Option<(usize, bool)>,
    pub(super) turn_primed: bool,
    pub(super) appear: Option<(Instant, Option<Instant>)>,
    autoscroll: Option<autoscroll::Autoscroll>,
    pub(super) turn_anim: Option<(usize, bool, HashSet<usize>)>,
    pub(super) answered: std::collections::HashMap<String, Vec<(String, String)>>,
    /// What was typed for each question's Other choice, and the field open for one.
    pub(super) others: std::collections::HashMap<String, Vec<String>>,
    pub(super) other: Option<ask::Other>,
    pub(super) perm_menu: Option<gpui::Point<gpui::Pixels>>,
    follow: motion::Follow,
    reveal: Option<motion::Reveal>,
    partial: Option<(usize, Vec<crate::markdown::Block>)>,
    /// The reply and its length that `partial` is the parse of.
    partial_at: Option<(usize, usize)>,
    stream: (usize, crate::markdown::Stream),
    pub(super) fade: crate::markdown::Fade,
    fresh: usize,
    seen: usize,
    /// How many items the thread had when it was last drawn, and the first one that came after
    /// that while it was off screen.
    read: usize,
    unread: Option<usize>,
    born: HashMap<usize, Instant>,
    working: Option<Instant>,
    doing: doing::Doing,
    drag_step: Option<Instant>,
    ticker: Option<Task<()>>,
    _subs: Vec<Subscription>,
    _focus: Vec<Subscription>,
    remote: bool,
    echoed: VecDeque<String>,
    find: Option<find::Find>,
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
        let input = cx.new(|cx| {
            let mut input = TextInput::new(IDLE_HINT, true, cx);
            input.set_text(thread.draft.clone(), cx);
            input
        });
        let sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Submit => this.submit(cx),
            // Esc ends autoscroll before it stops the agent
            InputEvent::Cancel if this.autoscroll.take().is_some() => cx.notify(),
            InputEvent::Cancel => this.interrupt(cx),
            InputEvent::Images(images) => {
                let asked = images.len();
                attach::mark(
                    &this.input,
                    this.images.len() + this.saving + 1,
                    &vec!["Image"; asked],
                    cx,
                );
                this.saving += asked;
                attach::save_later(images.clone(), cx, move |this, saved, cx| {
                    this.saving -= asked;
                    this.images.extend(saved);
                    this.keep_draft(cx);
                    if this.saving == 0 && std::mem::take(&mut this.send_saved) {
                        this.submit(cx);
                    }
                    cx.notify();
                })
            }
            InputEvent::Peek(at) => {
                this.peek = *at;
                cx.notify();
            }
            InputEvent::Open(n) => {
                if let Some(path) = n.checked_sub(1).and_then(|i| this.images.get(i)) {
                    cx.emit(TranscriptEvent::ShowMedia(path.clone()));
                }
            }
            InputEvent::Unmarked(gone) => {
                attach::drop_unmarked(&mut this.images, gone, &this.input, cx);
                this.keep_draft(cx);
                cx.notify();
            }
            // a message brought back comes with what it had attached, as the box sent it
            InputEvent::Recalled(at) => {
                this.images = at
                    .and_then(|i| this.recall_images.get(i))
                    .map(|all| all.iter().filter(|p| p.exists()).cloned().collect())
                    .unwrap_or_default();
                this.keep_draft(cx);
                cx.notify();
            }
            InputEvent::Changed => {
                this.typed_at = Some(Instant::now());
                this.keep_draft(cx);
                this.refresh_suggest(cx);
                let text = this.input.read(cx).text();
                let (empty, ultra) = (text.is_empty(), ring::says_ultrathink(text));
                if (empty, ultra) != (this.empty, this.ultra) {
                    (this.empty, this.ultra) = (empty, ultra);
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
            updated: None,
            cut: None,
            draft: None,
            probing: false,
            after: None,
            input,
            images: thread
                .draft_images
                .iter()
                .filter(|p| p.exists())
                .cloned()
                .collect(),
            cites: thread.draft_cites.clone(),
            noting: None,
            saving: 0,
            peek: None,
            lit: None,
            send_saved: false,
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            rows: Vec::new(),
            remeasure: false,
            rail: Default::default(),
            hover_tick: Default::default(),
            rail_near: Default::default(),
            rail_fade: Default::default(),
            rail_stir: Default::default(),
            rail_until: None,
            rail_timer: None,
            requeue_at: None,
            task_open: None,
            task_log: None,
            aside: None,
            cli_version: None,
            provider: None,
            task_poll: None,
            catalog: None,
            menu: None,
            want_menu: None,
            pending: false,
            anchor: Anchor::default(),
            effort_anchor: Anchor::default(),
            open_runs: HashSet::new(),
            empty: thread.draft.is_empty(),
            ultra: ring::says_ultrathink(&thread.draft),
            branch,
            repicked: false,
            status: Status::Idle,
            stopping: false,
            suggest: crate::suggest::Suggest::with(commands::items()).with_threads(),
            picks: Default::default(),
            steps: Default::default(),
            expanded: HashSet::new(),
            unfolded: HashSet::new(),
            typed_at: None,
            recall_images: Vec::new(),
            held: HashSet::new(),
            turn_flip: None,
            turn_primed: false,
            appear: None,
            autoscroll: None,
            turn_anim: None,
            answered: Default::default(),
            others: Default::default(),
            other: None,
            perm_menu: None,
            follow: Default::default(),
            reveal: None,
            partial: None,
            partial_at: None,
            stream: Default::default(),
            fade: Default::default(),
            fresh: 0,
            seen: 0,
            read: 0,
            unread: None,
            born: HashMap::new(),
            working: None,
            doing: Default::default(),
            drag_step: None,
            ticker: None,
            _subs: vec![sub],
            _focus: Vec::new(),
            remote: false,
            echoed: VecDeque::new(),
            find: None,
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

    /// Reads the branch again when the one under `path` was renamed outside a run.
    pub fn branch_moved(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        if self.launch.cwd == path {
            self.branch = branch::of(&self.launch.cwd);
            cx.notify();
        }
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

    pub fn elapsed_ms(&self) -> Option<u64> {
        self.model.elapsed_ms()
    }

    /// The tool the run is in, while it runs one.
    pub fn doing(&self) -> Option<String> {
        if self.model.compacting {
            return Some("Compacting".into());
        }
        self.model.running_tool().map(tool::label)
    }

    pub fn background_line(&self) -> Option<String> {
        let tasks: Vec<_> = self
            .model
            .background
            .iter()
            .filter(|t| t.kind != hyprspace_proto::TaskKind::Subagent)
            .collect();
        match tasks.as_slice() {
            [] => None,
            [t] if !t.label.trim().is_empty() => Some(format!("In the background: {}", t.label)),
            [_] => Some("1 task in the background".into()),
            all => Some(format!("{} tasks in the background", all.len())),
        }
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

    pub fn open_switched(&mut self, cx: &mut Context<Self>) {
        self.probing = true;
        self.after = (self.launch.agent != hyprspace_proto::Agent::Claude).then(|| {
            format!(
                "Messages sent in the terminal aren't shown here. {} still has them.",
                self.launch.agent.name()
            )
        });
        self.open = true;
        self.client.send(Command::OpenStructured {
            id: self.id,
            launch: self.launch.clone(),
            prompt: None,
            journal: Some(self.journal.clone()),
            delegate: self.delegate,
        });
        cx.notify();
    }

    pub(super) fn resume_cut(&mut self, cx: &mut Context<Self>) {
        let prompt = self.unanswered().unwrap_or_else(|| Prompt::text(RESUME));
        self.offer(prompt, cx);
    }

    fn unanswered(&self) -> Option<Prompt> {
        self.model.unanswered(self.cut?)
    }

    /// Comes on screen: what arrived while it was away shows as it is, and only what streams in
    /// from here on animates. A view off screen draws nothing, so its reveal fell behind.
    pub fn come_on_screen(&mut self) {
        self.follow.settle();
        self.unread = (self.read > 0 && self.read < self.model.items.len()).then_some(self.read);
        self.reveal = None;
        self.partial = None;
        self.partial_at = None;
        self.stream = Default::default();
        self.fade = Default::default();
        self.seen = self.model.items.len();
        self.born.clear();
    }

    /// The app quit to install an update while this thread ran: once its journal shows the run
    /// was cut off, it carries on, told what background work stopped with it.
    pub fn pick_up_after_update(&mut self, work: Vec<String>, cx: &mut Context<Self>) {
        self.updated = Some(work);
        self.continue_updated(cx);
    }

    fn continue_updated(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.cut.is_none() {
            return;
        }
        if let Some(work) = self.updated.take() {
            let prompt = self
                .unanswered()
                .unwrap_or_else(|| Prompt::text(after_update(&work)));
            self.offer(prompt, cx);
        }
    }

    /// The background work a run has going, as the agent would know it.
    pub fn tasks(&self) -> &[hyprspace_proto::Task] {
        &self.model.background
    }

    pub fn background_work(&self) -> Vec<String> {
        self.model
            .background
            .iter()
            .map(|t| t.label.clone())
            .collect()
    }

    fn light(&mut self, row: usize, image: Option<usize>, cx: &mut Context<Self>) {
        let lit = image.map(|n| (row, n));
        let mine = self.lit.is_some_and(|(r, _)| r == row);
        if lit != self.lit && (lit.is_some() || mine) {
            self.lit = lit;
            cx.notify();
        }
    }

    fn keep_draft(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().to_string();
        let images = self.images.clone();
        let cites = self.cites.clone();
        if text.is_empty() && images.is_empty() && cites.is_empty() {
            self.draft = None;
            cx.emit(TranscriptEvent::Draft(text, images, cites));
            return;
        }
        self.draft = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(600))
                .await;
            let _ = this.update(cx, |_, cx| {
                cx.emit(TranscriptEvent::Draft(text, images, cites))
            });
        }));
    }

    pub(super) fn cancel_resume(&mut self, cx: &mut Context<Self>) {
        if self.resume_at.take().is_some() {
            cx.emit(TranscriptEvent::ResumeAt(None));
            cx.notify();
        }
    }

    pub fn set_catalog(
        &mut self,
        catalog: AgentCatalog,
        status: hyprspace_proto::agents::ProviderStatus,
        cx: &mut Context<Self>,
    ) {
        self.catalog = Some(catalog);
        self.cli_version = status.version.clone();
        self.provider = Some(status);
        cx.notify();
    }

    pub fn gist(&self) -> String {
        self.model
            .items
            .iter()
            .filter_map(|i| match i {
                Item::User { text, .. } => Some(format!("You: {}", text.trim())),
                Item::Text { source, .. } => Some(format!("Agent: {}", source.trim())),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn note(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.model.note(text);
        cx.notify();
    }

    /// A message from the phone, sent the way the reply box sends one. A command runs here as it
    /// would from the box, and the lines it adds come back for the phone.
    pub fn send_prompt(&mut self, prompt: Prompt, cx: &mut Context<Self>) -> Vec<String> {
        if let Some(slash) = self.command(prompt.text.trim(), !prompt.images.is_empty(), cx) {
            let before = self.model.items.len();
            self.run_slash(slash, true, prompt.images, cx);
            return self.model.items[before.min(self.model.items.len())..]
                .iter()
                .filter_map(|i| match i {
                    model::Item::Note(text) => Some(text.clone()),
                    _ => None,
                })
                .collect();
        }
        self.redirect(prompt, cx);
        Vec::new()
    }

    /// An approval answered from the phone. An approved plan leaves plan mode, as a click does.
    pub fn approve(
        &mut self,
        request: String,
        answer: Answer,
        answers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let plan = self.model.items.iter().any(|i| {
            matches!(i, Item::Approval { request: r, tool, .. }
                if *r == request && ask::named(tool).is_some_and(|(n, _)| n == ask::PLAN))
        });
        if plan && answer == Answer::Allow {
            self.approve_plan(request, cx);
        } else {
            self.answer_with(request, answer, answers, cx);
        }
    }

    pub fn blocked(&self) -> bool {
        !self.remote
            && self
                .provider
                .as_ref()
                .is_some_and(|s| !s.installed || s.signed_out())
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
        let text = cites::with_cites(&self.cites, self.input.read(cx).text().trim());
        if text.is_empty() && self.images.is_empty() {
            return;
        }
        self.cites.clear();
        self.noting = None;
        if let Some(slash) = self.command(&text, !self.images.is_empty(), cx) {
            self.input.update(cx, |i, cx| i.set_text("", cx));
            let images = std::mem::take(&mut self.images);
            self.run_slash(slash, false, images, cx);
            self.keep_draft(cx);
            return;
        }
        let prompt = Prompt {
            text,
            images: std::mem::take(&mut self.images),
        };
        self.input.update(cx, |i, cx| i.set_text("", cx));
        self.redirect(prompt, cx);
    }

    /// The command `text` runs. With something attached only `/btw` is one, since it takes the
    /// attachments along; anything else goes to the agent as a message.
    fn command(&self, text: &str, attached: bool, cx: &mut gpui::App) -> Option<commands::Slash> {
        let slash = commands::parse(text, &crate::suggest::commands_of(self.launch.agent, cx))?;
        (!attached || matches!(slash, commands::Slash::Own(commands::Own::Btw, _))).then_some(slash)
    }

    fn release_held(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TYPING).await;
                let idle = this.update(cx, |v, cx| {
                    if v.typed_at.is_some_and(|t| t.elapsed() < TYPING) {
                        return false;
                    }
                    v.held.clear();
                    v.changed(cx);
                    true
                });
                if !matches!(idle, Ok(false)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn redirect(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        let open: Vec<String> = approval::open(self)
            .into_iter()
            .filter_map(|i| match i {
                Item::Approval { request, .. } => Some(request.clone()),
                _ => None,
            })
            .collect();
        if open.is_empty() {
            return self.offer(prompt, cx);
        }
        for request in open {
            self.answer(request, Answer::Deny, cx);
        }
        self.send(prompt, cx);
    }

    fn offer(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        let at = self.requeue_at.take();
        if self.loading {
            self.queued = Some(prompt);
        } else if self.model.running() {
            let at = at.map_or(self.queue.len(), |at| at.min(self.queue.len()));
            self.queue.insert(at, prompt);
            self.queue_changed(cx);
        } else {
            self.send(prompt, cx);
        }
    }

    /// A queued message the phone acted on: sent now, or dropped. It's found at `index` when the
    /// words there still match, else by its words, since the queue may have moved since.
    pub fn queued(&mut self, index: usize, text: &str, send: bool, cx: &mut Context<Self>) {
        let at = match self.queue.get(index) {
            Some(p) if p.text == text => Some(index),
            _ => self.queue.iter().position(|p| p.text == text),
        };
        match at {
            Some(ix) if send => self.steer(ix, cx),
            Some(ix) => self.unqueue(ix, cx),
            None => {}
        }
    }

    pub(super) fn steer(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.queue.len() {
            let prompt = self.queue.remove(ix);
            self.queue_changed(cx);
            self.send(prompt, cx);
        }
    }

    pub(super) fn edit_queued(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.queue.len() {
            return;
        }
        let prompt = self.queue.remove(ix);
        self.queue_changed(cx);
        self.requeue_at = Some(ix);
        let draft = self.input.read(cx).text().trim().to_string();
        let text = match draft.is_empty() {
            true => prompt.text,
            false => format!("{}\n\n{draft}", prompt.text),
        };
        self.images.splice(0..0, prompt.images);
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        window.focus(&self.input.focus_handle(cx), cx);
    }

    pub fn add_output(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let block = format!("```\n{}\n```", text.trim_matches(['\r', '\n']).trim_end());
        let draft = self.input.read(cx).text().trim_end().to_string();
        let text = match draft.is_empty() {
            true => format!("{block}\n\n"),
            false => format!("{draft}\n\n{block}\n\n"),
        };
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        window.focus(&self.input.focus_handle(cx), cx);
    }

    pub(super) fn move_queued(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.queue.len() || to >= self.queue.len() {
            return;
        }
        let prompt = self.queue.remove(from);
        self.queue.insert(to, prompt);
        self.queue_changed(cx);
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
        self.cut = None;
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

    pub(super) fn toggle_task(&mut self, task: String, cx: &mut Context<Self>) {
        self.task_log = None;
        if self.task_open.as_ref() == Some(&task) {
            self.task_open = None;
            self.task_poll = None;
            cx.notify();
            return;
        }
        let reads = self
            .model
            .background
            .iter()
            .any(|b| b.id == task && background::reads(b));
        self.task_open = Some(task);
        if !reads {
            self.task_poll = None;
            cx.notify();
            return;
        }
        self.task_poll = Some(cx.spawn(async move |this, cx| {
            loop {
                let live = this
                    .update(cx, |v, _| {
                        let Some(task) = v
                            .task_open
                            .clone()
                            .filter(|t| v.model.background.iter().any(|b| &b.id == t))
                        else {
                            return false;
                        };
                        v.client.send(Command::TaskOutput { id: v.id, task });
                        true
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
                cx.background_executor().timer(Duration::from_secs(1)).await;
            }
        }));
        cx.notify();
    }

    pub fn stop_task(&mut self, task: String, cx: &mut Context<Self>) {
        self.client.send(Command::StopTask { id: self.id, task });
        cx.notify();
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
        if self.other.as_ref().is_some_and(|o| o.is_for(&request)) {
            self.other = None;
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

    pub(super) fn show_question(&mut self, request: &str, question: usize, cx: &mut Context<Self>) {
        self.steps.insert(request.to_string(), question);
        cx.notify();
    }

    pub(super) fn approve_plan(&mut self, request: String, cx: &mut Context<Self>) {
        self.answer_with(request, Answer::Allow, Vec::new(), cx);
        if self.launch.permission == hyprspace_proto::Permission::Plan {
            self.launch.permission = hyprspace_proto::Permission::Ask;
            cx.emit(TranscriptEvent::Launch(self.launch.clone()));
        }
    }

    pub fn repick(&mut self, model: String, effort: String, fast: bool, cx: &mut Context<Self>) {
        if self.model.running() {
            return;
        }
        self.pick_model(model, cx);
        self.pick_effort(effort, cx);
        self.pick_fast(fast, cx);
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
        let id = self.launch.model.clone().unwrap_or_default();
        if !crate::models::takes_fast(self.launch.agent, &id) {
            self.launch.fast = false;
        }
        // an effort the new model does not take would be refused at start
        if let (Some(cat), Some(effort)) = (&self.catalog, &self.launch.effort) {
            let id = self.launch.model.clone().unwrap_or_default();
            if !cat.efforts_for(&id).contains(effort) {
                self.launch.effort = None;
            }
        }
        let name = self.model_label();
        self.change(format!("Model set to {name}."), cx);
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
        self.change(format!("Context window set to {size}."), cx);
    }

    fn pick_fast(&mut self, fast: bool, cx: &mut Context<Self>) {
        if fast == self.launch.fast {
            return;
        }
        self.launch.fast = fast;
        let state = if fast { "on" } else { "off" };
        self.change(format!("Fast mode {state}."), cx);
    }

    fn pick_effort(&mut self, effort: String, cx: &mut Context<Self>) {
        let effort = Some(effort).filter(|e| !e.is_empty());
        if effort == self.launch.effort {
            return;
        }
        self.launch.effort = effort;
        let name = crate::composer::effort_label(self.launch.effort.as_deref().unwrap_or_default());
        self.change(format!("Effort set to {name}."), cx);
    }

    /// After a model or effort change: the next prompt starts a session with it, resuming this
    /// conversation.
    pub(super) fn pick_permission(
        &mut self,
        permission: hyprspace_proto::Permission,
        cx: &mut Context<Self>,
    ) {
        self.perm_menu = None;
        if permission == self.launch.permission {
            cx.notify();
            return;
        }
        self.launch.permission = permission;
        let name = crate::composer::pickers::permission_label(permission);
        self.change(format!("Permission set to {name}."), cx);
    }

    fn change(&mut self, what: String, cx: &mut Context<Self>) {
        if self.model.running() {
            self.pending = true;
            self.model
                .note(format!("{what} It applies when this run ends."));
            cx.emit(TranscriptEvent::Launch(self.launch.clone()));
            cx.notify();
            return;
        }
        if self.open && !self.remote {
            self.client.send(Command::Configure {
                id: self.id,
                launch: self.launch.clone(),
            });
            self.model.note(what);
            cx.emit(TranscriptEvent::Launch(self.launch.clone()));
            cx.notify();
        } else {
            self.model
                .note(format!("{what} It applies from your next message."));
            cx.emit(TranscriptEvent::Launch(self.launch.clone()));
            cx.notify();
        }
    }

    pub(super) fn retry_at(&self) -> Option<usize> {
        if self.remote || self.loading || self.model.running() || self.resume_at.is_some() {
            return None;
        }
        let (ix, item) = self
            .model
            .items
            .iter()
            .enumerate()
            .rev()
            .find(|(_, i)| !matches!(i, Item::Changes { .. } | Item::Note(_)))?;
        let failed = matches!(
            item,
            Item::Error(_)
                | Item::Finished {
                    status: hyprspace_proto::RunStatus::Failed,
                    ..
                }
        );
        (failed && self.last_prompt().is_some()).then_some(ix)
    }

    fn last_prompt(&self) -> Option<Prompt> {
        self.model.items.iter().rev().find_map(|i| match i {
            Item::User {
                text,
                images,
                steer: false,
            } if !text.trim().is_empty() || !images.is_empty() => Some(Prompt {
                text: text.clone(),
                images: images.clone(),
            }),
            _ => None,
        })
    }

    pub(super) fn retry(&mut self, cx: &mut Context<Self>) {
        if let Some(prompt) = self.last_prompt() {
            self.offer(prompt, cx);
        }
    }

    pub(super) fn can_rewind(&self) -> bool {
        !self.remote && !self.loading && !self.model.running()
    }

    /// The message at `ix`, how many times its text was sent before it, and how many messages
    /// came before it.
    fn point(&self, ix: usize) -> Option<(String, Vec<PathBuf>, usize, usize)> {
        let Some(Item::User {
            text,
            images,
            steer: false,
        }) = self.model.items.get(ix)
        else {
            return None;
        };
        if !self.can_rewind() {
            return None;
        }
        let users: Vec<&str> = self.model.items[..ix]
            .iter()
            .filter_map(|i| match i {
                Item::User { text, .. } => Some(text.trim()),
                _ => None,
            })
            .collect();
        let nth = users.iter().filter(|t| **t == text.trim()).count();
        Some((text.clone(), images.clone(), nth, users.len()))
    }

    pub(super) fn fork(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some((text, images, nth, keep)) = self.point(ix) else {
            return;
        };
        let mut launch = self.launch.clone();
        launch.rewind = launch.resume.is_some().then(|| hyprspace_proto::Rewind {
            text: text.clone(),
            nth,
            fork: true,
        });
        cx.emit(TranscriptEvent::Fork {
            launch,
            keep,
            text,
            images,
        });
    }

    /// A new thread that keeps the first `keep` prompts and their replies, from the phone.
    pub fn fork_keeping(&mut self, keep: usize, prompt: &str, cx: &mut Context<Self>) {
        let users: Vec<(usize, &str)> = self
            .model
            .items
            .iter()
            .enumerate()
            .filter_map(|(ix, i)| match i {
                Item::User { text, .. } => Some((ix, text.trim())),
                _ => None,
            })
            .collect();
        let prompt = prompt.trim();
        let counted = keep
            .checked_sub(1)
            .and_then(|at| users.get(at))
            .is_some_and(|(_, text)| prompt.is_empty() || *text == prompt);
        let keep = if counted {
            keep
        } else {
            match users.iter().rposition(|(_, text)| *text == prompt) {
                Some(at) => at + 1,
                None => return,
            }
        };
        let next = users.get(keep).map(|(ix, _)| *ix);
        let Some(ix) = next else {
            return self.branch(cx);
        };
        let Some((text, _, nth, keep)) = self.point(ix) else {
            return;
        };
        let mut launch = self.launch.clone();
        launch.rewind = launch.resume.is_some().then_some(hyprspace_proto::Rewind {
            text,
            nth,
            fork: true,
        });
        cx.emit(TranscriptEvent::Fork {
            launch,
            keep,
            text: String::new(),
            images: Vec::new(),
        });
    }

    pub(super) fn rewind(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some((text, images, nth, keep)) = self.point(ix) else {
            return;
        };
        if self.open {
            self.client.send(Command::Close { id: self.id });
            self.open = false;
        }
        if self.launch.resume.is_some() {
            self.launch.rewind = Some(hyprspace_proto::Rewind {
                text: text.clone(),
                nth,
                fork: false,
            });
        }
        if !self.journal.is_empty() {
            self.client.send(Command::TrimJournal {
                journal: self.journal.clone(),
                keep,
            });
        }
        self.model.cut(ix);
        self.expanded.retain(|i| *i < ix);
        self.open_runs.retain(|i| *i < ix);
        self.queue.clear();
        self.queue_changed(cx);
        self.images = images;
        self.input.update(cx, |i, cx| i.set_text(&text, cx));
        cx.emit(TranscriptEvent::Launch(self.launch.clone()));
        cx.notify();
    }

    fn apply_pending(&mut self) {
        if std::mem::take(&mut self.pending) && self.open && !self.remote {
            self.client.send(Command::Configure {
                id: self.id,
                launch: self.launch.clone(),
            });
        }
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
        if let RunEvent::TaskOutput { task, output, .. } = event {
            if self.task_open.as_ref() == Some(&task) {
                self.task_log = Some(output);
                cx.notify();
            }
            return;
        }
        match &event {
            RunEvent::Failed { .. } if std::mem::take(&mut self.probing) => {
                cx.emit(TranscriptEvent::SwitchFailed)
            }
            RunEvent::Started { .. } => self.probing = false,
            _ => {}
        }
        if let RunEvent::Commands { names, info } = &event {
            crate::suggest::set_commands(self.launch.agent, names.clone(), cx);
            if !info.is_empty() {
                crate::suggest::set_info(
                    self.launch.agent,
                    self.launch.cwd.clone(),
                    info.clone(),
                    cx,
                );
            }
            cx.emit(TranscriptEvent::Commands(
                self.launch.agent,
                names.clone(),
                info.clone(),
            ));
        }
        if let RunEvent::Started { thread, cwd, .. } = &event {
            self.repicked = false;
            self.launch.rewind = None;
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
        // an approval that lands mid-keystroke must not be denied by the Enter already on its way
        if let RunEvent::Approval { request, .. } = &event
            && self.typed_at.is_some_and(|t| t.elapsed() < TYPING)
        {
            self.held.insert(request.clone());
            self.release_held(cx);
        }
        let done = matches!(
            event,
            RunEvent::Finished {
                status: hyprspace_proto::RunStatus::Done,
                ..
            }
        );
        let ended = matches!(event, RunEvent::Finished { .. });
        let n = self.model.items.len();
        let had = match self.model.items.last() {
            Some(Item::Text { source, .. }) => source.len(),
            _ => 0,
        };
        // a reveal still behind redraws on its beat, and the beat shows what arrives
        let quiet = matches!(event, RunEvent::Text { .. })
            && self
                .reveal
                .as_ref()
                .is_some_and(|r| r.ix + 1 == n && r.behind(had));
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
        self.settle(cx);
        if !quiet {
            cx.notify();
        }
        if ended {
            self.apply_pending();
        }
        if done {
            self.next_queued(cx);
        }
    }

    /// The engine could not start or reach the session.
    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.probing) {
            cx.emit(TranscriptEvent::SwitchFailed);
        }
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
            self.partial_at = None;
            self.fade = Default::default();
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
                Entry::Run { event } => {
                    let started = matches!(event, RunEvent::Started { .. });
                    self.model.apply(event);
                    if started {
                        self.model.restarted();
                    }
                }
            }
        }
        if !self.remote {
            self.cut = self.model.settle();
        }
        if let Some(note) = self.after.take() {
            self.model.note(note);
        }
        self.loading = false;
        self.read = self.model.items.len();
        self.unread = None;
        self.continue_updated(cx);
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
                let files: Vec<PathBuf> = paths.into_iter().filter(|p| p.is_file()).collect();
                v.attach(files, cx);
                v.keep_draft(cx);
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
        let thread = self.suggest.accept(Some(ix), &self.input, cx);
        self.took(thread, cx);
    }

    fn took(&mut self, thread: Option<u64>, cx: &mut Context<Self>) {
        match thread {
            Some(thread) => cx.emit(TranscriptEvent::Reference(thread)),
            None => self.run_if_own(cx),
        }
    }

    pub fn attach_files(&mut self, files: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.attach(files, cx);
        self.keep_draft(cx);
        cx.notify();
    }

    fn run_if_own(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        if let Some(commands::Slash::Own(own, _)) = commands::parse(&text, &[])
            && !commands::wants_text(own)
        {
            self.submit(cx);
        }
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
        let thread = self.suggest.accept(None, &self.input, cx);
        self.took(thread, cx);
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
        self.settle(cx);
        cx.notify();
    }

    fn settle(&mut self, cx: &mut Context<Self>) {
        if self.find.is_some() {
            self.refind(false, cx);
        }
        let hint = if !approval::open(self).is_empty() {
            APPROVAL_HINT
        } else if self.model.running() {
            RUNNING_HINT
        } else {
            IDLE_HINT
        };
        let (prompts, images): (Vec<String>, Vec<Vec<PathBuf>>) = self
            .model
            .items
            .iter()
            .filter_map(|i| match i {
                Item::User { text, images, .. } if !text.trim().is_empty() => {
                    Some((text.clone(), images.clone()))
                }
                _ => None,
            })
            .unzip();
        self.recall_images = images;
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
    }

    fn sync_rows(&mut self, rows: Vec<rows::Row>) {
        let same = self
            .rows
            .iter()
            .zip(&rows)
            .take_while(|(a, b)| a == b)
            .count();
        if same == 0 && !rows.is_empty() {
            // a row not drawn yet counts as no height, so a jump through it lands short
            self.list.reset_with_uniform_height(rows.len(), px(120.));
            self.follow.settle();
            if rows.iter().any(|r| matches!(r, rows::Row::Item { .. })) {
                self.appear = Some((Instant::now(), None));
            }
        } else if same < self.rows.len() || rows.len() > same {
            self.list.splice(same..self.rows.len(), rows.len() - same);
        }
        if std::mem::take(&mut self.remeasure) && same > 0 {
            self.list.remeasure_items(0..same);
        }
        self.rows = rows;
    }

    fn attach(&mut self, files: Vec<PathBuf>, cx: &mut Context<Self>) {
        let words: Vec<&str> = files.iter().map(|f| attach::word(f)).collect();
        attach::mark(&self.input, self.images.len() + self.saving + 1, &words, cx);
        self.images.extend(files);
    }

    /// Files dropped on the thread: images go in with the next message, anything else has its
    /// path typed in at the cursor, as a terminal does, and the box takes the keyboard.
    pub(crate) fn drop_paths(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (images, text) = attach::split_drop(paths.paths());
        self.attach(images, cx);
        self.keep_draft(cx);
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
            fast: crate::models::takes_fast(agent, &model).then_some(self.launch.fast),
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
            Choice::Fast(fast) => self.pick_fast(fast, cx),
        }
    }
}

actions!(transcript, [FindInThread, PrevPrompt, NextPrompt]);

pub fn bind_keys(cx: &mut gpui::App) {
    let c = Some("Thread");
    cx.bind_keys([
        KeyBinding::new("secondary-f", FindInThread, c),
        KeyBinding::new("alt-up", PrevPrompt, c),
        KeyBinding::new("alt-down", NextPrompt, c),
    ]);
}

const IDLE_HINT: &str = "Ask anything, @ for files, / for commands";
const CONTINUE: &str = "The usage limit has reset. Continue where you left off.";
const RESUME: &str = "The app closed before your last run finished. Continue where you left off.";

/// The prompt that picks a run back up after an update, naming the background work that stopped.
fn after_update(work: &[String]) -> String {
    const SHOWN: usize = 10;
    let mut text = String::from(
        "HyprSpace restarted to install an update while you were working. Continue where you left off.",
    );
    if !work.is_empty() {
        text.push_str(
            "\n\nThis background work stopped with it and will not report back, so start again what you still need:",
        );
        for label in work.iter().take(SHOWN) {
            text.push_str("\n- ");
            text.push_str(label);
        }
        if work.len() > SHOWN {
            text.push_str(&format!("\n- and {} more", work.len() - SHOWN));
        }
    }
    text
}
const RUNNING_HINT: &str = "Queue a message for when it finishes";
const TYPING: Duration = Duration::from_millis(1500);
const APPROVAL_HINT: &str = "Send a message to deny and redirect the agent";

impl Focusable for TranscriptView {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for TranscriptView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render::view(self, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_picked_up_after_an_update_hears_what_stopped() {
        assert!(!after_update(&[]).contains("background"));
        let work: Vec<String> = (1..=12).map(|n| format!("Dev server {n}")).collect();
        let text = after_update(&work);
        assert!(text.starts_with("HyprSpace restarted to install an update"));
        assert!(text.contains("\n- Dev server 1\n"));
        assert!(text.contains("- Dev server 10\n- and 2 more"));
        assert!(!text.contains("Dev server 11"));
    }
}
