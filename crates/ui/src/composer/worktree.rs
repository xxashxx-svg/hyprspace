use std::path::PathBuf;

use gpui::{AnyElement, ClickEvent, Context, Pixels, Point, Window, div, prelude::*};
use hyprspace_proto::git::Worktree;
use hyprspace_proto::{Command, FolderCommand, Launch, Prompt};
use hyprspace_theme::MONO;

use super::{Composer, ComposerEvent};
use crate::assets::icon;
use crate::zoom::px;
use crate::{colors, widgets};

pub(super) struct Waiting {
    request: u64,
    launch: Launch,
    prompt: Option<Prompt>,
    title: String,
    terminal: bool,
}

impl Composer {
    pub(super) fn worktree_on(&self) -> bool {
        self.worktree && !self.remote() && self.branch.is_some()
    }

    fn toggle_worktree(&mut self, cx: &mut Context<Self>) {
        self.worktree = !self.worktree;
        if !self.worktree {
            self.base = None;
        }
        self.keep_draft(cx);
        cx.notify();
    }

    fn open_branches(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        let Some(cwd) = self.cwd() else {
            return;
        };
        self.client
            .send(Command::Folder(FolderCommand::Branches { cwd }));
        self.menu = None;
        self.places_menu = None;
        self.branch_menu = Some(e.position());
        cx.notify();
    }

    pub fn set_branches(&mut self, cwd: &PathBuf, branches: Vec<String>, cx: &mut Context<Self>) {
        if self.cwd().as_ref() == Some(cwd) {
            self.branches = branches;
            cx.notify();
        }
    }

    fn pick_base(&mut self, branch: String, cx: &mut Context<Self>) {
        self.branch_menu = None;
        if self.branch.as_ref() == Some(&branch) {
            self.base = None;
        } else {
            self.base = Some(branch);
            self.worktree = true;
        }
        self.keep_draft(cx);
        cx.notify();
    }

    pub(super) fn start_in_worktree(
        &mut self,
        launch: Launch,
        prompt: Option<Prompt>,
        title: String,
        cx: &mut Context<Self>,
    ) {
        if self.waiting.is_some() {
            return;
        }
        let request = self.next_request;
        self.next_request += 1;
        self.client
            .send(Command::Folder(FolderCommand::AddWorktree {
                request,
                cwd: launch.cwd.clone(),
                name: title.clone(),
                base: self.base.clone(),
            }));
        self.waiting = Some(Waiting {
            request,
            launch,
            prompt,
            title,
            terminal: self.terminal(),
        });
        cx.notify();
    }

    pub fn worktree_added(
        &mut self,
        request: u64,
        result: Result<Worktree, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(w) = self.waiting.take_if(|w| w.request == request) else {
            return;
        };
        match result {
            Ok(worktree) => {
                let mut launch = w.launch;
                launch.cwd = worktree.path.clone();
                cx.emit(ComposerEvent::Start {
                    launch,
                    prompt: w.prompt,
                    title: w.title,
                    terminal: w.terminal,
                    worktree: Some(worktree),
                });
                self.reset(cx);
            }
            Err(e) => {
                if let Some(p) = w.prompt {
                    self.images = p.images;
                }
                self.error = Some(e);
            }
        }
        cx.notify();
    }
}

pub(super) fn branch_chip(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    let checked_out = c.branch.clone()?;
    let shown = c.base.clone().unwrap_or(checked_out);
    Some(
        div()
            .id("composer-branch")
            .flex()
            .items_center()
            .gap(px(5.))
            .min_w_0()
            .h(px(22.))
            .px(px(6.))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
            .child(icon("git-branch", 12., colors::text3()))
            .child(
                div()
                    .max_w(px(200.))
                    .truncate()
                    .font_family(MONO)
                    .child(shown),
            )
            .child(widgets::caret())
            .tooltip(widgets::tip("The branch a new worktree starts from"))
            .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_branches(e, cx)))
            .into_any_element(),
    )
}

pub(super) fn branch_menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let shown = c.base.clone().or_else(|| c.branch.clone());
    let rows: Vec<AnyElement> = c
        .branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let on = shown.as_ref() == Some(b);
            let pick = b.clone();
            div()
                .id(("branch", i))
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(on, |d| d.bg(colors::ink(0.07)))
                .hover(|s| s.bg(colors::ink(0.05)))
                .child(icon("git-branch", 13., colors::text3()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(MONO)
                        .text_size(px(12.))
                        .text_color(colors::text1())
                        .child(b.clone()),
                )
                .when(on, |d| d.child(icon("check", 13., colors::text2())))
                .on_click(
                    cx.listener(move |c, _: &ClickEvent, _, cx| c.pick_base(pick.clone(), cx)),
                )
                .into_any_element()
        })
        .collect();
    let body = div()
        .w(px(260.))
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(10.))
                .pt(px(4.))
                .pb(px(6.))
                .text_size(px(11.5))
                .text_color(colors::text3())
                .child("Start a new worktree from"),
        )
        .child(
            div()
                .id("branches")
                .max_h(px(320.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(1.))
                .children(rows),
        )
        .into_any_element();
    let close = cx.listener(|c, _: &(), _, cx| {
        c.branch_menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Down,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

pub(super) fn chip(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.remote() || c.branch.is_none() {
        return None;
    }
    let on = c.worktree;
    let tint = if on { colors::text1() } else { colors::text3() };
    Some(
        div()
            .id("composer-worktree")
            .flex()
            .items_center()
            .gap(px(5.))
            .h(px(22.))
            .px(px(7.))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(tint)
            .cursor_pointer()
            .when(on, |d| d.bg(colors::ink(0.1)))
            .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
            .child(icon("folder-tree", 12., tint))
            .child(if c.waiting.is_some() {
                "Making a worktree"
            } else {
                "New worktree"
            })
            .tooltip(widgets::tip(
                "Run this thread in its own copy of the repo, on a new branch",
            ))
            .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.toggle_worktree(cx)))
            .into_any_element(),
    )
}
