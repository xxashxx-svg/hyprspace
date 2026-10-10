// The viewer: a text file to read and edit (`edit`), with its syntax colored and its line
// numbers, at the line a terminal pointed at; an image fitted to the card; or one file's working
// tree diff. It shows in a card over the window (`crate::workbench`); showing another file
// replaces what it shows. The user's own editor stays one click away in the card's header.

mod code;
mod diff;
mod edit;
pub(crate) mod lightbox;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    MouseButton, ObjectFit, Render, SharedString, StyledImage, Subscription, Task,
    UniformListScrollHandle, Window, div, img, prelude::*, px,
};
use hyprspace_proto::{Client, Command, FolderCommand, FolderEvent, Pane};

use crate::colors;
use diff::Diff;
use edit::{Editor, EditorEvent};

pub enum ViewerEvent {
    /// A file's unsaved state flipped: the card's title shows it.
    Dirty,
    /// The editor is done with a close the user asked for.
    Close,
}

/// Files the viewer can't draw; they open in the system's app for them.
pub fn is_media(path: &Path) -> bool {
    const MEDIA: &[&str] = &[
        "ico", "pdf", "mp4", "mov", "webm", "mp3", "wav", "zip", "exe", "dll",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
}

enum Body {
    Loading,
    Failed(SharedString),
    Edit(Entity<Editor>),
    Diff(Diff),
    /// An image, and its size once read off its header.
    Image(PathBuf, Option<(u32, u32)>),
}

pub struct Viewer {
    client: Client,
    machine: Option<String>,
    focus: FocusHandle,
    pane: Option<Pane>,
    body: Body,
    scroll: UniformListScrollHandle,
    /// The diff's repo root, from the git status the engine sends with it.
    root: Option<PathBuf>,
    _work: Option<Task<()>>,
    /// The editor's events, passed on.
    _edit: Option<Subscription>,
}

impl Focusable for Viewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<ViewerEvent> for Viewer {}

impl Viewer {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        Self {
            client,
            machine: None,
            focus: cx.focus_handle(),
            pane: None,
            body: Body::Loading,
            scroll: UniformListScrollHandle::new(),
            root: None,
            _work: None,
            _edit: None,
        }
    }

    pub fn set_machine(&mut self, machine: Option<String>) {
        if self.machine != machine {
            self.client = self.client.via(machine.clone());
            self.machine = machine;
        }
    }

    pub fn machine(&self) -> Option<&str> {
        self.machine.as_deref()
    }

    /// The repo root a diff's path is relative to, once known.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// A file on screen has edits not saved yet.
    pub fn dirty(&self, cx: &App) -> bool {
        matches!(&self.body, Body::Edit(e) if e.read(cx).dirty())
    }

    /// The card's close. A file with unsaved edits asks first, and says `Close` when it's done.
    pub fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        match &self.body {
            Body::Edit(e) => e.update(cx, |e, cx| e.may_close(cx)),
            _ => true,
        }
    }

    /// The card closed: what it showed goes, so the next look reads the file fresh and a closed
    /// file stops following the disk. Edits the user let go of go with it.
    pub fn forget(&mut self) {
        self.pane = None;
        self.body = Body::Loading;
        self._edit = None;
        self._work = None;
    }

    /// Lines added and removed, for a diff on screen.
    pub fn counts(&self) -> Option<(usize, usize)> {
        match &self.body {
            Body::Diff(d) => Some(d.counts()),
            _ => None,
        }
    }

    pub fn show(&mut self, pane: Pane, cx: &mut Context<Self>) {
        let same_file = matches!(
            (&self.pane, &pane, &self.body),
            (Some(Pane::File { path: a, .. }), Pane::File { path: b, .. }, Body::Edit(..)) if a == b
        );
        self.pane = Some(pane.clone());
        if same_file {
            // the text is here already, edits and all; only the line moves
            if let (Pane::File { line, col, .. }, Body::Edit(e)) = (&pane, &self.body) {
                e.update(cx, |e, cx| e.target(*line, *col, cx));
            }
            cx.notify();
            return;
        }
        self.body = Body::Loading;
        self._work = None;
        self.scroll = UniformListScrollHandle::new();
        match pane {
            Pane::File { path, .. } if crate::attach::is_image(&path) => {
                self.body = Body::Image(path.clone(), None);
                let job = cx.background_spawn({
                    let path = path.clone();
                    async move { image::image_dimensions(&path).ok() }
                });
                self._work = Some(cx.spawn(async move |this, cx| {
                    let size = job.await;
                    let _ = this.update(cx, |v, cx| {
                        if let Body::Image(p, s) = &mut v.body
                            && *p == path
                        {
                            *s = size;
                            cx.notify();
                        }
                    });
                }));
            }
            Pane::File { path, .. } => self
                .client
                .send(Command::Folder(FolderCommand::ReadFile { path })),
            Pane::Diff { cwd, path } => {
                self.root = None;
                self.client.send(Command::Folder(FolderCommand::GitStatus {
                    cwd: cwd.clone(),
                }));
                self.client
                    .send(Command::Folder(FolderCommand::Diff { cwd, path }));
                self.poll_diff(cx);
            }
            Pane::Thread { .. } => {}
        }
        cx.notify();
    }

    /// The working tree moves while agents work, so a diff on screen rereads itself, as the
    /// Tauri app's did.
    fn poll_diff(&mut self, cx: &mut Context<Self>) {
        self._work = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                let alive = this.update(cx, |v, _| {
                    if let Some(Pane::Diff { cwd, path }) = &v.pane {
                        v.client.send(Command::Folder(FolderCommand::Diff {
                            cwd: cwd.clone(),
                            path: path.clone(),
                        }));
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    pub fn event(&mut self, e: &FolderEvent, cx: &mut Context<Self>) {
        match (e, self.pane.clone()) {
            (
                FolderEvent::File { path, text },
                Some(Pane::File {
                    path: want,
                    line,
                    col,
                }),
            ) if *path == want => {
                match (text, &self.body) {
                    // the file read again while open: the editor decides what to do with it
                    (Ok(text), Body::Edit(e)) => {
                        e.update(cx, |e, cx| e.disk(text, cx));
                        return;
                    }
                    // gone or unreadable while open: what is on screen stays, to save again
                    (Err(_), Body::Edit(..)) => return,
                    (Ok(text), _) => {
                        let (path, text) = (path.clone(), text.clone());
                        let (client, focus) = (self.client.clone(), self.focus.clone());
                        let editor = cx.new(|cx| Editor::new(path, text, client, focus, cx));
                        editor.update(cx, |e, cx| e.target(line, col, cx));
                        let sub = cx.subscribe(&editor, |_, _, e: &EditorEvent, cx| match e {
                            EditorEvent::Dirty => cx.emit(ViewerEvent::Dirty),
                            EditorEvent::Close => cx.emit(ViewerEvent::Close),
                        });
                        self.body = Body::Edit(editor);
                        self._edit = Some(sub);
                    }
                    (Err(e), _) => self.body = Body::Failed(e.clone().into()),
                }
                cx.notify();
            }
            (FolderEvent::Saved { path, result }, Some(Pane::File { path: want, .. }))
                if *path == want =>
            {
                if let Body::Edit(e) = &self.body {
                    e.update(cx, |e, cx| e.saved(result, cx));
                }
            }
            (FolderEvent::Diff { cwd, path, text }, Some(Pane::Diff { cwd: c, path: p }))
                if *cwd == c && *path == p =>
            {
                self.body = match text {
                    Ok(t) if t.trim().is_empty() => Body::Failed("No changes to show.".into()),
                    Ok(t) => {
                        // keep the scroll when a poll brings the same diff back
                        if let Body::Diff(d) = &self.body
                            && d.is(t)
                        {
                            return;
                        }
                        Body::Diff(Diff::new(t))
                    }
                    Err(e) => Body::Failed(e.clone().into()),
                };
                cx.notify();
            }
            (FolderEvent::Git { cwd, status }, Some(Pane::Diff { cwd: c, .. })) if *cwd == c => {
                self.root = status.root.clone();
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Render for Viewer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.body {
            Body::Loading => div().into_any_element(),
            Body::Failed(msg) => div()
                .p(px(14.))
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(msg.clone())
                .into_any_element(),
            Body::Edit(e) => e.clone().into_any_element(),
            Body::Diff(d) => d.render(self.scroll.clone(), cx),
            Body::Image(path, size) => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .p(px(20.))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(img(path.clone()).size_full().object_fit(ObjectFit::Contain)),
                )
                .children(size.map(|(w, h)| {
                    div()
                        .text_size(px(11.))
                        .text_color(colors::text3())
                        .child(format!("{w} \u{d7} {h}"))
                }))
                .into_any_element(),
        };
        // the editor holds the keyboard itself, with the same handle
        let editing = matches!(self.body, Body::Edit(..));
        div()
            .id("viewer")
            .when(!editing, |d| d.track_focus(&self.focus))
            .size_full()
            .bg(colors::bg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _, window, cx| window.focus(&v.focus, cx)),
            )
            .child(body)
    }
}
