// Folder requests from the dock, the viewer and the Open button. Each one reads the disk or runs
// git, so it goes to the blocking pool and answers with its own event. Git calls take one lock:
// two ticks in quick succession would otherwise race for git's index.lock and one would fail.

use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::Event;
use hyprspace_proto::folder::{DirEntry, FolderCommand, FolderEvent, GitStatus};
use hyprspace_proto::git::FileOp;

use crate::{git, open};

/// The viewer reads text up to this size; past it the file goes to an editor.
const MAX_FILE: u64 = 2_000_000;

#[derive(Clone, Default)]
pub struct Folders {
    git: Arc<Mutex<()>>,
}

impl Folders {
    pub fn handle(&self, cmd: FolderCommand, tx: UnboundedSender<Event>) {
        let lock = self.git.clone();
        tokio::task::spawn_blocking(move || {
            for event in run(cmd, &lock) {
                let _ = tx.unbounded_send(Event::Folder(event));
            }
        });
    }
}

fn run(cmd: FolderCommand, lock: &Mutex<()>) -> Vec<FolderEvent> {
    let git_lock = || lock.lock().unwrap_or_else(|e| e.into_inner());
    match cmd {
        FolderCommand::ListDir { path } => {
            let entries = list_dir(&path);
            vec![FolderEvent::Dir { path, entries }]
        }
        FolderCommand::ReadFile { path } => {
            let text = read_file(&path);
            vec![FolderEvent::File { path, text }]
        }
        FolderCommand::GitStatus { cwd } => {
            let status = {
                let _g = git_lock();
                status(&cwd)
            };
            vec![FolderEvent::Git { cwd, status }]
        }
        FolderCommand::Stage { cwd, path, stage } => {
            let op = if stage {
                FileOp::Stage
            } else {
                FileOp::Unstage
            };
            let _g = git_lock();
            // the path is relative to the repo root, like the diff's
            let result = git::root(&cwd)
                .ok_or_else(|| "This folder is not a git repository.".to_string())
                .and_then(|root| git::file_op(&root, op, &path))
                .map(|_| String::new());
            done(cwd, result)
        }
        FolderCommand::Commit { cwd, message, push } => {
            let _g = git_lock();
            let result = git::commit(&cwd, &message, push, false);
            done(cwd, result)
        }
        FolderCommand::Push { cwd } => {
            let _g = git_lock();
            let result = git::push(&cwd);
            done(cwd, result)
        }
        FolderCommand::Diff { cwd, path } => {
            let text = {
                let _g = git_lock();
                diff(&cwd, &path)
            };
            vec![FolderEvent::Diff { cwd, path, text }]
        }
        FolderCommand::Openers => vec![FolderEvent::Openers {
            openers: open::openers(),
        }],
        FolderCommand::OpenIn { opener, path } => match open::open_in(opener, &path) {
            Ok(()) => vec![],
            Err(message) => vec![FolderEvent::OpenFailed { message }],
        },
    }
}

/// The answer to a git change, then the tree as it now stands. Called with the git lock held.
fn done(cwd: std::path::PathBuf, result: git::Result<String>) -> Vec<FolderEvent> {
    let status = status(&cwd);
    vec![
        FolderEvent::GitDone {
            cwd: cwd.clone(),
            result,
        },
        FolderEvent::Git { cwd, status },
    ]
}

fn status(cwd: &Path) -> GitStatus {
    let Some(root) = git::root(cwd) else {
        return GitStatus::default();
    };
    let mut changes = git::changes(cwd).unwrap_or_default();
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    GitStatus {
        root: Some(root),
        branch: git::branch_info(cwd),
        changes,
    }
}

/// `path` is relative to the repo root (it came from `status`), but git resolves paths after
/// `--` against the folder it runs in, so run it at the root.
fn diff(cwd: &Path, path: &str) -> Result<String, String> {
    let root = git::root(cwd).ok_or("This folder is not a git repository.")?;
    git::diff(&root, path)
}

/// One level of a folder: folders first, then files, each by name ignoring case. `.git` is left
/// out; nobody browses it from a file tree.
fn list_dir(path: &Path) -> Result<Vec<DirEntry>, String> {
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut out: Vec<(String, DirEntry)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name == ".git" {
                return None;
            }
            // file_type comes with the entry; following a link to a folder needs a stat
            let dir = e.file_type().is_ok_and(|t| t.is_dir())
                || (e.file_type().is_ok_and(|t| t.is_symlink()) && e.path().is_dir());
            Some((name.to_lowercase(), DirEntry { name, dir }))
        })
        .collect();
    out.sort_by(|(ka, a), (kb, b)| b.dir.cmp(&a.dir).then_with(|| ka.cmp(kb)));
    Ok(out.into_iter().map(|(_, e)| e).collect())
}

