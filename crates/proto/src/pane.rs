// What can be on screen. The main area shows one thread at a time; a file or a diff opens over it
// in the viewer. Reasons: docs/adr/0015-one-thread-on-screen.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A thread, or a file or diff for the viewer. A sidebar row carries one while it is dragged.
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
}
