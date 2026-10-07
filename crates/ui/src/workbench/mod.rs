// The main area: one thread at a time, with the bar above it and the dock beside it. A file or a
// diff opens over it in the viewer card. This module holds what the root does to them; `frame.rs`
// draws the thread and the dock, `header.rs` a terminal thread's title bar, `bar.rs` the row
// above with the Open button, `card.rs` the viewer card. Reasons for the shape:
// docs/internals/threads.md.

mod bar;
mod card;
mod frame;
mod header;

pub use header::{PaneDrag, short};

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, KeyBinding, Pixels, Point, SharedString,
    Task, Window, actions,
};
use hyprspace_proto::{Command, FolderCommand, FolderEvent, Opener, Pane};

use crate::dock::{Dock, DockEvent};
use crate::root::{Root, Screen};
use crate::viewer::{Viewer, ViewerEvent};

actions!(hyprspace, [ToggleDock, ToggleSidebar]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("secondary-shift-g", ToggleDock, None),
        KeyBinding::new("secondary-shift-b", ToggleSidebar, None),
    ]);
}

/// What the viewer card shows, the space it was opened in, and what had the keyboard before it,
/// which gets it back when the card closes.
pub(crate) struct Viewing {
    pub(crate) space: u64,
    pub(crate) pane: Pane,
    back: Option<FocusHandle>,
}

/// The root's state for the main area, the dock and the viewer. One field on `Root` so the root
/// stays small.
pub struct Work {
    pub(crate) dock: Entity<Dock>,
    /// The viewer, made the first time a file or a diff is shown.
    pub(crate) viewer: Option<Entity<Viewer>>,
    /// The viewer card, while it is open.
    pub(crate) viewing: Option<Viewing>,
    /// Apps that can open a folder here, editors first. Explorer or Finder is always there.
    pub(crate) openers: Vec<Opener>,
    /// The Open button's menu, where it was opened.
    pub(crate) popup: Option<Point<Pixels>>,
    /// A launch that failed, shown in the bar for a few seconds.
    pub(crate) notice: Option<SharedString>,
    notice_task: Option<Task<()>>,
    /// A path a terminal asked to open, waiting for the next draw to have the window.
    pub(crate) pending: Option<(PathBuf, Option<u32>, Option<u32>)>,
    /// What the dock was last told, so it is only told again when that changes.
    dock_sync: Option<(Option<PathBuf>, bool, Option<PathBuf>)>,
    /// The dock's slide (`crate::slide`).
    pub(crate) dock_flips: crate::slide::Flips,
}

impl Work {
    pub fn new(
        client: hyprspace_proto::Client,
        window: &mut Window,
        cx: &mut Context<Root>,
    ) -> Self {
        client.send(Command::Folder(FolderCommand::Openers));
        let dock = cx.new(|cx| Dock::new(client, cx));
        cx.subscribe_in(&dock, window, |root, _, e: &DockEvent, window, cx| {
            root.on_dock(e, window, cx)
        })
        .detach();
        Self {
            dock,
            viewer: None,
            viewing: None,
            openers: vec![Opener::Files],
            popup: None,
            notice: None,
            notice_task: None,
            pending: None,
            dock_sync: None,
            dock_flips: Default::default(),
        }
    }
}

impl Root {
    /// The space on screen: the thread's, or the composer's.
    pub(crate) fn current_space(&self) -> Option<u64> {
        match self.screen {
            Screen::Thread(id) => self.state.thread(id).map(|(s, _)| s.id),
            Screen::Compose(space) => space,
            Screen::Settings => None,
        }
    }

    /// Opens a file or a diff in the viewer card, over whatever is on screen.
    pub(crate) fn show_viewer(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.current_space() else {
            return;
        };
        let viewer = self.viewer(window, cx);
        // another file over unsaved edits: the edits come first
        let other = self.work.viewing.as_ref().is_some_and(|v| v.pane != pane);
        if other && viewer.read(cx).dirty(cx) {
            viewer.update(cx, |v, cx| v.may_close(cx));
            return;
        }
        viewer.update(cx, |v, cx| v.show(pane.clone(), cx));
        // a card already open hands on the focus it was keeping
        let back = match self.work.viewing.take() {
            Some(open) => open.back,
            None => window.focused(cx),
        };
        self.work.viewing = Some(Viewing { space, pane, back });
        let focus = viewer.focus_handle(cx);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Closes the viewer card. A file with unsaved edits asks first, and closes the card itself
    /// once the user has answered (`ViewerEvent::Close`).
    pub(crate) fn close_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(v) = self.work.viewer.clone()
            && self.work.viewing.is_some()
            && !v.update(cx, |v, cx| v.may_close(cx))
        {
            return;
        }
        self.close_viewer_now(window, cx);
    }