/// A text file, refusing what the viewer can't show: big files and binaries (a NUL byte).
fn read_file(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Err("That is a folder.".into());
    }
    if meta.len() > MAX_FILE {
        return Err("This file is over 2 MB, too big to show here.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.contains(&0) {
        return Err("This looks like a binary file.".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testing;

    fn run1(cmd: FolderCommand) -> Vec<FolderEvent> {
        run(cmd, &Mutex::new(()))
    }

    #[test]
    fn lists_folders_first_without_git() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join("zeta")).unwrap();
        std::fs::write(dir.path().join("B.txt"), "").unwrap();
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        let names: Vec<_> = list_dir(dir.path())
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.dir))
            .collect();
        assert_eq!(
            names,
            [
                ("zeta".to_string(), true),
                ("a.txt".to_string(), false),
                ("B.txt".to_string(), false)
            ]
        );
        assert!(list_dir(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn reads_text_and_refuses_binaries() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("a.rs");
        let bin = dir.path().join("a.bin");
        std::fs::write(&text, "fn main() {}\n").unwrap();
        std::fs::write(&bin, [1u8, 0, 2]).unwrap();
        assert_eq!(read_file(&text).unwrap(), "fn main() {}\n");
        assert!(read_file(&bin).unwrap_err().contains("binary"));
        assert!(read_file(dir.path()).is_err());
    }

    #[test]
    fn stages_commits_and_pushes_to_a_local_remote() {
        let dir = tempfile::tempdir().unwrap();
        let (remote, cwd) = (dir.path().join("remote.git"), dir.path().join("work"));
        std::fs::create_dir_all(&cwd).unwrap();
        testing::bare(&remote);
        testing::repo(&cwd);
        std::fs::create_dir(cwd.join("src")).unwrap();
        std::fs::write(cwd.join("src/a.rs"), "one\n").unwrap();
        testing::commit_all(&cwd, "first");
        testing::push_upstream(&cwd, &remote);

        // a change in a subfolder, asked about from that subfolder
        let sub = cwd.join("src");
        std::fs::write(sub.join("a.rs"), "one\ntwo\n").unwrap();
        let [FolderEvent::Git { status, .. }] =
            &run1(FolderCommand::GitStatus { cwd: sub.clone() })[..]
        else {
            panic!("no status")
        };
        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].path, "src/a.rs");
        assert!(!status.changes[0].staged());
        assert_eq!(status.branch.ahead, 0);

        let [FolderEvent::Diff { text, .. }] = &run1(FolderCommand::Diff {
            cwd: sub.clone(),
            path: "src/a.rs".into(),
        })[..] else {
            panic!("no diff")
        };
        assert!(text.as_ref().unwrap().contains("+two"));

        let out = run1(FolderCommand::Stage {
            cwd: sub.clone(),
            path: "src/a.rs".into(),
            stage: true,
        });
        assert!(
            matches!(&out[0], FolderEvent::GitDone { result: Ok(_), .. }),
            "{out:?}"
        );
        let FolderEvent::Git { status, .. } = &out[1] else {
            panic!("no status after staging")
        };
        assert!(status.changes[0].staged());

        let out = run1(FolderCommand::Commit {
            cwd: sub.clone(),
            message: "Add two\n\nThe second line.".into(),
            push: false,
        });
        assert_eq!(
            out[0],
            FolderEvent::GitDone {
                cwd: sub.clone(),
                result: Ok("Changes committed.".into())
            }
        );
        let FolderEvent::Git { status, .. } = &out[1] else {
            panic!("no status after commit")
        };
        assert!(status.changes.is_empty());
        assert_eq!(status.branch.ahead, 1);

        let out = run1(FolderCommand::Push { cwd: sub.clone() });
        assert!(
            matches!(&out[0], FolderEvent::GitDone { result: Ok(_), .. }),
            "{out:?}"
        );
        let FolderEvent::Git { status, .. } = &out[1] else {
            panic!("no status after push")
        };
        assert_eq!(status.branch.ahead, 0);
        let log = std::process::Command::new("git")
            .arg("-C")
            .arg(&remote)
            .args(["log", "-1", "--format=%s"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), "Add two");
    }

    #[test]
    fn outside_a_repo_the_status_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(status(dir.path()), GitStatus::default());
        assert!(diff(dir.path(), "a").is_err());
    }
}
