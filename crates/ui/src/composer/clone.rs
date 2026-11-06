// The clone list, shown under the composer when its text starts with a repository link: each row
// is one whole action, a folder to clone into, with the usual one first so Enter clones it. Up
// and Down pick another; Escape keeps the text as a message instead. While git runs, the list is
// one row with its progress. Text after the link becomes the new project's first message.

use std::path::{Path, PathBuf};

use gpui::{AnyElement, ClickEvent, Context, Entity, IntoElement, div, prelude::*, relative};
use hyprspace_theme::MONO;

use super::Composer;
use super::repo::RepoRef;
use crate::assets::icon;
use crate::colors;
use crate::input::TextInput;
use crate::zoom::px;

pub struct CloneCard {
    /// The link the list was set up for; a different one resets it.
    pub url: String,
    pub name: Entity<TextInput>,
    /// The highlighted row.
    pub pick: usize,
    /// Escape put the list away, so Enter sends the text as a message.
    pub off: bool,
    /// A folder picked with "Clone somewhere else", offered first.
    pub elsewhere: Option<PathBuf>,
    /// The current project's folder is empty, so the clone can go straight into it.
    pub here_ok: bool,
    /// Where the running clone goes.
    pub into: Option<PathBuf>,
    /// The running clone goes straight into the current project's folder and starts there.
    pub here: bool,
    pub request: Option<u64>,
    pub line: Option<String>,
    pub error: Option<String>,
}

impl CloneCard {
    pub fn busy(&self) -> bool {
        self.request.is_some()
    }
}

/// One row: a folder to clone a new folder into, a way to pick one, or the current project's
/// own (empty) folder.
#[derive(Clone, PartialEq)]
pub enum Row {
    Into { parent: PathBuf, why: String },
    Elsewhere,
    Here(PathBuf),
}

// git's counters, in the words someone waiting on them would use
const PHASES: &[(&str, &str)] = &[
    ("Enumerating objects", "Listing files on the server"),
    ("Counting objects", "Counting files on the server"),
    ("Compressing objects", "Packing on the server"),
    ("Receiving objects", "Downloading"),
    ("Resolving deltas", "Unpacking"),
    ("Updating files", "Writing files"),
];

/// "Receiving objects:  62% (620/1000)" reads as ("Downloading", Some(62)).
pub fn progress(line: Option<&str>) -> (String, Option<u8>) {
    let Some(line) = line else {
        return ("Connecting".into(), None);
    };
    let line = line.trim_start_matches("remote:").trim();
    // "Cloning into 'C:\...'" has a colon of its own, in the drive
    if line.starts_with("Cloning into") {
        return ("Connecting".into(), None);
    }
    let Some((phase, rest)) = line.split_once(':') else {
        return ("Working".into(), None);
    };
    let pct = rest
        .trim()
        .split('%')
        .next()
        .and_then(|n| n.trim().parse::<u8>().ok());
    let label = PHASES
        .iter()
        .find(|(git, _)| *git == phase.trim())
        .map_or(phase.trim(), |(_, ours)| ours);
    (label.to_string(), pct)
}

fn shown(p: &Path) -> String {
    p.display().to_string()
}

/// "C:\Main" and "hyprcall" as a path with the new folder's name in full color.
fn path(parent: &Path, name: &str) -> gpui::Div {
    let sep = std::path::MAIN_SEPARATOR;
    let parent = shown(parent);
    let parent = parent.trim_end_matches(['\\', '/']).to_string();
    div()
        .flex()
        .min_w_0()
        .font_family(MONO)
        .text_size(px(12.))
        .child(
            div()
                .truncate()
                .text_color(colors::text2())
                .child(format!("{parent}{sep}")),
        )
        .child(
            div()
                .flex_none()
                .text_color(colors::text1())
                .child(name.to_string()),
        )
}

fn row_frame(i: usize, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(("clone-row", i))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(9.))
        .cursor_pointer()
        .when(on, |d| d.bg(colors::ink(0.07)))
        .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.04))))
}

fn hint(keys: &'static str, says: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .child(
            div()
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(colors::text2())
                .child(keys),
        )
        .child(says)
}

