// Several things on screen at once. A space tiles its panes (threads, plus at most one file or
// diff viewer) in a layout the user picks, with the dock beside them. This module holds what the
// root does to a space's grid; `grid.rs` draws it, `header.rs` each pane's title bar, `bar.rs`
// the row above with the layout picker and the Open button. Reasons for the shape:
// docs/adr/0007-panes-dock-and-viewer.md.

mod bar;
mod grid;
mod header;
pub mod layout;

pub use header::short;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, Focusable, KeyBinding, Pixels, Point, SharedString, Task, Window,
    actions,
};
use hyprspace_proto::{Command, FolderCommand, FolderEvent, Opener, Pane};

use crate::dock::{Dock, DockEvent};
use crate::root::{Root, Screen, View};
use crate::viewer::{Viewer, ViewerEvent};

actions!(hyprspace, [ToggleDock]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([KeyBinding::new("secondary-shift-g", ToggleDock, None)]);
}

/// Which menu hangs off the bar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    Layout,
    Open,
}

/// The root's state for panes, the dock and the viewers. One field on `Root` so the root stays
/// small.
pub struct Work {
    pub(crate) dock: Entity<Dock>,
    /// One viewer per space, made when the space first shows a file or a diff.
    pub(crate) viewers: HashMap<u64, Entity<Viewer>>,
    /// Apps that can open a folder here, editors first. Explorer or Finder is always there.
    pub(crate) openers: Vec<Opener>,
    pub(crate) popup: Option<(Point<Pixels>, Popup)>,
    /// A launch that failed, shown in the bar for a few seconds.
    pub(crate) notice: Option<SharedString>,
    notice_task: Option<Task<()>>,
    /// A path a terminal asked to open, waiting for the next draw to have the window.
    pub(crate) pending: Option<(PathBuf, Option<u32>, Option<u32>)>,
    /// What the dock was last told, so it is only told again when that changes.
    dock_sync: Option<(Option<PathBuf>, bool, Option<PathBuf>)>,
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
            viewers: HashMap::new(),
            openers: vec![Opener::Files],
            popup: None,
            notice: None,
            notice_task: None,
            pending: None,
            dock_sync: None,
        }
    }
}

impl Root {
    /// The space on screen: the focused thread's, or the composer's.
    pub(crate) fn current_space(&self) -> Option<u64> {
        match self.screen {
            Screen::Thread(id) => self.state.thread(id).map(|(s, _)| s.id),
            Screen::Compose(space) => space,
        }
    }

    /// The panes of `space` that still have something to show: a removed or archived thread
    /// drops out without anyone having to tidy the saved grid.
    pub(crate) fn live_panes(&self, space: u64) -> Vec<Pane> {
        let Some(s) = self.state.space(space) else {
            return vec![];
        };
        s.grid
            .panes
            .iter()
            .filter(|p| match p {
                Pane::Thread { id } => s.threads.iter().any(|t| t.id == *id && !t.archived),
                _ => true,
            })
            .cloned()
            .collect()
    }

    /// Another thread pane to show when `gone` was removed or archived from the screen.
    pub(crate) fn next_pane_after(&self, gone: u64) -> Option<u64> {
        let gone = Pane::Thread { id: gone };
        let s = self.state.spaces.iter().find(|s| s.grid.has(&gone))?;
        let live = self.live_panes(s.id);
        live.iter().find_map(|p| p.thread())
    }

    /// Puts a thread on screen in its space. Already there: nothing moves. Otherwise it takes
    /// the focused pane's place, or joins the others when `add` (a new thread, or a ctrl+click in
    /// the sidebar). The thread it replaces keeps running; its row is still in the sidebar.
    pub(crate) fn place_thread(&mut self, id: u64, add: bool) {
        let live: Vec<Pane> = match self.state.thread(id) {
            Some((s, _)) => self.live_panes(s.id),
            None => return,
        };
        let Some(space) = self.state.thread(id).map(|(s, _)| s.id) else {
            return;
        };
        let pane = Pane::Thread { id };
        let Some(grid) = self.state.space_mut(space).map(|s| &mut s.grid) else {
            return;
        };
        grid.panes = live;
        if !grid.has(&pane) {
            let old = grid
                .focus
                .clone()
                .filter(|f| grid.has(f) && f.thread().is_some())
                .or_else(|| grid.panes.iter().find(|p| p.thread().is_some()).cloned());
            match old {
                Some(old) if !add => grid.replace(&old, pane.clone()),
                _ => {
                    grid.panes.push(pane.clone());
                    grid.maximized = None;
                }
            }
        }
        grid.focus = Some(pane);
    }

