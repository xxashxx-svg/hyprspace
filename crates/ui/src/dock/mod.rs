// The right dock (Ctrl+Shift+G), after the Tauri app's Dock: the folder of the thread on screen as a
// lazy file tree, its git working tree with a commit box, and the thread's side chats. It follows
// whatever the root tells it to, polls git every few seconds while it is out, and hands files and
// diffs back to the root to show in the viewer card.

mod automations;
mod files;
mod git;
pub(crate) mod kinds;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, ClickEvent, Context, EventEmitter, FontWeight, IntoElement, Render,
    SharedString, Task, Window, div, prelude::*,
};
use hyprspace_proto::state::DockTab;
use hyprspace_proto::{Client, Command, FolderCommand, FolderEvent, Pane};

use crate::assets::icon;
use crate::colors;
use crate::zoom::px;
use files::Tree;
use git::GitTab;

pub enum DockEvent {
    /// A file or a diff for the viewer.
    Open(Pane),
    Tab(DockTab),
    Hide,
    /// A side chat's tab picked, or `None` for Files or Git.
    Side(Option<u64>),
    BringBack(u64),
    DeleteSide(u64),
    RunAutomation(u64),
    PauseAutomation(u64, bool),
    DeleteAutomation(u64),
    EditAutomation(u64),
    /// Open a thread: the one an automation just sent to.
    GoTo(u64),
}

pub use automations::Automation;

/// A side chat of the thread on screen, drawn by its own view.
#[derive(Clone)]
pub struct Side {
    pub id: u64,
    pub title: SharedString,
    pub view: AnyView,
}

pub struct Dock {
    client: Client,
    machine: Option<String>,
    folder: Option<PathBuf>,
    open: bool,
    tab: DockTab,
    tree: Tree,
    git: GitTab,
    sides: Vec<Side>,
    side: Option<u64>,
    /// The thread the side chats belong to isn't working, so one can be brought back.
    idle: bool,
    automations: Vec<Automation>,
    /// A thread is on screen, so the Automations tab has something to belong to.
    on_thread: bool,
    _poll: Task<()>,
}

impl EventEmitter<DockEvent> for Dock {}

