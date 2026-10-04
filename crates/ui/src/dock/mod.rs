// The right dock (Ctrl+Shift+G), after the Tauri app's Dock: the folder of the thread on screen as a
// lazy file tree, and its git working tree with a commit box. It follows whatever the root tells
// it to, polls git every few seconds while it is out, and hands files and diffs back to the root
// to show in the viewer card.

mod files;
mod git;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, EventEmitter, FontWeight, IntoElement, Render, Task, Window,
    div, prelude::*, px,
};
use hyprspace_proto::state::DockTab;
use hyprspace_proto::{Client, Command, FolderCommand, FolderEvent, Pane};

use crate::assets::icon;
use crate::colors;
use files::Tree;
use git::GitTab;

pub enum DockEvent {
    /// A file or a diff for the viewer.
    Open(Pane),
    Tab(DockTab),
    Hide,
}

pub struct Dock {
    client: Client,
    folder: Option<PathBuf>,
    open: bool,
    tab: DockTab,
    tree: Tree,
    git: GitTab,
    _poll: Task<()>,
}

impl EventEmitter<DockEvent> for Dock {}

/// How often git is asked again while the dock is out, as the Tauri app did.
const POLL: Duration = Duration::from_secs(4);

impl Dock {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |d, _| d.refresh_git()).is_err() {
                    break;
                }
            }
        });
        Self {
            client: client.clone(),
            folder: None,
            open: false,
            tab: DockTab::Files,
            tree: Tree::default(),
            git: GitTab::new(cx),
            _poll: poll,
        }
    }

    /// What to show: the folder to follow, whether the dock is out, its tab, and the file the
    /// viewer has open (its row is marked).
    pub fn sync(
        &mut self,
        folder: Option<PathBuf>,
        open: bool,
        tab: DockTab,
        viewing: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let moved = folder != self.folder;
        let opened = open && !self.open;
        self.folder = folder;
        self.open = open;
        self.tab = tab;
        self.tree.viewing = viewing;
        if moved {
            self.tree.reset();
            self.git.reset(cx);
        }
        if open && (moved || opened) {
            if let Some(f) = self.folder.clone() {
                self.tree.load(&self.client, f);
            }
            self.refresh_git();
        }
        cx.notify();
    }

    fn refresh_git(&self) {
        if self.open
            && let Some(cwd) = &self.folder
        {
            self.client.send(Command::Folder(FolderCommand::GitStatus {
                cwd: cwd.clone(),
            }));
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.tree.reload(&self.client);
        self.refresh_git();
        cx.notify();
    }

    pub fn event(&mut self, e: &FolderEvent, cx: &mut Context<Self>) {
        match e {
            FolderEvent::Dir { path, entries } => self.tree.listed(path, entries),
            FolderEvent::Git { cwd, status } if Some(cwd) == self.folder.as_ref() => {
                self.tree.decorate(status);
                self.git.status(status.clone());
            }
            FolderEvent::GitDone { cwd, result } if Some(cwd) == self.folder.as_ref() => {
                self.git.done(result.clone(), cx);
            }
            _ => return,
        }
        cx.notify();
    }

    fn set_tab(&mut self, tab: DockTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.emit(DockEvent::Tab(tab));
        cx.notify();
    }

    fn tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = |id: &'static str, glyph: &str, label: &'static str, which: DockTab| {
            let on = self.tab == which;
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(6.))
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { colors::text1() } else { colors::text3() })
                .when(on, |d| d.bg(colors::surface3()))
                .cursor_pointer()
                .hover(|s| {
                    if on {
                        s
                    } else {
                        s.bg(colors::surface2()).text_color(colors::text1())
                    }
                })
                .child(icon(
                    glyph,
                    13.,
                    if on { colors::text1() } else { colors::text3() },
                ))
                .child(label)
                .on_click(cx.listener(move |d, _: &ClickEvent, _, cx| d.set_tab(which, cx)))
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .h(px(38.))
            .px(px(6.))
            .border_b_1()
            .border_color(colors::border1())
            .child(tab("dock-files", "folder-tree", "Files", DockTab::Files))
            .child(tab("dock-git", "git-branch", "Git", DockTab::Git))
            .child(div().flex_1())
            .child(
                div()
                    .id("dock-x")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::surface2()))
                    .child(icon("x", 14., colors::text3()))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DockEvent::Hide))),
            )
            .into_any_element()
    }
}

/// The dim note a tab shows when it has nothing to list.
fn empty(text: &'static str) -> AnyElement {
    div()
        .px(px(14.))
        .py(px(16.))
        .text_size(px(12.5))
        .line_height(px(19.))
        .text_color(colors::text3())
        .child(text)
        .into_any_element()
}

impl Render for Dock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match (&self.folder, self.tab) {
            (None, _) => empty("No folder yet. Open a project, or start a thread in a folder."),
            (Some(_), DockTab::Files) => self.files(cx),
            (Some(_), DockTab::Git) => self.git_tab(window, cx),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(colors::text1())
            .child(self.tabs(cx))
            .child(div().flex_1().min_h_0().flex().flex_col().child(body))
    }
}