fn frame() -> gpui::Div {
    div()
        .mt(px(10.))
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(14.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .overflow_hidden()
        .text_size(px(12.5))
        .text_color(colors::text1())
}

pub fn render(
    c: &CloneCard,
    repo: &RepoRef,
    rows: &[Row],
    target: Option<&str>,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let name = c.name.read(cx).text().trim().to_string();
    let link = repo
        .url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("git@")
        .trim_end_matches(".git")
        .to_string();
    let top = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(11.))
        .child(icon("github", 15., colors::text1()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_family(MONO)
                .text_size(px(12.))
                .child(link),
        );
    if let (true, Some(into)) = (c.busy(), &c.into) {
        let (label, pct) = progress(c.line.as_deref());
        return frame()
            .child(top)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .p(px(14.))
                    .child(
                        div()
                            .pt(px(1.))
                            .child(crate::spinner::ring("clone-spin", colors::text2())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(7.))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(5.))
                                    .min_w_0()
                                    .child(div().flex_none().child("Cloning into"))
                                    .child(path(into.parent().unwrap_or(into), &shown_name(into))),
                            )
                            .child(
                                div().h(px(3.)).rounded(px(2.)).bg(colors::ink(0.08)).child(
                                    div()
                                        .h_full()
                                        .rounded(px(2.))
                                        .bg(colors::text1())
                                        .w(relative(pct.map_or(0.04, |p| p as f32 / 100.))),
                                ),
                            )
                            .child(div().text_size(px(11.5)).text_color(colors::text3()).child(
                                match pct {
                                    Some(p) => format!("{label} {p}%"),
                                    None => label,
                                },
                            )),
                    ),
            )
            .into_any_element();
    }
    let list = rows.iter().enumerate().map(|(i, row)| {
        let on = i == c.pick.min(rows.len().saturating_sub(1));
        let lead = |glyph: &str| {
            icon(
                glyph,
                14.,
                if on { colors::text1() } else { colors::text3() },
            )
        };
        let (main, sub): (AnyElement, String) = match row {
            Row::Into { parent, why } => (
                div()
                    .flex()
                    .gap(px(5.))
                    .min_w_0()
                    .child(div().flex_none().child("Clone into"))
                    .child(path(parent, &name))
                    .into_any_element(),
                if i == 0 {
                    format!("{why}. Adds it as a new project.")
                } else {
                    why.clone()
                },
            ),
            Row::Elsewhere => (
                div().child("Clone somewhere else…").into_any_element(),
                "Pick a folder, then Enter".into(),
            ),
            Row::Here(cwd) => (
                div()
                    .flex()
                    .gap(px(5.))
                    .min_w_0()
                    .child(div().flex_none().child("Clone into"))
                    .child(path(cwd.parent().unwrap_or(cwd), &shown_name(cwd)))
                    .into_any_element(),
                format!(
                    "{} is empty, so it goes straight in and starts there",
                    target.unwrap_or("This project's folder")
                ),
            ),
        };
        let glyph = if matches!(row, Row::Elsewhere) {
            "folder-plus"
        } else {
            "folder"
        };
        let row = row.clone();
        row_frame(i, on)
            .child(lead(glyph))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(main)
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(colors::text3())
                            .child(sub),
                    ),
            )
            .when(on, |d| {
                d.child(icon("corner-down-left", 13., colors::text3()))
            })
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                v.clone.pick = i;
                v.run_row(row.clone(), cx);
            }))
            .into_any_element()
    });
    let error = c.error.clone().map(|e| {
        div()
            .flex()
            .items_start()
            .gap_2()
            .mx(px(14.))
            .mb(px(8.))
            .text_size(px(12.))
            .text_color(colors::error())
            .child(icon("circle-alert", 13., colors::error()))
            .child(e)
    });
    let foot = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(14.))
        .px(px(14.))
        .py(px(8.))
        .bg(colors::ink(0.025))
        .text_size(px(11.))
        .text_color(colors::text3())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(7.))
                .child("Folder name")
                .child(
                    div()
                        .min_w(px(110.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .px(px(7.))
                        .rounded(px(5.))
                        .border_1()
                        .border_color(colors::border1())
                        .bg(colors::bg())
                        .font_family(MONO)
                        .text_size(px(11.5))
                        .text_color(colors::text1())
                        .child(c.name.clone()),
                ),
        )
        .child(div().flex_1())
        .child(hint("↑↓", "choose"))
        .child(hint("Enter", "clone"))
        .child(hint("Esc", "send as text"));
    frame()
        .child(top)
        .child(div().p(px(6.)).flex().flex_col().gap(px(2.)).children(list))
        .children(error)
        .child(foot)
        .into_any_element()
}

fn shown_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| shown(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gits_progress_lines() {
        assert_eq!(progress(None), ("Connecting".into(), None));
        assert_eq!(
            progress(Some("Receiving objects:  62% (620/1000), 1.2 MiB")),
            ("Downloading".into(), Some(62))
        );
        assert_eq!(
            progress(Some("remote: Counting objects: 100% (5/5), done.")),
            ("Counting files on the server".into(), Some(100))
        );
        assert_eq!(
            progress(Some("Cloning into 'x'...")),
            ("Connecting".into(), None)
        );
        assert_eq!(
            progress(Some(r"Cloning into 'C:\w\x'...")),
            ("Connecting".into(), None)
        );
    }
}
