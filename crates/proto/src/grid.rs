// How a space tiles its panes: what is on screen, in which arrangement, and how far each
// boundary was dragged. The UI owns the shape and saves it with the space, so a space opens the
// way it was left. Reasons: docs/adr/0007-panes-dock-and-viewer.md.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grid {
    /// In layout order: the first pane takes the layout's first cell.
    pub panes: Vec<Pane>,
    /// The pane that takes keys, and the one a thread opened from the sidebar replaces.
    pub focus: Option<Pane>,
    /// The pane filling the whole space, while one is maximized.
    pub maximized: Option<Pane>,
    /// The layout picked for each pane count, by preset id. A count with no pick uses its first
    /// preset.
    pub layouts: BTreeMap<usize, String>,
    /// Dragged track sizes per layout, keyed `"<count>:<preset>"`.
    pub tracks: BTreeMap<String, Tracks>,
}

/// What one pane shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Pane {
    Thread {
        id: u64,
    },
    /// A file in the read-only viewer, scrolled to `line` (1-based) when there is one.
    File {
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
    /// One file's working tree diff. `path` is relative to the repo containing `cwd`.
    Diff {
        cwd: PathBuf,
        path: String,
    },
}

impl Pane {
    pub fn thread(&self) -> Option<u64> {
        match self {
            Pane::Thread { id } => Some(*id),
            _ => None,
        }
    }

    /// A file or a diff. A space shows at most one, and opening another replaces it.
    pub fn is_viewer(&self) -> bool {
        !matches!(self, Pane::Thread { .. })
    }
}

/// Relative sizes of a layout's columns and rows. Empty means equal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tracks {
    pub cols: Vec<f32>,
    pub rows: Vec<f32>,
}

impl Grid {
    pub fn has(&self, pane: &Pane) -> bool {
        self.panes.contains(pane)
    }

    /// Takes a pane out, along with its focus and maximize. Focus moves to a neighbour.
    pub fn remove(&mut self, pane: &Pane) {
        let Some(ix) = self.panes.iter().position(|p| p == pane) else {
            return;
        };
        self.panes.remove(ix);
        if self.maximized.as_ref() == Some(pane) {
            self.maximized = None;
        }
        if self.focus.as_ref() == Some(pane) || self.focus.is_none() {
            self.focus = self
                .panes
                .get(ix.min(self.panes.len().saturating_sub(1)))
                .cloned();
        }
    }

    /// Puts `new` where `old` was. Adds it at the end when `old` is not on screen.
    pub fn replace(&mut self, old: &Pane, new: Pane) {
        self.panes.retain(|p| p != &new);
        match self.panes.iter().position(|p| p == old) {
            Some(ix) => self.panes[ix] = new.clone(),
            None => self.panes.push(new.clone()),
        }
        if self.maximized.as_ref() == Some(old) {
            self.maximized = Some(new.clone());
        }
        self.focus = Some(new);
    }

    /// Trades the places of two panes.
    pub fn swap(&mut self, a: &Pane, b: &Pane) {
        let ia = self.panes.iter().position(|p| p == a);
        let ib = self.panes.iter().position(|p| p == b);
        if let (Some(ia), Some(ib)) = (ia, ib) {
            self.panes.swap(ia, ib);
        }
    }

    /// Opens a file or diff: in place of the viewer pane already on screen, else as a new pane.
    pub fn show_viewer(&mut self, pane: Pane) {
        match self.panes.iter().find(|p| p.is_viewer()).cloned() {
            Some(old) => self.replace(&old, pane),
            None => {
                self.panes.push(pane.clone());
                self.maximized = None;
                self.focus = Some(pane);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: u64) -> Pane {
        Pane::Thread { id }
    }

    fn file(p: &str) -> Pane {
        Pane::File {
            path: p.into(),
            line: None,
            col: None,
        }
    }

    #[test]
    fn removing_the_focused_pane_focuses_a_neighbour() {
        let mut g = Grid {
            panes: vec![t(1), t(2), t(3)],
            focus: Some(t(3)),
            maximized: Some(t(3)),
            ..Default::default()
        };
        g.remove(&t(3));
        assert_eq!(g.panes, [t(1), t(2)]);
        assert_eq!(g.focus, Some(t(2)));
        assert_eq!(g.maximized, None);
        g.remove(&t(1));
        g.remove(&t(2));
        assert_eq!(g.focus, None);
    }

    #[test]
    fn replace_keeps_the_place_and_never_doubles_a_pane() {
        let mut g = Grid {
            panes: vec![t(1), t(2)],
            ..Default::default()
        };
        g.replace(&t(1), t(5));
        assert_eq!(g.panes, [t(5), t(2)]);
        g.replace(&t(5), t(2));
        assert_eq!(g.panes, [t(2)]);
        g.replace(&t(9), t(7));
        assert_eq!(g.panes, [t(2), t(7)]);
        assert_eq!(g.focus, Some(t(7)));
        g.swap(&t(2), &t(7));
        assert_eq!(g.panes, [t(7), t(2)]);
    }

    #[test]
    fn one_viewer_pane_per_space() {
        let mut g = Grid {
            panes: vec![t(1)],
            ..Default::default()
        };
        g.show_viewer(file("a.rs"));
        g.show_viewer(Pane::Diff {
            cwd: "/w".into(),
            path: "b.rs".into(),
        });
        assert_eq!(g.panes.len(), 2);
        assert!(g.panes[1].is_viewer());
        assert_eq!(g.focus.as_ref(), Some(&g.panes[1]));
    }

    #[test]
    fn grids_survive_json_with_number_keys() {
        let mut g = Grid {
            panes: vec![t(1), file("/w/a.rs")],
            focus: Some(t(1)),
            ..Default::default()
        };
        g.layouts.insert(2, "rows".into());
        g.tracks.insert(
            "2:rows".into(),
            Tracks {
                cols: vec![],
                rows: vec![1.0, 1.5],
            },
        );
        let back: Grid = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(back, g);
    }
}
