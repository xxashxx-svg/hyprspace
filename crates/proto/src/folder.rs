// Requests about a folder on disk: the dock's file tree and git tab, the file viewer, and opening
// the folder in another app. They ride the channel as one `Command::Folder` and one
// `Event::Folder`, each answer naming the path it is about so it lands on the view that asked.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::git::{BranchInfo, FileChange};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum FolderCommand {
    /// One level of a folder. Answered with `Dir`.
    ListDir {
        path: PathBuf,
    },
    /// A text file for the viewer. Answered with `File`.
    ReadFile {
        path: PathBuf,
    },
    /// The branch and changed files of the repo holding `cwd`. Answered with `Git`.
    GitStatus {
        cwd: PathBuf,
    },
    /// Stage or unstage one file, or every file when `path` is empty. Answered with `GitDone`.
    Stage {
        cwd: PathBuf,
        path: String,
        stage: bool,
    },
    /// Commit what is staged, then push when asked. Answered with `GitDone`.
    Commit {
        cwd: PathBuf,
        message: String,
        push: bool,
    },
    Push {
        cwd: PathBuf,
    },
    /// One file's working tree diff, `path` relative to the repo. Answered with `Diff`.
    Diff {
        cwd: PathBuf,
        path: String,
    },
    /// Which apps can open a folder here. Answered with `Openers`.
    Openers,
    /// Open `path`, a folder, in `opener`. Answered only when that fails.
    OpenIn {
        opener: Opener,
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum FolderEvent {
    Dir {
        path: PathBuf,
        entries: Result<Vec<DirEntry>, String>,
    },
    File {
        path: PathBuf,
        text: Result<String, String>,
    },
    Git {
        cwd: PathBuf,
        status: GitStatus,
    },
    /// A stage, commit or push ended: a line for the UI, or git's reason. A fresh `Git` for the
    /// same `cwd` follows either way.
    GitDone {
        cwd: PathBuf,
        result: Result<String, String>,
    },
    Diff {
        cwd: PathBuf,
        path: String,
        text: Result<String, String>,
    },
    Openers {
        openers: Vec<Opener>,
    },
    OpenFailed {
        message: String,
    },
}

/// One entry of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub dir: bool,
}

/// A working tree as the git tab shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// The repo's top folder. The change paths are relative to it. None outside a repo.
    pub root: Option<PathBuf>,
    pub branch: BranchInfo,
    /// Sorted by path, so a row stays put when its file is staged.
    pub changes: Vec<FileChange>,
}

impl FileChange {
    /// Something of this file is staged (git's X column).
    pub fn staged(&self) -> bool {
        !matches!(self.status.as_bytes().first(), Some(b' ' | b'?') | None)
    }
}

/// An app that opens a folder: a code editor, or the system's file manager.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Opener {
    #[default]
    VsCode,
    Cursor,
    /// Explorer on Windows, Finder on macOS.
    Files,
}

impl Opener {
    pub const ALL: [Opener; 3] = [Opener::VsCode, Opener::Cursor, Opener::Files];

    pub fn name(self) -> &'static str {
        match self {
            Opener::VsCode => "VS Code",
            Opener::Cursor => "Cursor",
            Opener::Files if cfg!(target_os = "macos") => "Finder",
            Opener::Files => "Explorer",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_reads_the_index_column() {
        let c = |s: &str| FileChange {
            path: "a".into(),
            status: s.into(),
            added: 0,
            removed: 0,
        };
        assert!(c("M ").staged() && c("A ").staged() && c("MM").staged());
        assert!(!c(" M").staged() && !c("??").staged());
    }

    #[test]
    fn names_are_plain() {
        assert_eq!(Opener::VsCode.name(), "VS Code");
        assert!(["Explorer", "Finder"].contains(&Opener::Files.name()));
    }
}
