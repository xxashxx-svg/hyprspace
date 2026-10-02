// The clone card, shown under the composer when its text starts with a repository link: where
// the clone goes, where it opens, and git's progress while it runs. Carried over from the
// Tauri app's ClonePanel.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, IntoElement, div, prelude::*, px, relative,
};
use hyprspace_theme::MONO;

use super::Composer;
use super::repo::RepoRef;
use crate::assets::icon;
use crate::colors;
use crate::input::TextInput;

pub struct CloneCard {
    /// The link the card was set up for; a different one resets it.
    pub url: String,
    pub parent: PathBuf,
    pub name: Entity<TextInput>,
    /// Straight into `parent` rather than a new folder inside it.
    pub here: bool,
    /// Start the thread in the current space rather than a new one.
    pub open_here: bool,
    pub request: Option<u64>,
    pub line: Option<String>,
    pub error: Option<String>,
    /// Whether `parent` has nothing in it; None while it doesn't exist (the clone makes it).
    pub parent_empty: Option<bool>,
}

impl CloneCard {
    pub fn busy(&self) -> bool {
        self.request.is_some()
    }

    pub fn set_parent(&mut self, parent: PathBuf) {
        self.parent_empty = std::fs::read_dir(&parent)
            .ok()
            .map(|mut d| d.next().is_none());
        if self.parent_empty == Some(false) {
            self.here = false;
        }
        self.parent = parent;
    }
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
        let label = if line.starts_with("Cloning into") {
            "Connecting"
        } else {
            "Working"
        };
        return (label.into(), None);
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

fn base(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

/// One button of a segmented pick (clone-seg in composer.css).
fn seg(
    id: &'static str,
    label: impl Into<gpui::SharedString>,
    on: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(10.))
        .rounded(px(6.))
        .text_size(px(12.))
        .when(on, |d| d.bg(colors::surface3()).text_color(colors::text1()))
        .when(!on && enabled, |d| d.text_color(colors::text2()))
        .when(!on && !enabled, |d| d.text_color(colors::text3()))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|s| s.text_color(colors::text1()))
        })
        .child(div().truncate().child(label.into()))
}

fn segs(children: impl IntoIterator<Item = AnyElement>) -> gpui::Div {
    div()
        .flex()
        .gap(px(3.))
        .p(px(3.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.04))
        .children(children)
}

