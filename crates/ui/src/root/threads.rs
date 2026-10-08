// What the root does to spaces and threads: create, open, rename, archive, remove. Every change
// to the saved state ends in `save`.

use std::path::{Path, PathBuf};

use gpui::{AppContext, Context, Entity, FocusHandle, Focusable, PathPromptOptions, Window};
use hyprspace_proto::{Agent, Command, Prompt, SessionId, Space, Thread, ThreadKind};

use super::{Action, Rename, Root, Screen, Start, View};
use crate::composer::Target;
use crate::input::{InputEvent, TextInput};
use crate::terminal::{TerminalEvent, TerminalView};
use crate::time::now_ms;
use crate::transcript::{Status, TranscriptEvent, TranscriptView};

const RETRY_LIMIT: u64 = 30 * 60 * 1000;

/// Two paths name the same folder. Windows paths are case-blind.
fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();
        if cfg!(windows) {
            s.to_lowercase().replace('/', "\\")
        } else {
            s
        }
    };
    norm(a) == norm(b)
}

/// A prompt as keystrokes for an agent in a terminal: one line, so no newline sends it early,
/// with attached images as paths after it (the agents read images by path).
fn typed(prompt: &Prompt) -> String {
    let text = prompt.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let paths = prompt.images.iter().map(|p| {
        let p = p.display().to_string();
        if p.contains(' ') {
            format!("\"{p}\"")
        } else {
            p
        }
    });
    std::iter::once(text)
        .chain(paths)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `path` without a trailing separator, the way the folder's own tools name it. A drive's root
/// keeps its own.
fn trim_separator(path: PathBuf) -> PathBuf {
    let full = path.to_string_lossy();
    let trimmed = full.trim_end_matches(['/', '\\']);
    if trimmed.len() == full.len() || trimmed.is_empty() || trimmed.ends_with(':') {
        return path;
    }
    PathBuf::from(trimmed)
}

pub(crate) fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

impl Root {
    /// The space for `path`, made if the sidebar has none yet.
    pub(crate) fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) -> u64 {
        let path = trim_separator(path);
        if let Some(s) = self
            .state
            .spaces
            .iter_mut()
            .find(|s| s.cwd.as_deref().is_some_and(|c| same_folder(c, &path)))
        {
            s.archived = false;
            return s.id;
        }
        let id = self.state.take_id();
        self.state.spaces.push(Space {
            id,
            name: folder_name(&path),
            cwd: Some(path),
            ..Default::default()
        });
        self.save();
        cx.notify();
        id
    }

    /// New thread from the sidebar's top button: pick a folder in the in-app browser, then the
    /// composer for its space. It starts beside the space on screen, where projects tend to live.
    pub(crate) fn pick_thread_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_picker.is_some() {
            return;
        }
        let home = std::env::home_dir().unwrap_or_default();
        let near = self
            .current_space()
            .and_then(|id| self.state.space(id))
            .or_else(|| self.state.spaces.iter().find(|s| !s.archived))
            .and_then(|s| s.cwd.as_deref())
            .and_then(|c| c.parent())
            .filter(|p| p.is_dir())
            .map(std::path::Path::to_path_buf);
        let start = near.unwrap_or_else(|| home.clone());
        let back = window.focused(cx);
        let client = self.client.clone();
        let picker = cx.new(|cx| crate::folders::FolderPicker::new(client, start, home, back, cx));
        cx.subscribe_in(
            &picker,
            window,
            |r, _, e: &crate::folders::PickerEvent, window, cx| match e {
                crate::folders::PickerEvent::Open(path) => {
                    let path = path.clone();
                    r.close_folder_picker(window, cx);
                    let space = r.add_project(path, cx);
                    r.save();
                    r.compose(Some(space), window, cx);
                }
                crate::folders::PickerEvent::System => {
                    r.close_folder_picker(window, cx);
                    r.pick_folder_from_system(window, cx);
                }
                crate::folders::PickerEvent::Close => r.close_folder_picker(window, cx),
            },
        )
        .detach();
        let focus = picker.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.folder_picker = Some(picker);
        self.menu = None;
        cx.notify();
    }

    pub(crate) fn close_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.folder_picker.take()
            && let Some(back) = p.read(cx).back.clone()
        {
            window.focus(&back, cx);
        }
        cx.notify();
    }

    /// The system's own folder dialog, from the browser's footer.
    pub(crate) fn pick_folder_from_system(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |root, window, cx| {
                let space = root.add_project(path, cx);
                root.save();
                root.compose(Some(space), window, cx);
            });
        })
        .detach();
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.settings.tab();
        self.open_settings_at(tab, window, cx);
    }

    /// Settings open at `tab`. Focus moves to the screen once, here, so Esc closes it; taking
    /// focus on every render instead stole it from the inputs inside Settings and the palette.
    pub(crate) fn open_settings_at(
        &mut self,
        tab: crate::settings::Tab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen != Screen::Settings {
            self.back = self.screen;
            self.screen = Screen::Settings;
        }
        self.settings.set_tab(tab);
        self.menu = None;
        window.focus(&self.settings.focus_handle(), cx);
        cx.notify();
    }

    /// Settings' Back: the screen it was opened from, or the first space if that one is gone.
    pub(crate) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.back {
            Screen::Thread(id) if self.state.thread(id).is_some() => {
                self.open_thread(id, window, cx)
            }
            Screen::Compose(Some(id)) if self.state.space(id).is_some() => {
                self.compose(Some(id), window, cx)
            }
            _ => {
                let first = self.state.spaces.iter().find(|s| !s.archived).map(|s| s.id);
                self.compose(first, window, cx);
            }
        }
    }

    pub(crate) fn compose(
        &mut self,
        space: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::Compose(space);
        let target = space.and_then(|id| self.state.space(id)).map(|s| Target {
            space: s.id,
            name: s.name.clone(),
            cwd: s.cwd.clone(),
        });
        self.composer.update(cx, |c, cx| c.set_target(target, cx));
        let focus = self.composer.focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    fn session_id(&mut self, thread: u64) -> SessionId {
        let id = SessionId(self.next_session);
        self.next_session += 1;
        self.sessions.insert(id, thread);
        id
    }

    /// Makes the view for a thread. `first` goes out as the first prompt; `history` reads the
    /// thread's journal first, for a thread from an earlier run of the app.
    pub(crate) fn make_view(
        &mut self,
        thread: &Thread,
        first: Option<Prompt>,
        history: bool,
        cx: &mut Context<Self>,
    ) {
        let id = thread.id;
        let session = self.session_id(id);
        let client = self.client.clone();
        let view = match &thread.kind {
            ThreadKind::Structured { launch } => {
                let launch = launch.clone();
                let journal = thread.journal();
                let queue = thread.queue.clone();
                let resume_at = thread.resume_at;
                let catalog = self
                    .agents
                    .iter()
                    .find(|a| a.agent == launch.agent)
                    .map(|a| a.catalog.clone());
                let v = cx.new(|cx| {
                    let mut v =
                        TranscriptView::new(session, launch, journal, history, first, client, cx);
                    v.set_queue(queue);
                    v.set_resume(resume_at, cx);
                    if let Some(c) = catalog {
                        v.set_catalog(c, cx);
                    }
                    v
                });
                let sub = cx.subscribe(&v, move |root, _, e: &TranscriptEvent, cx| {
                    root.on_transcript(id, e, cx)
                });
                self._subs.push(sub);
                if let Some(status) = Some(v.read(cx).status()) {
                    self.set_status(id, status);
                }
                View::Structured(v)
            }
            ThreadKind::Terminal { cwd, run } => {
                let (cwd, run) = (cwd.clone(), run.clone());
                let prompt = first.as_ref().map(typed);
                let v = cx.new(|cx| TerminalView::new(session, client, cwd, run, prompt, cx));
                let sub = cx.subscribe(&v, |root, _, e: &TerminalEvent, cx| match e {
                    // an image opens over the window, anything else in the viewer
                    TerminalEvent::OpenFile {
                        path, line: None, ..
                    } if crate::attach::is_image(path) => root.show_image(path.clone(), cx),
                    TerminalEvent::OpenFile { path, line, col } => {
                        root.open_file(path.clone(), *line, *col, cx)
                    }
                });
                self._subs.push(sub);
                View::Terminal(v)
            }
        };
        self.views.insert(id, view);
    }

    /// The agent running in a terminal thread changed under us: started by hand, swapped for
    /// another, or quit back to the shell. The thread takes on the new agent, so its row shows
    /// that agent's mark and model and a restart brings that agent back. When the agent quits
    /// the thread keeps the last one, and whatever it was doing is over.
    pub(crate) fn terminal_agent(
        &mut self,
        thread: u64,
        agent: Option<Agent>,
        model: Option<String>,
    ) {
        let Some(agent) = agent else {
            self.status.remove(&thread);
            self.activity.remove(&thread);
            self.turns.remove(&thread);
            self.session_ended(thread);
            return;
        };
        let Some(Thread {
            kind: ThreadKind::Terminal { cwd, run },
            ..
        }) = self.state.thread_mut(thread)
        else {
            return;
        };
        match run {
            Some(r) if r.agent == agent => {
                if model.is_none() || r.model == model {
                    return;
                }
                r.model = model;
            }
            _ => {
                let mut launch = hyprspace_proto::Launch::new(agent, cwd.clone());
                launch.model = model;
                *run = Some(launch);
                self.status.remove(&thread);
                self.activity.remove(&thread);
                self.turns.remove(&thread);
            }
        }
        self.save();
    }

    pub(crate) fn set_status(&mut self, thread: u64, status: Status) {
        if status == Status::Done && self.screen != Screen::Thread(thread) {
            self.unseen.insert(thread);
        } else {
            self.unseen.remove(&thread);
        }
        let before = self.status.insert(thread, status);
        // a turn starting from rest is a message sent to it, which brings a shelved thread back
        if status == Status::Working && !matches!(before, Some(Status::Working | Status::Waiting)) {
            self.bring_back(thread);
        }
        // a real change, not the status a view reports when it is made
        if before.is_some_and(|b| b != status) || (before.is_none() && status == Status::Working) {
            self.touched(thread, status);
        }
    }

    fn on_transcript(&mut self, thread: u64, e: &TranscriptEvent, cx: &mut Context<Self>) {
        match e {
            TranscriptEvent::Status(s) => {
                self.set_status(thread, *s);
                self.tick(cx);
            }
            TranscriptEvent::Started { thread: t, cwd } => {
                if let Some(Thread {
                    kind: ThreadKind::Structured { launch },
                    ..
                }) = self.state.thread_mut(thread)
                {
                    launch.resume = Some(t.clone());
                    launch.cwd = cwd.clone();
                }
                self.save();
            }
            TranscriptEvent::Limited(resets) => {
                let now = now_ms();
                let agent = self
                    .state
                    .thread(thread)
                    .and_then(|(_, t)| t.agent().map(|l| l.agent));
                let meter = agent.and_then(|a| {
                    self.limits
                        .read(cx)
                        .readings
                        .picture(now as i64)
                        .spent_until(a, now as i64)
                });
                let at = resets
                    .or(meter.map(|m| m as u64))
                    .filter(|at| *at > now)
                    .unwrap_or(now + RETRY_LIMIT)
                    + 60_000;
                if let Some(t) = self.state.thread_mut(thread) {
                    t.resume_at = Some(at);
                }
                if let Some(View::Structured(v)) = self.views.get(&thread) {
                    v.update(cx, |v, cx| v.set_resume(Some(at), cx));
                }
                self.save();
            }
            TranscriptEvent::ResumeAt(at) => {
                if let Some(t) = self.state.thread_mut(thread) {
                    t.resume_at = *at;
                }
                self.save();
            }
            TranscriptEvent::Queue(queue) => {
                if let Some(t) = self.state.thread_mut(thread) {
                    t.queue = queue.clone();
                }
                self.save();
            }
            TranscriptEvent::Launch(l) => {
                if let Some(Thread {
                    kind: ThreadKind::Structured { launch },
                    ..
                }) = self.state.thread_mut(thread)
                {
                    launch.model = l.model.clone();
                    launch.effort = l.effort.clone();
                }
                self.save();
            }
        }
        cx.notify();
    }

    /// Opens a file a terminal pointed at, in the viewer card at its line. The event
    /// comes without the window, so the workbench picks it up on its next draw.
    pub(crate) fn open_file(
        &mut self,
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        self.work.pending = Some((path, line, col));
        cx.notify();
    }

    /// Makes a thread and starts it. `show` puts it on screen; a thread started from the phone
    /// starts without taking the desktop away from what it shows.
    pub(crate) fn start_thread(
        &mut self,
        space: u64,
        start: Start,
        show: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let Start {
            mut launch,
            prompt,
            title,
            terminal,
        } = start;
        let resumed = launch.resume.is_some() && !terminal;
        let kind = if terminal {
            // a Claude thread claims its conversation id up front, so it can resume it later
            if launch.agent == Agent::Claude && launch.resume.is_none() {
                launch.resume = Some(uuid::Uuid::new_v4().to_string());
            }
            ThreadKind::Terminal {
                cwd: launch.cwd.clone(),
                run: Some(launch),
            }
        } else {
            ThreadKind::Structured { launch }
        };
        let thread = Thread {
            id: self.state.take_id(),
            title,
            kind,
            created: now_ms(),
            touched: now_ms(),
            ..Thread::default()
        };
        let s = self.state.space_mut(space)?;
        s.folded = false;
        s.threads.insert(0, thread.clone());
        self.make_view(&thread, prompt, false, cx);
        if resumed && let Some(View::Structured(v)) = self.views.get(&thread.id) {
            v.update(cx, |v, cx| {
                v.note(
                    "Resumed an earlier conversation. Its messages before this point stay in the agent's own history.",
                    cx,
                )
            });
        }
        if show {
            self.open_thread(thread.id, window, cx);
        } else {
            self.save();
            cx.notify();
        }
        Some(thread.id)
    }

    /// A plain shell in the space's folder, on screen when `show`.
    pub(crate) fn new_terminal(
        &mut self,
        space: u64,
        show: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let cwd = self
            .state
            .space(space)
            .and_then(|s| s.cwd.clone())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let thread = Thread {
            id: self.state.take_id(),
            title: "Terminal".into(),
            kind: ThreadKind::Terminal { cwd, run: None },
            created: now_ms(),
            touched: now_ms(),
            ..Thread::default()
        };
        let s = self.state.space_mut(space)?;
        s.folded = false;
        s.threads.insert(0, thread.clone());
        self.make_view(&thread, None, false, cx);
        if show {
            self.open_thread(thread.id, window, cx);
        } else {
            self.save();
            cx.notify();
        }
        Some(thread.id)
    }

    /// Shows an image over the whole window, zoomable and movable, until it is closed.
    pub(crate) fn show_image(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let lightbox = cx.new(|cx| crate::viewer::lightbox::Lightbox::new(path, cx));
        let sub = cx.subscribe(&lightbox, |r, _, _: &crate::viewer::lightbox::Close, cx| {
            r.lightbox = None;
            cx.notify();
        });
        self.lightbox = Some((lightbox, sub));
        cx.notify();
    }

    pub(crate) fn open_thread(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.reveal = Some(id);
        let Some((_, thread)) = self.state.thread(id) else {
            return;
        };
        // a settled or snoozed thread opens where it is, on its shelf; a message sent to it brings
        // it back (bring_back)
        self.peek = (!thread.active()).then_some(id);
        let thread = thread.clone();
        if !self.views.contains_key(&id) {
            self.make_view(&thread, None, true, cx);
        }
        self.screen = Screen::Thread(id);
        self.unseen.remove(&id);
        self.state.active = Some(id);
        self.save();
        let focus = match &self.views[&id] {
            View::Structured(v) => v.focus_handle(cx),
            View::Terminal(v) => v.focus_handle(cx),
        };
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Kills a thread's session and forgets its view.
    pub(crate) fn drop_view(&mut self, thread: u64) {
        if self.views.remove(&thread).is_some() {
            let ids: Vec<SessionId> = self
                .sessions
                .iter()
                .filter(|(_, t)| **t == thread)
                .map(|(s, _)| *s)
                .collect();
            for id in ids {
                self.sessions.remove(&id);
                self.client.send(Command::Close { id });
            }
        }
        self.status.remove(&thread);
        self.unseen.remove(&thread);
    }

    /// After the thread or space on screen went away.
    pub(crate) fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let still_there = match self.screen {
            Screen::Thread(id) => self
                .state
                .thread(id)
                .is_some_and(|(s, t)| (t.active() || self.peek == Some(id)) && !s.archived),
            Screen::Compose(Some(id)) => self.state.space(id).is_some_and(|s| !s.archived),
            Screen::Compose(None) | Screen::Settings => true,
        };
        if !still_there {
            // a settled or snoozed thread's own space stays, with its composer; a space that went
            // away gives way to the first one left
            let own = match self.screen {
                Screen::Thread(gone) => self
                    .state
                    .thread(gone)
                    .filter(|(s, _)| !s.archived)
                    .map(|(s, _)| s.id),
                _ => None,
            };
            let first = self.state.spaces.iter().find(|s| !s.archived).map(|s| s.id);
            self.compose(own.or(first), window, cx);
        }
    }

    pub(crate) fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        // a menu row that opens another menu opens it where this one was
        self.menu = None;
        match action {
            Action::NewThread(space) => self.compose(Some(space), window, cx),
            Action::NewTerminal(space) => {
                self.new_terminal(space, true, window, cx);
            }
            Action::Rename(target) => self.start_rename(target, window, cx),
            Action::SettleSpace(id) => self.settle_space(id, window, cx),
            Action::Settle(id, on) => self.settle(id, on, window, cx),
            Action::Wake(id) => self.snooze(id, None, window, cx),
            Action::RemoveThread(id) => {
                self.drop_view(id);
                for s in &mut self.state.spaces {
                    s.threads.retain(|t| t.id != id);
                }
                self.leave(window, cx);
            }
            Action::OpenIn(opener, space) => {
                if let Some(cwd) = self.state.space(space).and_then(|s| s.cwd.clone()) {
                    self.open_in(opener, &cwd, cx);
                }
            }
            Action::SnoozeUntil(id, at) => {
                self.snooze(id, Some(hyprspace_proto::Snooze::Time { at }), window, cx)
            }
            Action::SnoozeDone(id) => {
                self.snooze(id, Some(hyprspace_proto::Snooze::Done), window, cx)
            }
            Action::Filter(space) => {
                if let Some(name) = self.state.space(space).map(|s| s.name.clone()) {
                    self.search.update(cx, |i, cx| i.set_text(&name, cx));
                }
            }
            Action::CopyTitle(thread) => {
                if let Some((_, t)) = self.state.thread(thread) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(t.title.clone()));
                }
            }
            Action::CopyPath(thread) => {
                if let Some((_, t)) = self.state.thread(thread) {
                    let cwd = match &t.kind {
                        ThreadKind::Terminal { cwd, .. } => cwd.clone(),
                        ThreadKind::Structured { launch } => launch.cwd.clone(),
                    };
                    let text = cwd.display().to_string();
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                }
            }
            Action::CopyConversation(thread) => {
                if let Some(id) = self.conversation(thread) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(id));
                }
            }
        }
        self.save();
        cx.notify();
    }

    /// The conversation a thread's agent is on, which `--resume` takes.
    fn conversation(&self, thread: u64) -> Option<String> {
        self.state
            .thread(thread)
            .and_then(|(_, t)| t.agent()?.resume.clone())
    }

    /// Renames a thread in place, after T3 Code's: Enter saves, Esc cancels, and clicking
    /// anywhere else saves too. Enter and Esc give the keyboard back to what had it; a click
    /// away leaves it where the click put it.
    fn start_rename(&mut self, target: Rename, window: &mut Window, cx: &mut Context<Self>) {
        let Rename::Thread(thread) = target;
        let current = self
            .state
            .thread(thread)
            .map(|(_, t)| t.title.clone())
            .unwrap_or_default();
        let input: Entity<TextInput> = cx.new(|cx| {
            let mut i = TextInput::new("Name", false, cx);
            i.set_text(current, cx);
            i.select_all_text(cx);
            i
        });
        let back = window.focused(cx);
        let keys = cx.subscribe_in(
            &input,
            window,
            move |root, input, e: &InputEvent, window, cx| match e {
                InputEvent::Submit => root.end_rename(input, true, back.clone(), window, cx),
                InputEvent::Cancel => root.end_rename(input, false, back.clone(), window, cx),
                _ => {}
            },
        );
        let focus = input.focus_handle(cx);
        let this = input.clone();
        let blur = cx.on_focus_out(&focus, window, move |root, _, window, cx| {
            root.end_rename(&this, true, None, window, cx)
        });
        window.focus(&focus, cx);
        self.rename = Some((target, input, [keys, blur]));
        cx.notify();
    }

    /// Ends the rename `input` belongs to, saving the name when `save` and it isn't blank, and
    /// gives the keyboard to `back`. A rename that already gave way to another is left alone.
    fn end_rename(
        &mut self,
        input: &Entity<TextInput>,
        save: bool,
        back: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let thread = match &self.rename {
            Some((Rename::Thread(thread), current, _)) if current == input => *thread,
            _ => return,
        };
        let name = input.read(cx).text().trim().to_string();
        if save
            && !name.is_empty()
            && let Some(t) = self.state.thread_mut(thread)
            && t.title != name
        {
            t.title = name;
            self.save();
        }
        self.rename = None;
        // after the key that ended it has finished with the box
        if let Some(back) = back {
            window.defer(cx, move |window, cx| window.focus(&back, cx));
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_match_the_way_the_os_does() {
        assert!(same_folder(Path::new("/w/app/"), Path::new("/w/app")));
        assert!(!same_folder(Path::new("/w/app"), Path::new("/w/apps")));
        if cfg!(windows) {
            assert!(same_folder(Path::new(r"C:\Main\X"), Path::new("c:/main/x")));
        }
        assert_eq!(folder_name(Path::new("/w/app")), "app");
    }

    #[test]
    fn a_terminal_prompt_is_one_line_with_its_images() {
        let p = Prompt {
            text: "fix  this\nplease".into(),
            images: vec![PathBuf::from("/t/a b.png"), PathBuf::from("/t/c.png")],
        };
        assert_eq!(typed(&p), "fix this please \"/t/a b.png\" /t/c.png");
    }
}
