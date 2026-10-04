// The read-only viewer: a file with its syntax colored and its line numbers, scrolled to the line
// a terminal pointed at, an image fitted to the pane, or one file's working tree diff. A space has one viewer pane; showing
// another file replaces what it shows. Editing stays in the user's editor, one click away in the
// pane header.

mod code;
mod diff;
pub(crate) mod lightbox;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, MouseButton, ObjectFit,
    Render, ScrollStrategy, SharedString, StyledImage, Task, UniformListScrollHandle, Window, div,
    img, prelude::*, px,
};
use hyprspace_proto::{Client, Command, FolderCommand, FolderEvent, Pane};

use crate::colors;
use code::Code;
use diff::Diff;

/// Files the viewer can't draw; they open in the system's app for them.
pub fn is_media(path: &Path) -> bool {
    const MEDIA: &[&str] = &[
        "ico", "pdf", "mp4", "mov", "webm", "mp3", "wav", "zip", "exe", "dll",
    ];
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
}

pub enum ViewerEvent {
    /// Clicked: the root makes this the space's focused pane.
    Focused,
}

enum Body {
    Loading,
    Failed(SharedString),
    Code(Code),
    Diff(Diff),
    /// An image, and its size once read off its header.
    Image(PathBuf, Option<(u32, u32)>),
}

pub struct Viewer {
    client: Client,
    focus: FocusHandle,
    pane: Option<Pane>,
    body: Body,
    scroll: UniformListScrollHandle,
    /// The diff's repo root, from the git status the engine sends with it.
    root: Option<PathBuf>,
    _work: Option<Task<()>>,
}

impl EventEmitter<ViewerEvent> for Viewer {}

impl Focusable for Viewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Viewer {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        Self {
            client,
            focus: cx.focus_handle(),
            pane: None,
            body: Body::Loading,
            scroll: UniformListScrollHandle::new(),
            root: None,
            _work: None,
        }
    }

    pub fn pane(&self) -> Option<&Pane> {
        self.pane.as_ref()
    }

    /// The repo root a diff's path is relative to, once known.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
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
            (Some(Pane::File { path: a, .. }), Pane::File { path: b, .. }, Body::Code(_)) if a == b
        );
        self.pane = Some(pane.clone());
        if same_file {
            // the text is here already; only the line moves
            if let (Pane::File { line, col, .. }, Body::Code(c)) = (&pane, &mut self.body) {
                c.target(*line, *col);
                self.jump();
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

    fn jump(&self) {
        if let Body::Code(c) = &self.body
            && let Some(line) = c.target_line()
        {
            self.scroll.scroll_to_item(line, ScrollStrategy::Center);
        }
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
                match text {
                    Ok(text) => {
                        let code = Code::new(text, line, col);
                        let (path, text) = (path.clone(), text.clone());
                        self.body = Body::Code(code);
                        self.jump();
                        // tree-sitter takes a moment on a big file; plain text shows meanwhile
                        let job = cx.background_spawn(async move {
                            hyprspace_syntax::highlight(&path, &text)
                        });
                        self._work = Some(cx.spawn(async move |this, cx| {
                            if let Some(spans) = job.await {
                                let _ = this.update(cx, |v, cx| {
                                    if let Body::Code(c) = &mut v.body {
                                        c.color(spans);
                                        cx.notify();
                                    }
                                });
                            }
                        }));
                    }
                    Err(e) => self.body = Body::Failed(e.clone().into()),
                }
                cx.notify();
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
            Body::Code(c) => c.render(self.scroll.clone(), cx),
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
        div()
            .id("viewer")
            .track_focus(&self.focus)
            .size_full()
            .bg(colors::bg())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _, window, cx| {
                    window.focus(&v.focus, cx);
                    cx.emit(ViewerEvent::Focused);
                }),
            )
            .child(body)
    }
}
