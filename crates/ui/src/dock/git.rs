// The dock's Git tab, in the shape of the Tauri app's GitPanel (and GitHub Desktop): the branch
// with a push button once it is ahead, the changed files with a tick to stage each, and a commit
// card with a summary and a description. Each file reads name first, its folder dim after it and
// its state at the far end, as in VS Code. A click on a file opens its diff in the viewer.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable, FontWeight, IntoElement, Subscription,
    Window, div, prelude::*, px,
};
use hyprspace_proto::folder::GitStatus;
use hyprspace_proto::git::FileChange;
use hyprspace_proto::{Command, FolderCommand, Pane};
use hyprspace_theme::MONO;

use super::files::Deco;
use super::kinds::file_icon;
use super::{Dock, DockEvent, empty};
use crate::assets::icon;
use crate::colors;
use crate::input::{InputEvent, TextInput};
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    Stage,
    Commit,
    Push,
}

pub struct GitTab {
    status: Option<GitStatus>,
    summary: Entity<TextInput>,
    body: Entity<TextInput>,
    busy: Option<Busy>,
    /// The last commit or push: its line, or git's reason it failed.
    note: Option<Result<String, String>>,
    _subs: Vec<Subscription>,
}

/// Every file ticked, some, or none: the head box shows a check, a dash or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tick {
    All,
    Some,
    None,
}

fn tick(changes: &[FileChange]) -> Tick {
    let staged = changes.iter().filter(|c| c.staged()).count();
    match staged {
        0 => Tick::None,
        n if n == changes.len() => Tick::All,
        _ => Tick::Some,
    }
}

/// The commit message: the summary, then the description after a blank line.
fn message(summary: &str, body: &str) -> String {
    let (s, b) = (summary.trim(), body.trim());
    if b.is_empty() {
        s.to_string()
    } else {
        format!("{s}\n\n{b}")
    }
}

/// A hand-drawn tick box, as the Tauri app drew them: a soft square, the accent when ticked.
fn checkbox(state: Tick, enabled: bool) -> gpui::Div {
    let on = state != Tick::None;
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(15.))
        .rounded(px(4.))
        .border_1()
        .border_color(if on {
            colors::accent()
        } else {
            colors::text3().opacity(0.55)
        })
        .when(on, |d| d.bg(colors::accent()))
        .when(!enabled, |d| d.opacity(0.5))
        .when(state == Tick::All, |d| {
            d.child(icon("check", 11., colors::on_accent()))
        })
        .when(state == Tick::Some, |d| {
            d.child(div().w(px(7.)).h(px(1.5)).bg(colors::on_accent()))
        })
}

/// The commit button while there is nothing it can do yet: flat, not a faded accent.
fn idle_button(id: &'static str, label: String) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .h(px(26.))
        .px(px(10.))
        .rounded(px(6.))
        .bg(colors::ink(0.05))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(label)
}

impl GitTab {
    pub fn new(cx: &mut Context<Dock>) -> Self {
        let summary = cx.new(|cx| TextInput::new("Summary (required)", false, cx));
        let body = cx.new(|cx| TextInput::new("Description", true, cx));
        let submit =
            |d: &mut Dock, _: Entity<TextInput>, e: &InputEvent, cx: &mut Context<Dock>| match e {
                InputEvent::Submit => d.commit(false, cx),
                InputEvent::Changed => cx.notify(),
                _ => {}
            };
        let subs = vec![cx.subscribe(&summary, submit), cx.subscribe(&body, submit)];
        Self {
            status: None,
            summary,
            body,
            busy: None,
            note: None,
            _subs: subs,
        }
    }

    /// A new folder: forget the last one's tree and note, keep what is typed.
    pub fn reset(&mut self, _: &mut Context<Dock>) {
        self.status = None;
        self.busy = None;
        self.note = None;
    }

    /// How many files have changes, as of the last look.
    pub fn changes(&self) -> usize {
        self.status.as_ref().map_or(0, |s| s.changes.len())
    }

