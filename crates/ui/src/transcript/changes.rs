use gpui::{AnyElement, ClickEvent, Context, FontWeight, SharedString, div, prelude::*};
use hyprspace_proto::run::RunFile;
use hyprspace_proto::{Command, Pane};
use hyprspace_theme::MONO;

use super::model::{Item, Undo};
use super::render::{fold_key, folding, reveal, settle_fold};
use super::{TranscriptEvent, TranscriptView};
use crate::assets::icon;
use crate::dock::kinds::file_icon;
use crate::hover::fill_on_hover;
use crate::zoom::px;
use crate::{colors, widgets};

const FIRST: usize = 8;

impl TranscriptView {
    fn undo_run(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.model.running() {
            return;
        }
        let Some(Item::Changes {
            before,
            after,
            files,
            undo,
            ..
        }) = self.model.items.get_mut(ix)
        else {
            return;
        };
        if matches!(undo, Undo::Busy | Undo::Done(_)) {
            return;
        }
        *undo = Undo::Busy;
        self.client.send(Command::Undo {
            id: self.id,
            cwd: self.launch.cwd.clone(),
            before: before.clone(),
            after: after.clone(),
            files: files.iter().map(|f| f.path.clone()).collect(),
        });
        cx.notify();
    }
}

fn counts(added: u32, removed: u32, size: f32) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .gap(px(6.))
        .font_family(MONO)
        .text_size(px(size))
        .font_features(widgets::tabular())
        .child(
            div()
                .text_color(colors::diff_add())
                .child(format!("+{added}")),
        )
        .child(
            div()
                .text_color(colors::diff_del())
                .child(format!("-{removed}")),
        )
        .into_any_element()
}

fn row(
    v: &TranscriptView,
    ix: usize,
    i: usize,
    f: &RunFile,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let (dir, name) = match f.path.rsplit_once('/') {
        Some((dir, name)) => (format!("{dir}/"), name.to_string()),
        None => (String::new(), f.path.clone()),
    };
    let pane = run_pane(v, ix, &f.path);
    let local = pane.is_some();
    let (glyph, tint) = file_icon(&name);
    div()
        .id(SharedString::from(format!("changed-{ix}-{i}")))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .mx(px(8.))
        .px(px(8.))
        .rounded(px(6.))
        .when(local, |d| {
            d.cursor_pointer()
                .child(fill_on_hover("hover", colors::ink(0.05), 6.))
        })
        .child(icon(glyph, 14., tint))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(MONO)
                .text_size(px(12.))
                .child(
                    div()
                        .flex()
                        .child(div().text_color(colors::text3()).child(dir))
                        .child(div().text_color(colors::text1()).child(name)),
                ),
        )
        .child(counts(f.added, f.removed, 11.))
        .when_some(pane, |d, pane| {
            d.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.emit(TranscriptEvent::Open(pane.clone()))
            }))
        })
        .into_any_element()
}

fn run_pane(v: &TranscriptView, ix: usize, path: &str) -> Option<Pane> {
    let Some(Item::Changes { before, after, .. }) = v.model.items.get(ix) else {
        return None;
    };
    (!v.remote).then(|| Pane::Changes {
        cwd: v.launch.cwd.clone(),
        before: before.clone(),
        after: after.clone(),
        path: path.to_string(),
    })
}

pub(super) fn card(
    v: &TranscriptView,
    ix: usize,
    files: &[RunFile],
    all: bool,
    undo: &Undo,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let (added, removed) = files
        .iter()
        .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    let title = match files.len() {
        1 => "Edited 1 file".to_string(),
        n => format!("Edited {n} files"),
    };
    let live = v.model.running();
    let undo_button = match undo {
        Undo::Done(_) => div()
            .text_size(px(12.5))
            .text_color(colors::text3())
            .child("Undone")
            .into_any_element(),
        _ => {
            let off = live || *undo == Undo::Busy;
            div()
                .id(("undo-run", ix))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .px(px(10.))
                .rounded(px(6.))
                .text_size(px(12.5))
                .text_color(colors::text1())
                .child(if *undo == Undo::Busy {
                    "Undoing"
                } else {
                    "Undo"
                })
                .child(icon("rotate-ccw", 12., colors::text2()))
                .when(off, |d| d.opacity(0.5))
                .when(!off, |d| {
                    d.cursor_pointer()
                        .hover(|s| s.bg(colors::ink(0.06)))
                        .tooltip(widgets::tip(
                            "Put these files back the way they were before this run",
                        ))
                        .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.undo_run(ix, cx)))
                })
                .into_any_element()
        }
    };
    let note = match undo {
        Undo::Done(line) => Some((line.clone(), colors::text3())),
        Undo::Failed(e) => Some((e.clone(), colors::error())),
        _ => None,
    };
    let head = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(16.))
        .pr(px(8.))
        .py(px(8.))
        .child(
            div()
                .flex_none()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(title),
        )
        .child(counts(added, removed, 11.))
        .child(div().flex_1())
        .when_some(run_pane(v, ix, ""), |d, pane| {
            d.child(undo_button).child(
                widgets::button(("view-changes", ix), "View changes").on_click(cx.listener(
                    move |_, _: &ClickEvent, _, cx| cx.emit(TranscriptEvent::Open(pane.clone())),
                )),
            )
        });
    let first = files.len().min(FIRST);
    let more = if all { 0 } else { files.len() - first };
    div()
        .flex()
        .flex_col()
        .rounded(px(8.))
        .bg(colors::ink(0.03))
        .overflow_hidden()
        .pb(px(8.))
        .child(head)
        .children(note.map(|(line, color)| {
            div()
                .px(px(16.))
                .pb(px(10.))
                .text_size(px(12.))
                .text_color(color)
                .child(line)
        }))
        .children(
            files[..first]
                .iter()
                .enumerate()
                .map(|(i, f)| row(v, ix, i, f, cx)),
        )
        .when(
            first < files.len() && (all || folding(cx, ("more-files", ix))),
            |d| {
                let rest = div().flex().flex_col().children(
                    files[first..]
                        .iter()
                        .enumerate()
                        .map(|(i, f)| row(v, ix, first + i, f, cx)),
                );
                d.child(reveal(cx, ("more-files", ix), all, rest))
            },
        )
        .when(more > 0 || (all && files.len() > FIRST), |d| {
            d.child(
                div()
                    .id(("more-files", ix))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(28.))
                    .mx(px(8.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .child(fill_on_hover("hover", colors::ink(0.05), 6.))
                    .text_size(px(12.5))
                    .text_color(colors::text3())
                    .child(if all {
                        "Show fewer files".to_string()
                    } else if more == 1 {
                        "Show 1 more file".to_string()
                    } else {
                        format!("Show {more} more files")
                    })
                    .child(icon(
                        if all { "chevron-up" } else { "chevron-down" },
                        12.,
                        colors::text3(),
                    ))
                    .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                        if let Some(Item::Changes { all, .. }) = v.model.items.get_mut(ix) {
                            *all = !*all;
                        }
                        crate::slide::flip(fold_key(cx, &("more-files", ix).into()));
                        settle_fold(cx);
                        cx.notify();
                    })),
            )
        })
        .into_any_element()
}
