// The dock's Files tab, after the Tauri app's FilesPanel: a quiet tree that lists a folder only
// when it is opened, colors changed files by their git state (and the folders holding them), and
// marks the file the viewer has open. A click on a file opens it in the viewer.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, UniformListScrollHandle, div,
    prelude::*, px, uniform_list,
};
use hyprspace_proto::folder::{DirEntry, GitStatus};
use hyprspace_proto::{Client, Command, FolderCommand, Pane};
use hyprspace_theme::MONO;

use super::{Dock, DockEvent, empty};
use crate::assets::icon;
use crate::colors;

const ROW: f32 = 26.;
const INDENT: f32 = 12.;

/// A file's git state, as the tree shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deco {
    Modified,
    New,
    Deleted,
}

impl Deco {
    /// From a porcelain XY code, the way the Tauri app read it.
    pub fn of(code: &str) -> Self {
        if code.contains('?') || code.contains('A') {
            Deco::New
        } else if code.contains('D') {
            Deco::Deleted
        } else {
            Deco::Modified
        }
    }

    pub fn letter(self, code: &str) -> &'static str {
        match self {
            Deco::New if code.contains('?') => "U",
            Deco::New => "A",
            Deco::Deleted => "D",
            Deco::Modified if code.contains('R') => "R",
            Deco::Modified => "M",
        }
    }

    pub fn color(self) -> Hsla {
        match self {
            Deco::Modified => colors::busy(),
            Deco::New => colors::ok(),
            Deco::Deleted => colors::error(),
        }
    }
}

/// A path in one spelling, so a path git printed (forward slashes) matches one the tree built.
/// Windows paths are case-blind.
pub fn key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/');
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

#[derive(Default)]
pub struct Tree {
    root: Option<PathBuf>,
    open: HashSet<PathBuf>,
    kids: HashMap<PathBuf, Result<Vec<DirEntry>, String>>,
    /// Files with changes and the folders above them, by `key`.
    git: HashMap<String, (Deco, &'static str)>,
    dirty: HashSet<String>,
    pub viewing: Option<PathBuf>,
    scroll: UniformListScrollHandle,
}

/// One visible row of the tree.
struct Row {
    path: PathBuf,
    name: String,
    dir: bool,
    depth: usize,
    open: bool,
}

impl Tree {
    /// Forgets the last folder's listings, keeping which file the viewer has open.
    pub fn reset(&mut self) {
        *self = Tree {
            viewing: self.viewing.take(),
            ..Tree::default()
        };
    }

    pub fn load(&mut self, client: &Client, root: PathBuf) {
        self.root = Some(root.clone());
        if !self.kids.contains_key(&root) {
            ask(client, root);
        }
    }

    /// Lists the root and every open folder again.
    pub fn reload(&mut self, client: &Client) {
        let dirs: Vec<PathBuf> = self.root.iter().chain(self.open.iter()).cloned().collect();
        for d in dirs {
            ask(client, d);
        }
    }

    pub fn listed(&mut self, path: &Path, entries: &Result<Vec<DirEntry>, String>) {
        let Some(root) = &self.root else {
            return;
        };
        if path.starts_with(root) {
            self.kids.insert(path.to_path_buf(), entries.clone());
        }
    }

    pub fn decorate(&mut self, status: &GitStatus) {
        self.git.clear();
        self.dirty.clear();
        let Some(repo) = &status.root else {
            return;
        };
        for c in &status.changes {
            let full = repo.join(&c.path);
            let deco = Deco::of(&c.status);
            self.git.insert(key(&full), (deco, deco.letter(&c.status)));
            let mut up = full.parent();
            while let Some(p) = up {
                if !self.dirty.insert(key(p)) || p == repo.as_path() {
                    break;
                }
                up = p.parent();
            }
        }
    }

    fn toggle(&mut self, client: &Client, dir: PathBuf) {
        if !self.open.remove(&dir) {
            if !self.kids.contains_key(&dir) {
                ask(client, dir.clone());
            }
            self.open.insert(dir);
        }
    }

    fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            self.walk(root, 0, &mut out);
        }
        out
    }

    fn walk(&self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        let Some(Ok(entries)) = self.kids.get(dir) else {
            return;
        };
        for e in entries {
            let path = dir.join(&e.name);
            let open = e.dir && self.open.contains(&path);
            out.push(Row {
                path: path.clone(),
                name: e.name.clone(),
                dir: e.dir,
                depth,
                open,
            });
            if open {
                self.walk(&path, depth + 1, out);
            }
        }
    }

    fn row(&self, r: &Row, cx: &mut Context<Dock>) -> AnyElement {
        let k = key(&r.path);
        let git = (!r.dir).then(|| self.git.get(&k).copied()).flatten();
        let dirty = r.dir && self.dirty.contains(&k);
        let viewing = !r.dir && self.viewing.as_deref().is_some_and(|v| key(v) == k);
        let pad = r.depth as f32 * INDENT + 8.;
        let name_color = match (git, dirty, r.dir) {
            (Some((d, _)), _, _) => d.color(),
            (None, true, _) => Deco::Modified.color().opacity(0.85),
            (None, false, true) => colors::text2(),
            _ if viewing => colors::text1(),
            _ => colors::text3(),
        };
        let guides = (1..=r.depth).map(|d| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px((d - 1) as f32 * INDENT + 14.))
                .w(px(1.))
                .bg(colors::border1())
        });
        let path = r.path.clone();
        let dir = r.dir;
        div()
            .id(gpui::ElementId::Name(k.into()))
            .relative()
            .w_full()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(ROW))
            .pl(px(pad))
            .pr(px(8.))
            .rounded(px(6.))
            .text_size(px(12.5))
            .cursor_pointer()
            .when(viewing, |d| {
                d.bg(colors::accent().opacity(0.1))
                    .border_l_2()
                    .border_color(colors::accent())
            })
            .hover(|s| s.bg(colors::surface3().opacity(0.55)))
            .children(guides)
            .child(if r.dir {
                icon(
                    if r.open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    12.,
                    colors::text3(),
                )
                .into_any_element()
            } else {
                div().w(px(12.)).flex_none().into_any_element()
            })
            .when(!r.dir, |d| {
                d.child(icon("file", 13., colors::text3().opacity(0.7)))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(name_color)
                    .when(r.dir, |d| d.font_weight(FontWeight::MEDIUM))
                    .when(matches!(git, Some((Deco::Deleted, _))), |d| {
                        d.line_through()
                    })
                    .child(r.name.clone()),
            )
            .children(git.map(|(d, letter)| {
                div()
                    .flex_none()
                    .pl(px(6.))
                    .font_family(MONO)
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(d.color().opacity(0.85))
                    .child(letter)
            }))
            .on_click(cx.listener(move |d, _: &ClickEvent, _, cx| {
                if dir {
                    d.tree.toggle(&d.client, path.clone());
                    cx.notify();
                } else {
                    cx.emit(DockEvent::Open(Pane::File {
                        path: path.clone(),
                        line: None,
                        col: None,
                    }));
                }
            }))
            .into_any_element()
    }
}

fn ask(client: &Client, path: PathBuf) {
    client.send(Command::Folder(FolderCommand::ListDir { path }));
}

impl Dock {
    pub(super) fn files(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(root) = self.folder.clone() else {
            return empty("No folder yet.");
        };
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.display().to_string());
        let rows = self.tree.rows();
        let body = match self.tree.kids.get(&root) {
            None => empty("Loading"),
            Some(Err(e)) => div()
                .px(px(14.))
                .py(px(12.))
                .text_size(px(12.))
                .text_color(colors::error())
                .child(e.clone())
                .into_any_element(),
            Some(Ok(list)) if list.is_empty() => empty("Empty folder."),
            Some(Ok(_)) => uniform_list(
                "tree",
                rows.len(),
                cx.processor(|d, range: Range<usize>, _, cx| {
                    let rows = d.tree.rows();
                    range
                        .filter_map(|i| rows.get(i).map(|r| d.tree.row(r, cx)))
                        .collect()
                }),
            )
            .flex_1()
            .px(px(6.))
            .pb(px(10.))
            .track_scroll(&self.tree.scroll)
            .into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(36.))
                    .pl(px(14.))
                    .pr(px(8.))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(name),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(colors::text3())
                            .child(crate::workbench::short(&root)),
                    )
                    .child(
                        div()
                            .id("files-refresh")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(24.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(colors::surface3()))
                            .child(icon("refresh-cw", 13., colors::text3()))
                            .on_click(cx.listener(|d, _: &ClickEvent, _, cx| d.refresh(cx))),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::git::FileChange;

    fn change(path: &str, status: &str) -> FileChange {
        FileChange {
            path: path.into(),
            status: status.into(),
            added: 0,
            removed: 0,
        }
    }

    #[test]
    fn codes_read_like_the_tauri_app() {
        assert_eq!(Deco::of("??").letter("??"), "U");
        assert_eq!(Deco::of("A ").letter("A "), "A");
        assert_eq!(Deco::of(" D"), Deco::Deleted);
        assert_eq!(Deco::of("R ").letter("R "), "R");
        assert_eq!(Deco::of(" M").letter(" M"), "M");
    }

    #[test]
    fn git_paths_match_tree_paths_and_mark_their_folders() {
        let repo = std::env::temp_dir().join("repo");
        let mut t = Tree::default();
        t.decorate(&GitStatus {
            root: Some(PathBuf::from(repo.to_string_lossy().replace('\\', "/"))),
            changes: vec![change("src/ui/a.rs", " M"), change("new.txt", "??")],
            ..Default::default()
        });
        let a = repo.join("src").join("ui").join("a.rs");
        assert_eq!(t.git.get(&key(&a)).map(|g| g.0), Some(Deco::Modified));
        assert!(t.dirty.contains(&key(&repo.join("src"))));
        assert!(t.dirty.contains(&key(&repo.join("src").join("ui"))));
        assert!(!t.dirty.contains(&key(&repo.join("docs"))));
    }

    #[test]
    fn rows_walk_open_folders_only() {
        let root = PathBuf::from("/w");
        let mut t = Tree {
            root: Some(root.clone()),
            ..Default::default()
        };
        let entry = |n: &str, dir| DirEntry {
            name: n.into(),
            dir,
        };
        t.kids.insert(
            root.clone(),
            Ok(vec![entry("src", true), entry("a.rs", false)]),
        );
        t.kids
            .insert(root.join("src"), Ok(vec![entry("main.rs", false)]));
        assert_eq!(t.rows().len(), 2);
        t.open.insert(root.join("src"));
        let rows = t.rows();
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[1].name.as_str(), rows[1].depth), ("main.rs", 1));
    }
}
