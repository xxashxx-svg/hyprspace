// The viewer card: a file or a diff over the window, on a dimmed backdrop. Its header names the
// file and its folder, marks unsaved edits, and carries the diff's line counts, a button to view
// the whole file from a diff, Open in browser for a page, one to open it in the user's editor, and
// close. Esc or a click outside closes it too, asking first about unsaved edits.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, KeyDownEvent, MouseButton, Window,
    anchored, deferred, div, point, prelude::*,
};
use hyprspace_proto::{Command, Pane};
use hyprspace_theme::MONO;

use super::header::{head_button, short};
use crate::assets::icon;
use crate::colors;
use crate::root::Root;
use crate::widgets::tip;
use crate::zoom::px;

/// The card's margin from the window's edges, and its widest.
const MARGIN: f32 = 24.;
const MAX_W: f32 = 1500.;

impl Root {
    /// The file a pane shows, with the line to open it at: a diff's file sits under its repo.
    fn viewed_file(
        &self,
        pane: &Pane,
        cx: &Context<Self>,
    ) -> Option<(PathBuf, Option<u32>, Option<u32>)> {
        match pane {
            Pane::File { path, line, col } => Some((path.clone(), *line, *col)),
            Pane::Diff { cwd, path } | Pane::Changes { cwd, path, .. } if !path.is_empty() => {
                let root = self
                    .work
                    .viewer
                    .as_ref()
                    .and_then(|v| v.read(cx).root().map(Path::to_path_buf))
                    .unwrap_or_else(|| cwd.clone());
                Some((root.join(path), None, None))
            }
            Pane::Diff { .. } | Pane::Changes { .. } | Pane::Thread { .. } => None,
        }
    }

    /// A file's name, and its folder relative to the space's.
    fn file_title(&self, space: u64, file: &Path) -> (String, String) {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = self.state.space(space).and_then(|s| s.cwd.clone());
        let dir = file.parent().map(|p| match &base {
            Some(b) => p
                .strip_prefix(b)
                .map(|r| r.display().to_string())
                .unwrap_or_else(|_| short(p)),
            None => short(p),
        });
        (name, dir.unwrap_or_default().replace('\\', "/"))
    }

    /// The viewer card, while a file or a diff is open.
    pub(crate) fn viewer_card(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let viewing = self.work.viewing.as_ref()?;
        let viewer = self.work.viewer.clone()?;
        let (space, pane) = (viewing.space, viewing.pane.clone());
        let file = self.viewed_file(&pane, cx);
        let (name, dir) = match (&file, &pane) {
            (Some((path, ..)), _) => self.file_title(space, path),
            (None, Pane::Changes { cwd, .. }) => ("Changes".to_string(), short(cwd)),
            (None, _) => return None,
        };
        let diff = matches!(pane, Pane::Diff { .. } | Pane::Changes { .. });
        let counts = diff.then(|| viewer.read(cx).counts()).flatten();
        let unsaved = viewer.read(cx).dirty(cx);
        let size = window.viewport_size();
        let z = crate::zoom::zoom();
        let (vw, vh) = (f32::from(size.width) / z, f32::from(size.height) / z);

        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some((add, del)) = counts {
            actions.push(
                div()
                    .id("card-counts")
                    .tooltip(tip("Lines added and removed"))
                    .flex_none()
                    .flex()
                    .gap(px(6.))
                    .mr(px(4.))
                    .font_family(MONO)
                    .text_size(px(11.))
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_color(colors::diff_add())
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_color(colors::diff_del())
                                .child(format!("-{del}")),
                        )
                    })
                    .into_any_element(),
            );
        }
        if let Some((path, line, col)) = file {
            if diff {
                let open = path.clone();
                actions.push(
                    head_button(("card-view-file", 0), "file-code", false)
                        .tooltip(tip("View the whole file"))
                        .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                            r.open_path(open.clone(), None, None, window, cx)
                        }))
                        .into_any_element(),
                );
            }
            if !diff && page(&path) {
                let url = file_url(&path);
                actions.push(
                    crate::widgets::button_frame("card-browser")
                        .h(px(24.))
                        .px(px(9.))
                        .mr(px(2.))
                        .text_size(px(12.))
                        .child(icon("globe", 12., colors::text2()))
                        .child("Open in browser")
                        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
                        .into_any_element(),
                );
            }
            let client = self.client.clone();
            actions.push(
                head_button(("card-editor", 0), "external-link", false)
                    .tooltip(tip("Open in your editor"))
                    .on_click(move |_, _, _| {
                        client.send(Command::OpenFile {
                            path: path.clone(),
                            line,
                            col,
                        })
                    })
                    .into_any_element(),
            );
        }
        actions.push(
            head_button(("card-close", 0), "x", true)
                .tooltip(tip("Close"))
                .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.close_viewer(window, cx)))
                .into_any_element(),
        );

        let header = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(46.))
            .pl(px(18.))
            .pr(px(10.))
            .child(if diff {
                icon("file-diff", 13., colors::text3())
            } else {
                let (glyph, tint) = crate::dock::kinds::file_icon(&name);
                icon(glyph, 13., tint)
            })
            .child(
                div()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(name),
            )
            // unsaved edits, as editors mark a tab
            .when(unsaved, |d| {
                d.child(
                    div()
                        .id("card-unsaved")
                        .tooltip(tip(if cfg!(target_os = "macos") {
                            "Unsaved changes. Cmd+S saves them."
                        } else {
                            "Unsaved changes. Ctrl+S saves them."
                        }))
                        .flex_none()
                        .size(px(7.))
                        .rounded_full()
                        .bg(colors::text2()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(8.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(dir),
            )
            .children(actions);
        let card = div()
            .id("viewer-card")
            .w(px((vw - 2. * MARGIN).min(MAX_W)))
            .h(px(vh - 2. * MARGIN))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(14.))
            .border_1()
            .border_color(colors::ink(0.1))
            .bg(colors::bg())
            .shadow(colors::shadow())
            // clicks inside stay inside; only the backdrop closes it
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(div().flex_1().min_h_0().child(viewer));
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("viewer-scrim")
                        .w(size.width)
                        .h(size.height)
                        .occlude()
                        .bg(colors::bg().opacity(0.64))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|r, _, window, cx| r.close_viewer(window, cx)),
                        )
                        .on_key_down(cx.listener(|r, e: &KeyDownEvent, window, cx| {
                            if e.keystroke.key == "escape" {
                                r.close_viewer(window, cx);
                            }
                        }))
                        .child(crate::slide::rise_in(card, "viewer-card-in"))
                        .map(|scrim| {
                            crate::slide::ease_in(scrim, "viewer-scrim-in", 140, |d, t| {
                                d.opacity(t)
                            })
                        }),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

fn page(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

fn file_url(path: &Path) -> String {
    let p = path.to_string_lossy().replace('\\', "/");
    let p: String = p
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '?' => "%3F".to_string(),
            c => c.to_string(),
        })
        .collect();
    if p.starts_with('/') {
        format!("file://{p}")
    } else {
        format!("file:///{p}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_opens_by_its_file_url() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\Ash\My Site\index.html")),
            "file:///C:/Users/Ash/My%20Site/index.html"
        );
        assert_eq!(
            file_url(Path::new("/Users/ash/a#b/index.html")),
            "file:///Users/ash/a%23b/index.html"
        );
        assert!(page(Path::new("x/INDEX.HTM")));
        assert!(!page(Path::new("x/index.md")));
    }
}