    /// Focuses a pane. `keys` moves keyboard focus into it too, for a pane opened from
    /// elsewhere; a click does that itself.
    pub(crate) fn focus_pane(
        &mut self,
        space: u64,
        pane: Pane,
        keys: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(s) = self.state.space_mut(space) {
            if s.grid.focus.as_ref() == Some(&pane) && !keys {
                return;
            }
            s.grid.focus = Some(pane.clone());
        }
        match &pane {
            Pane::Thread { id } => {
                self.screen = Screen::Thread(*id);
                self.state.active = Some(*id);
                if keys && let Some(view) = self.views.get(id) {
                    let focus = match view {
                        View::Structured(v) => v.focus_handle(cx),
                        View::Terminal(v) => v.focus_handle(cx),
                    };
                    window.focus(&focus, cx);
                }
            }
            _ => {
                if keys && let Some(v) = self.work.viewers.get(&space) {
                    let focus = v.focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
        }
        self.save();
        cx.notify();
    }

    /// Takes a pane off screen. A thread keeps running and stays in the sidebar. When the last
    /// thread goes, the space shows its composer.
    pub(crate) fn close_pane(
        &mut self,
        space: u64,
        pane: Pane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let live = self.live_panes(space);
        let Some(s) = self.state.space_mut(space) else {
            return;
        };
        s.grid.panes = live;
        s.grid.remove(&pane);
        let next = s
            .grid
            .focus
            .clone()
            .and_then(|f| f.thread())
            .or_else(|| s.grid.panes.iter().find_map(|p| p.thread()));
        match next {
            Some(id) => {
                s.grid.focus = Some(Pane::Thread { id });
                self.focus_pane(space, Pane::Thread { id }, true, window, cx);
            }
            None => {
                // a viewer alone is not a space's working view
                s.grid.panes.clear();
                s.grid.focus = None;
                self.compose(Some(space), window, cx);
                self.save();
            }
        }
    }

    pub(crate) fn toggle_max(&mut self, space: u64, pane: Pane, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(space) {
            s.grid.maximized = if s.grid.maximized.as_ref() == Some(&pane) {
                None
            } else {
                Some(pane)
            };
        }
        self.save();
        cx.notify();
    }

    pub(crate) fn swap_panes(&mut self, space: u64, a: &Pane, b: &Pane, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(space) {
            s.grid.swap(a, b);
        }
        self.save();
        cx.notify();
    }

    pub(crate) fn set_layout(&mut self, space: u64, n: usize, id: &str, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(space) {
            s.grid.layouts.insert(n, id.to_string());
        }
        self.work.popup = None;
        self.save();
        cx.notify();
    }

    /// Shows a file or a diff in the space on screen, in its one viewer pane.
    pub(crate) fn show_viewer(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.current_space() else {
            return;
        };
        // the composer has no grid to put it in; the space's grid shows it next time
        let live = self.live_panes(space);
        if let Some(s) = self.state.space_mut(space) {
            s.grid.panes = live;
            s.grid.show_viewer(pane.clone());
        }
        let viewer = self.viewer(space, cx);
        viewer.update(cx, |v, cx| v.show(pane.clone(), cx));
        if let Screen::Thread(_) = self.screen {
            self.focus_pane(space, pane, true, window, cx);
        } else {
            self.save();
        }
        cx.notify();
    }

    /// The viewer entity for `space`, made on first use.
    pub(crate) fn viewer(&mut self, space: u64, cx: &mut Context<Self>) -> Entity<Viewer> {
        if let Some(v) = self.work.viewers.get(&space) {
            return v.clone();
        }
        let client = self.client.clone();
        let v = cx.new(|cx| Viewer::new(client, cx));
        cx.subscribe(&v, move |root, _, e: &ViewerEvent, cx| match e {
            ViewerEvent::Focused => {
                if let Some(s) = root.state.space_mut(space)
                    && let Some(p) = s.grid.panes.iter().find(|p| p.is_viewer()).cloned()
                {
                    s.grid.focus = Some(p);
                    cx.notify();
                }
            }
        })
        .detach();
        self.work.viewers.insert(space, v.clone());
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

    /// The folder the dock follows: the focused thread's, else the space's.
    fn dock_folder(&self, space: u64) -> Option<PathBuf> {
        let s = self.state.space(space)?;
        let focused = s.grid.focus.as_ref().and_then(|p| match p {
            Pane::Thread { id } => s.threads.iter().find(|t| t.id == *id).map(|t| t.cwd()),
            _ => None,
        });
        focused
            .filter(|p| !p.as_os_str().is_empty())
            .cloned()
            .or_else(|| s.cwd.clone())
    }

    /// Tells the dock what to follow, when that changed.
    fn sync_dock(&mut self, space: u64, cx: &mut Context<Self>) {
        let folder = self.dock_folder(space);
        let viewing = self.state.space(space).and_then(|s| {
            s.grid.panes.iter().find_map(|p| match p {
                Pane::File { path, .. } => Some(path.clone()),
                _ => None,
            })
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

    /// Answers to folder requests, for the dock, the viewers and the Open button.
    pub(crate) fn folder_event(&mut self, e: FolderEvent, cx: &mut Context<Self>) {
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
            FolderEvent::File { .. } | FolderEvent::Diff { .. } | FolderEvent::Git { .. } => {
                for v in self.work.viewers.values() {
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