pub fn render(
    c: &CloneCard,
    repo: &RepoRef,
    agent: &str,
    can_open_here: bool,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let busy = c.busy();
    let folder = if c.here {
        base(&c.parent)
    } else {
        c.name.read(cx).text().trim().to_string()
    };
    let shown = repo
        .url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("git@")
        .trim_end_matches(".git")
        .to_string();
    let parent_label = if c.parent.as_os_str().is_empty() {
        "Pick a folder".to_string()
    } else {
        c.parent.display().to_string()
    };
    let into_new = seg("clone-new", "New folder inside", !c.here, !busy)
        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
            v.clone.here = false;
            cx.notify();
        }))
        .into_any_element();
    let can_here = !busy && c.parent_empty != Some(false);
    let into_here = seg(
        "clone-here",
        if c.parent_empty == Some(false) {
            format!("Straight into {} (not empty)", base(&c.parent))
        } else {
            format!("Straight into {}", base(&c.parent))
        },
        c.here,
        can_here,
    )
    .when(can_here, |d| {
        d.on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
            v.clone.here = true;
            cx.notify();
        }))
    })
    .into_any_element();
    // not a mode, just a shortcut to the picker: the two choices beside it then apply to it
    let other = div()
        .id("clone-other")
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(10.))
        .rounded(px(6.))
        .text_size(px(12.))
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text1()))
        .child(icon("folder-open", 13., colors::text3()))
        .child("Other folder")
        .on_click(
            cx.listener(|v, _: &ClickEvent, _, cx| v.pick_folder(super::PickFor::CloneParent, cx)),
        )
        .into_any_element();
    let open_new = seg("clone-open-new", "A new space", !c.open_here, !busy)
        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
            v.clone.open_here = false;
            cx.notify();
        }))
        .into_any_element();
    let open_here = seg(
        "clone-open-here",
        "This space",
        c.open_here,
        !busy && can_open_here,
    )
    .when(can_open_here, |d| {
        d.on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
            v.clone.open_here = true;
            cx.notify();
        }))
    })
    .into_any_element();
    let folder_shown = if folder.is_empty() {
        "the clone".to_string()
    } else {
        folder
    };
    let says = if c.open_here {
        format!("{agent} starts in this space, working in {folder_shown}.")
    } else {
        format!("Adds {folder_shown} to the sidebar as its own space.")
    };
    let path = div()
        .flex()
        .items_center()
        .max_w_full()
        .h(px(30.))
        .pr(px(4.))
        .rounded(px(7.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::bg())
        .font_family(MONO)
        .text_size(px(12.))
        .child(
            div()
                .id("clone-parent")
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .h_full()
                .px(px(8.))
                .border_r_1()
                .border_color(colors::border1())
                .text_color(colors::text2())
                .cursor_pointer()
                .hover(|s| s.text_color(colors::text1()))
                .child(icon("folder-open", 13., colors::text3()))
                .child(div().truncate().child(parent_label))
                .on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
                    v.pick_folder(super::PickFor::CloneParent, cx)
                })),
        )
        .when(!c.here, |d| {
            d.child(
                div()
                    .min_w(px(120.))
                    .px(px(8.))
                    .text_color(colors::text1())
                    .child(c.name.clone()),
            )
        });
    let row = |label: &'static str, body: AnyElement| {
        div()
            .flex()
            .gap(px(12.))
            .child(
                div()
                    .w(px(56.))
                    .flex_none()
                    .pt(px(7.))
                    .text_color(colors::text3())
                    .child(label),
            )
            .child(div().flex_1().min_w_0().child(body))
    };
    let (label, pct) = progress(c.line.as_deref());
    let status = if let Some(e) = &c.error {
        Some(
            div()
                .flex()
                .items_start()
                .gap_2()
                .text_size(px(12.))
                .text_color(colors::error())
                .child(icon("circle-alert", 13., colors::error()))
                .child(e.clone())
                .into_any_element(),
        )
    } else if busy {
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(11.5))
                .text_color(colors::text2())
                .child(
                    div()
                        .w(relative(0.38))
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(colors::ink(0.08))
                        .child(
                            div()
                                .h_full()
                                .rounded(px(2.))
                                .bg(colors::accent())
                                .w(relative(pct.map_or(0.35, |p| p as f32 / 100.0))),
                        ),
                )
                .child(match pct {
                    Some(p) => format!("{label} {p}%"),
                    None => label,
                })
                .into_any_element(),
        )
    } else {
        None
    };
    let go = div()
        .id("clone-go")
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .h(px(30.))
        .pl(px(12.))
        .pr(px(if busy { 12. } else { 6. }))
        .rounded(px(7.))
        .bg(colors::accent())
        .text_color(colors::on_accent())
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .when(busy, |d| d.opacity(0.55))
        .when(!busy, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(colors::accent_hover()))
                .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.start_clone(cx)))
        })
        .child(if busy { "Cloning" } else { "Clone" })
        .when(!busy, |d| {
            d.child(
                div()
                    .px(px(5.))
                    .py(px(1.))
                    .rounded(px(4.))
                    .bg(colors::ink(0.18))
                    .font_family(MONO)
                    .text_size(px(10.))
                    .child("Enter"),
            )
        });
    div()
        .mt(px(10.))
        .w_full()
        .flex()
        .flex_col()
        .gap(px(10.))
        .pt(px(12.))
        .px(px(14.))
        .pb(px(11.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2().opacity(0.85))
        .text_size(px(12.5))
        .text_color(colors::text2())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(30.))
                        .rounded(px(8.))
                        .bg(colors::surface3())
                        .child(icon("git-branch", 15., colors::text1())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child(repo.label.clone()),
                        )
                        .child(
                            div()
                                .truncate()
                                .font_family(MONO)
                                .text_size(px(11.))
                                .text_color(colors::text3())
                                .child(shown),
                        ),
                )
                .child(go),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .pt(px(10.))
                .border_t_1()
                .border_color(colors::border0())
                .child(row(
                    "Folder",
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(segs([into_new, into_here, other]))
                        .child(path)
                        .into_any_element(),
                ))
                .child(row(
                    "Open in",
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(segs([open_new, open_here]))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child(says),
                        )
                        .into_any_element(),
                )),
        )
        .children(status)
        .into_any_element()
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