/// How often git is asked again while the dock is out, behind the engine's watcher.
const POLL: Duration = Duration::from_secs(30);

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
            machine: None,
            folder: None,
            open: false,
            tab: DockTab::Files,
            tree: Tree::default(),
            git: GitTab::new(cx),
            sides: Vec::new(),
            side: None,
            idle: false,
            automations: Vec::new(),
            on_thread: false,
            _poll: poll,
        }
    }

    /// What to show: the folder to follow, whether the dock is out, its tab, and the file the
    /// viewer has open (its row is marked).
    pub fn sync(
        &mut self,
        folder: Option<PathBuf>,
        machine: Option<String>,
        open: bool,
        tab: DockTab,
        viewing: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let moved = folder != self.folder || machine != self.machine;
        if machine != self.machine {
            self.client = self.client.via(machine.clone());
            self.machine = machine;
        }
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

    /// The side chats of the thread on screen, the one picked, and whether that thread is idle.
    pub fn set_sides(
        &mut self,
        sides: Vec<Side>,
        side: Option<u64>,
        idle: bool,
        cx: &mut Context<Self>,
    ) {
        self.sides = sides;
        self.side = side;
        self.idle = idle;
        cx.notify();
    }

    pub fn machine(&self) -> Option<&str> {
        self.machine.as_deref()
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
            FolderEvent::Git { status, .. } if self.git.status.as_ref() == Some(status) => return,
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
        if self.side.take().is_some() {
            cx.emit(DockEvent::Side(None));
        }
        cx.emit(DockEvent::Tab(tab));
        cx.notify();
    }

    fn shown_side(&self) -> Option<&Side> {
        let id = self.side?;
        self.sides.iter().find(|s| s.id == id)
    }

    fn tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        // the Git tab carries how many files changed, so it shows from the Files tab too
        let changed = self.git.changes();
        let picked = self.shown_side().map(|s| s.id);
        let tab = |id: &'static str, glyph: &str, label: &'static str, which: DockTab| {
            let on = picked.is_none() && self.tab == which;
            let count = (which == DockTab::Git && changed > 0).then(|| {
                div()
                    .px(px(5.))
                    .rounded(px(4.))
                    .bg(colors::ink(0.08))
                    .font_family(hyprspace_theme::MONO)
                    .text_size(px(10.5))
                    .text_color(colors::text2())
                    .child(changed.to_string())
            });
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
                .children(count)
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
            .when(
                self.on_thread
                    && (!self.automations.is_empty() || self.tab == DockTab::Automations),
                |d| {
                    d.child(tab(
                        "dock-automations",
                        "clock",
                        "Automations",
                        DockTab::Automations,
                    ))
                },
            )
            .children(self.sides.iter().map(|s| {
                let (id, on) = (s.id, picked == Some(s.id));
                let ink = if on { colors::text1() } else { colors::text3() };
                div()
                    .id(("dock-side", id))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .max_w(px(160.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(ink)
                    .when(on, |d| d.bg(colors::surface3()))
                    .cursor_pointer()
                    .when(!on, |d| {
                        d.hover(|s| s.bg(colors::surface2()).text_color(colors::text1()))
                    })
                    .child(icon("message-square", 13., ink))
                    .child(div().min_w_0().truncate().child(s.title.clone()))
                    .on_click(cx.listener(move |d, _: &ClickEvent, _, cx| {
                        d.side = Some(id);
                        cx.emit(DockEvent::Side(Some(id)));
                        cx.notify();
                    }))
            }))
            .child(div().flex_1())
            .child(
                crate::widgets::icon_button("dock-x", "x", crate::widgets::LARGE)
                    .tooltip(crate::widgets::tip("Hide"))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DockEvent::Hide))),
            )
            .into_any_element()
    }
}

/// The dim note a tab shows when it has nothing to list.
fn loading(text: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(14.))
        .py(px(16.))
        .text_size(px(12.))
        .line_height(px(19.))
        .text_color(colors::text3())
        .child(crate::spinner::ring("dock-loading", colors::text3()))
        .child(text)
        .into_any_element()
}

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

/// A side chat's title, Bring back and Delete, over the side chat itself.
fn side_chat(side: &Side, idle: bool, cx: &mut Context<Dock>) -> AnyElement {
    let id = side.id;
    let back = crate::widgets::button(("side-back", id), "Bring back")
        .h(px(26.))
        .px(px(10.))
        .text_size(px(12.))
        .tooltip(crate::widgets::tip(if idle {
            "Adds this side chat to the thread's next message."
        } else {
            "Available when the thread is idle."
        }))
        .when(!idle, |d| d.opacity(0.5).cursor_default())
        .when(idle, |d| {
            d.on_click(
                cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(DockEvent::BringBack(id))),
            )
        });
    let head = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(40.))
        .px(px(12.))
        .border_b_1()
        .border_color(colors::border1())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text2())
                .child(side.title.clone()),
        )
        .child(back)
        .child(
            crate::widgets::icon_button(("side-delete", id), "trash-2", crate::widgets::LARGE)
                .tooltip(crate::widgets::tip("Delete side chat"))
                .on_click(
                    cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(DockEvent::DeleteSide(id))),
                ),
        );
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(head)
        .child(div().flex_1().min_h_0().child(side.view.clone()))
        .into_any_element()
}

impl Render for Dock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = self.shown_side().cloned();
        let body = match (&self.folder, self.tab, side) {
            (_, _, Some(side)) => side_chat(&side, self.idle, cx),
            (_, DockTab::Automations, _) if self.on_thread => self.automations_tab(cx),
            (None, ..) => empty("No folder yet. Open a project, or start a thread in a folder."),
            (Some(_), DockTab::Files | DockTab::Automations, _) => self.files(cx),
            (Some(_), DockTab::Git, _) => self.git_tab(window, cx),
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