    pub fn status(&mut self, status: GitStatus) {
        // a stage answers with GitDone and then this; the busy flag clears on GitDone
        self.status = Some(status);
    }

    pub fn done(&mut self, result: Result<String, String>, cx: &mut Context<Dock>) {
        let was = self.busy.take();
        if result.is_ok() && was == Some(Busy::Commit) {
            self.summary.update(cx, |i, cx| i.set_text("", cx));
            self.body.update(cx, |i, cx| i.set_text("", cx));
        }
        // a stage that worked says nothing; the tick is the answer
        if result.is_err() || was != Some(Busy::Stage) {
            self.note = Some(result);
        }
    }
}

impl Dock {
    fn stage(&mut self, path: String, stage: bool, cx: &mut Context<Self>) {
        let Some(cwd) = self.folder.clone() else {
            return;
        };
        if self.git.busy.is_some() {
            return;
        }
        self.git.busy = Some(Busy::Stage);
        self.client
            .send(Command::Folder(FolderCommand::Stage { cwd, path, stage }));
        cx.notify();
    }

    fn commit(&mut self, push: bool, cx: &mut Context<Self>) {
        let Some(cwd) = self.folder.clone() else {
            return;
        };
        let summary = self.git.summary.read(cx).text().to_string();
        let body = self.git.body.read(cx).text().to_string();
        let staged = self
            .git
            .status
            .as_ref()
            .is_some_and(|s| s.changes.iter().any(|c| c.staged()));
        if summary.trim().is_empty() || !staged || self.git.busy.is_some() {
            return;
        }
        self.git.busy = Some(Busy::Commit);
        self.git.note = None;
        self.client.send(Command::Folder(FolderCommand::Commit {
            cwd,
            message: message(&summary, &body),
            push,
        }));
        cx.notify();
    }

    fn push(&mut self, cx: &mut Context<Self>) {
        let Some(cwd) = self.folder.clone() else {
            return;
        };
        if self.git.busy.is_some() {
            return;
        }
        self.git.busy = Some(Busy::Push);
        self.git.note = None;
        self.client
            .send(Command::Folder(FolderCommand::Push { cwd }));
        cx.notify();
    }