    /// Closes the viewer card and gives the keyboard back to what had it.
    fn close_viewer_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(v) = &self.work.viewer {
            v.update(cx, |v, _| v.forget());
        }
        // after the click that closed it, which would otherwise keep the focus where it landed
        if let Some(back) = self.work.viewing.take().and_then(|v| v.back) {
            window.defer(cx, move |window, cx| window.focus(&back, cx));
        }
        cx.notify();
    }

    /// The viewer, made on first use.
    pub(crate) fn viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Viewer> {
        if let Some(v) = &self.work.viewer {
            return v.clone();
        }
        let client = self.client.clone();
        let v = cx.new(|cx| Viewer::new(client, cx));
        cx.subscribe_in(&v, window, |root, _, e: &ViewerEvent, window, cx| match e {
            ViewerEvent::Close => root.close_viewer_now(window, cx),
            // the card's title marks unsaved edits
            ViewerEvent::Dirty => cx.notify(),
        })
        .detach();
        self.work.viewer = Some(v.clone());
        v
    }

    /// A path ctrl+clicked in a terminal, or a file picked in the dock: the viewer shows it,
    /// at its line. Media the viewer can't draw goes to the system's app for it.
    pub(crate) fn open_path(
        &mut self,
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::viewer::is_media(&path) {
            self.client.send(Command::OpenFile { path, line, col });
            return;
        }
        self.show_viewer(Pane::File { path, line, col }, window, cx);
    }

    pub(crate) fn toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.sidebar_hidden = !self.state.sidebar_hidden;
        self.save();
        cx.notify();
    }

    pub(crate) fn toggle_dock(&mut self, _: &ToggleDock, _: &mut Window, cx: &mut Context<Self>) {
        self.state.dock.open = !self.state.dock.open;
        self.save();
        cx.notify();
    }

    fn on_dock(&mut self, e: &DockEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e {
            DockEvent::Tab(tab) => {
                self.state.dock.tab = *tab;
                self.save();
            }
            DockEvent::Hide => {
                self.state.dock.open = false;
                self.save();
            }
            DockEvent::Open(Pane::File { path, line, col }) => {
                self.open_path(path.clone(), *line, *col, window, cx)
            }
            DockEvent::Open(pane) => self.show_viewer(pane.clone(), window, cx),
        }
        cx.notify();
    }

    /// The folder the dock follows: the thread on screen's, else the space's.
    fn dock_folder(&self, space: u64) -> Option<PathBuf> {
        let s = self.state.space(space)?;
        let thread = match self.screen {
            Screen::Thread(id) => s.threads.iter().find(|t| t.id == id).map(|t| t.cwd()),
            _ => None,
        };
        thread
            .filter(|p| !p.as_os_str().is_empty())
            .cloned()
            .or_else(|| s.cwd.clone())
    }

    /// Tells the dock what to follow, when that changed.
    fn sync_dock(&mut self, space: u64, cx: &mut Context<Self>) {
        let folder = self.dock_folder(space);
        let viewing = self.work.viewing.as_ref().and_then(|v| match &v.pane {
            Pane::File { path, .. } => Some(path.clone()),
            _ => None,
        });
        let want = (folder, self.state.dock.open, viewing);
        if self.work.dock_sync.as_ref() == Some(&want) {
            return;
        }
        self.work.dock_sync = Some(want.clone());
        let tab = self.state.dock.tab;
        let (folder, open, viewing) = want;
        self.work
            .dock
            .update(cx, |d, cx| d.sync(folder, open, tab, viewing, cx));
    }

    /// Answers to folder requests, for the dock, the viewer and the Open button.
    pub(crate) fn folder_event(&mut self, e: FolderEvent, cx: &mut Context<Self>) {
        if let (FolderEvent::Dir { path, entries }, Some(picker)) = (&e, &self.folder_picker) {
            picker.update(cx, |p, cx| p.listed(path, entries, cx));
        }
        if let FolderEvent::Git { cwd, status } = &e {
            self.git.insert(cwd.clone(), status.clone());
        }
        match &e {
            FolderEvent::Openers { openers } => {
                self.work.openers = openers.clone();
                // the saved pick may be an editor that is gone now
                if !openers.contains(&self.state.open_with)
                    && let Some(first) = openers.first()
                {
                    self.state.open_with = *first;
                }
            }
            FolderEvent::OpenFailed { message } => self.notify(message.clone(), cx),
            FolderEvent::File { .. }
            | FolderEvent::Saved { .. }
            | FolderEvent::Diff { .. }
            | FolderEvent::Git { .. } => {
                if let Some(v) = &self.work.viewer {
                    v.update(cx, |v, cx| v.event(&e, cx));
                }
            }
            _ => {}
        }
        if matches!(
            e,
            FolderEvent::Dir { .. } | FolderEvent::Git { .. } | FolderEvent::GitDone { .. }
        ) {
            self.work.dock.update(cx, |d, cx| d.event(&e, cx));
        }
        cx.notify();
    }

    /// A line in the bar that fades after a few seconds.
    fn notify(&mut self, message: String, cx: &mut Context<Self>) {
        self.work.notice = Some(message.into());
        self.work.notice_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let _ = this.update(cx, |r, cx| {
                r.work.notice = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Opens the space's folder with `opener`.
    pub(crate) fn open_in(&mut self, opener: Opener, folder: &Path, cx: &mut Context<Self>) {
        self.client.send(Command::Folder(FolderCommand::OpenIn {
            opener,
            path: folder.to_path_buf(),
        }));
        self.work.popup = None;
        cx.notify();
    }

    pub(crate) fn pick_opener(&mut self, opener: Opener, cx: &mut Context<Self>) {
        self.state.open_with = opener;
        self.work.popup = None;
        self.save();
        cx.notify();
    }
}