    fn branch_row(&self, s: &GitStatus, cx: &mut Context<Self>) -> AnyElement {
        let b = &s.branch;
        let name = if b.branch.is_empty() || b.branch == "HEAD" {
            "No commits yet".to_string()
        } else {
            b.branch.clone()
        };
        let busy = self.git.busy.is_some();
        let end = if b.ahead > 0 {
            div()
                .id("git-push")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .h(px(24.))
                .px(px(9.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface2())
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .when(busy, |d| d.opacity(0.5))
                .when(!busy, |d| {
                    d.cursor_pointer()
                        .hover(|s| s.bg(colors::surface3()))
                        .on_click(cx.listener(|d, _: &ClickEvent, _, cx| d.push(cx)))
                })
                .child(icon("arrow-up", 12., colors::text1()))
                .child(if self.git.busy == Some(Busy::Push) {
                    "Pushing".to_string()
                } else {
                    format!("Push {}", b.ahead)
                })
                .into_any_element()
        } else {
            div()
                .id("git-refresh")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::surface2()))
                .child(icon("refresh-cw", 12., colors::text3()))
                .on_click(cx.listener(|d, _: &ClickEvent, _, cx| d.refresh(cx)))
                .into_any_element()
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(7.))
            .h(px(36.))
            .px(px(12.))
            .border_b_1()
            .border_color(colors::border1())
            .child(icon("git-branch", 13., colors::text3()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(12.))
                    .text_color(colors::text1())
                    .child(name),
            )
            .when(b.behind > 0, |d| {
                d.child(
                    div()
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(colors::text3())
                        .child(format!("↓{}", b.behind)),
                )
            })
            .child(end)
            .into_any_element()
    }

    fn file_row(&self, ix: usize, c: &FileChange, cx: &mut Context<Self>) -> AnyElement {
        let deco = Deco::of(&c.status);
        let (dir, name) = match c.path.rfind('/') {
            Some(i) => (c.path[..=i].to_string(), c.path[i + 1..].to_string()),
            None => (String::new(), c.path.clone()),
        };
        let staged = c.staged();
        let busy = self.git.busy.is_some();
        let (tick_path, diff_path) = (c.path.clone(), c.path.clone());
        let (glyph, tint) = file_icon(&name);
        let deleted = deco == Deco::Deleted;
        div()
            .id(("git-file", ix))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(6.))
            .rounded(px(6.))
            .hover(|s| s.bg(colors::surface3().opacity(0.55)))
            .child(
                div()
                    .id(("git-tick", ix))
                    .flex_none()
                    .cursor_pointer()
                    .child(checkbox(if staged { Tick::All } else { Tick::None }, !busy))
                    .on_click(cx.listener(move |d, _: &ClickEvent, _, cx| {
                        d.stage(tick_path.clone(), !staged, cx)
                    })),
            )
            .child(
                div()
                    .id(("git-open", ix))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h_full()
                    .cursor_pointer()
                    .child(icon(glyph, 14., tint).when(deleted, |i| i.opacity(0.6)))
                    // the folder gives way first, then the name truncates
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(if deleted {
                                colors::text3()
                            } else {
                                colors::text1()
                            })
                            .when(deleted, |d| d.line_through())
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .child(dir.trim_end_matches('/').to_string()),
                    )
                    .on_click(cx.listener(move |d, _: &ClickEvent, _, cx| {
                        if let Some(cwd) = d.folder.clone() {
                            cx.emit(DockEvent::Open(Pane::Diff {
                                cwd,
                                path: diff_path.clone(),
                            }));
                        }
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(5.))
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .when(c.added > 0, |d| {
                        d.child(
                            div()
                                .text_color(colors::diff_add())
                                .child(format!("+{}", c.added)),
                        )
                    })
                    .when(c.removed > 0, |d| {
                        d.child(
                            div()
                                .text_color(colors::diff_del())
                                .child(format!("-{}", c.removed)),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(12.))
                    .flex()
                    .justify_center()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(deco.color())
                    .child(deco.letter(&c.status)),
            )
            .into_any_element()
    }

    pub(super) fn git_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(s) = self.git.status.clone() else {
            return empty("Reading the working tree");
        };
        if s.root.is_none() {
            return empty("This folder is not a git repository.");
        }
        let n = s.changes.len();
        let ticked = tick(&s.changes);
        let staged = s.changes.iter().filter(|c| c.staged()).count();
        let added: u32 = s.changes.iter().map(|c| c.added).sum();
        let removed: u32 = s.changes.iter().map(|c| c.removed).sum();
        let busy = self.git.busy.is_some();
        let summary = self.git.summary.read(cx).text().trim().to_string();
        let can_commit = !busy && !summary.is_empty() && staged > 0;
        let branch = if s.branch.branch.is_empty() {
            "branch".to_string()
        } else {
            s.branch.branch.clone()
        };
        let files: Vec<AnyElement> = s
            .changes
            .iter()
            .enumerate()
            .map(|(i, c)| self.file_row(i, c, cx))
            .collect();
        let hint: Option<AnyElement> = match &self.git.note {
            Some(Err(e)) => Some(
                div()
                    .text_color(colors::error())
                    .child(e.clone())
                    .into_any_element(),
            ),
            Some(Ok(line)) if !line.is_empty() => {
                Some(div().child(line.clone()).into_any_element())
            }
            _ if staged > 0 => Some(
                div()
                    .child(format!("{staged} of {n} files ticked"))
                    .into_any_element(),
            ),
            _ if n > 0 => Some(div().child("Tick the files to include").into_any_element()),
            _ => None,
        };
        let commit_label = if self.git.busy == Some(Busy::Commit) {
            "Committing".to_string()
        } else {
            format!("Commit to {branch}")
        };
        let typing = self.git.summary.focus_handle(cx).is_focused(window)
            || self.git.body.focus_handle(cx).is_focused(window);
        let commit = if can_commit {
            widgets::primary("git-commit", commit_label)
                .on_click(cx.listener(|d, _: &ClickEvent, _, cx| d.commit(false, cx)))
        } else {
            idle_button("git-commit", commit_label)
        };
        let commit_push = if can_commit {
            widgets::button("git-commit-push", "Commit and push")
                .on_click(cx.listener(|d, _: &ClickEvent, _, cx| d.commit(true, cx)))
        } else {
            idle_button("git-commit-push", "Commit and push".into())
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.branch_row(&s, cx))
            // nothing changed: the note below says so
            .when(n > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .h(px(32.))
                        .px(px(12.))
                        .border_b_1()
                        .border_color(colors::border1())
                        .child(
                            div()
                                .id("git-all")
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(12.))
                                .text_color(colors::text2())
                                .when(n > 0 && !busy, |d| {
                                    d.cursor_pointer().on_click(cx.listener(
                                        move |d, _: &ClickEvent, _, cx| {
                                            d.stage(String::new(), ticked != Tick::All, cx)
                                        },
                                    ))
                                })
                                .child(checkbox(ticked, n > 0 && !busy))
                                .child(format!(
                                    "{n} changed {}",
                                    if n == 1 { "file" } else { "files" }
                                )),
                        )
                        .child(div().flex_1())
                        // the whole tree's lines, like zeron's "80 changed files +9341 -3630"
                        .when(added > 0, |d| {
                            d.child(
                                div()
                                    .ml(px(6.))
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .text_color(colors::diff_add())
                                    .child(format!("+{added}")),
                            )
                        })
                        .when(removed > 0, |d| {
                            d.child(
                                div()
                                    .ml(px(6.))
                                    .font_family(MONO)
                                    .text_size(px(11.))
                                    .text_color(colors::diff_del())
                                    .child(format!("-{removed}")),
                            )
                        }),
                )
            })
            .child(
                div()
                    .id("git-files")
                    .flex_1()
                    .min_h(px(80.))
                    .overflow_y_scroll()
                    .px(px(6.))
                    .py(px(4.))
                    .when(n == 0, |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(6.))
                                .pt(px(40.))
                                .px(px(16.))
                                .child(icon("circle-check", 22., colors::text3()))
                                .child(
                                    div()
                                        .mt(px(2.))
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(colors::text2())
                                        .child("No changes"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(colors::text3())
                                        .child("The working tree is clean."),
                                ),
                        )
                    })
                    .children(files),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .pt(px(12.))
                    .px(px(12.))
                    .pb(px(12.))
                    .border_t_1()
                    .border_color(colors::border1())
                    .bg(colors::surface1())
                    // the message as one card: its summary, a hairline, its description
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(if typing {
                                colors::accent()
                            } else {
                                colors::border2()
                            })
                            .bg(colors::surface2())
                            .text_size(px(12.5))
                            .child(
                                div()
                                    .px(px(10.))
                                    .py(px(8.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.git.summary.clone()),
                            )
                            .child(div().h(px(1.)).bg(colors::ink(0.06)))
                            .child(
                                div()
                                    .min_h(px(56.))
                                    .px(px(10.))
                                    .py(px(8.))
                                    .child(self.git.body.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .child(commit.flex_1().justify_center())
                            .child(commit_push.flex_1().justify_center()),
                    )
                    .child(
                        div()
                            .min_h(px(14.))
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .children(hint),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(status: &str) -> FileChange {
        FileChange {
            path: "a".into(),
            status: status.into(),
            added: 0,
            removed: 0,
        }
    }

    #[test]
    fn the_head_box_reads_all_some_or_none() {
        assert_eq!(tick(&[c("M "), c("A ")]), Tick::All);
        assert_eq!(tick(&[c("M "), c(" M")]), Tick::Some);
        assert_eq!(tick(&[c("??")]), Tick::None);
        assert_eq!(tick(&[]), Tick::None);
    }

    #[test]
    fn messages_join_summary_and_description() {
        assert_eq!(message(" Fix it ", ""), "Fix it");
        assert_eq!(message("Fix", " why\n"), "Fix\n\nwhy");
    }
}
