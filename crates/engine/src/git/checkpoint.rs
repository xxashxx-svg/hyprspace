use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use hyprspace_proto::run::{ChangeKind, RunFile};

use super::{Result, git, git_cmd};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) fn top(cwd: &Path) -> Option<PathBuf> {
    let root = git(cwd, &["rev-parse", "--show-toplevel"]).ok()?;
    Some(PathBuf::from(root.trim()))
}

fn indexed(root: &Path, index: &Path, args: &[&str]) -> Option<String> {
    let out = git_cmd()
        .env("GIT_INDEX_FILE", index)
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The folder's files as a git tree, ignored files left out. Written through a scratch index so
/// the repo's own index, branch and stash never change.
pub fn snapshot(cwd: &Path) -> Option<String> {
    let root = top(cwd)?;
    let index = std::env::temp_dir().join(format!(
        "hyprspace-index-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // starting from the real index lets git skip hashing every file it already knows
    if let Ok(real) = git(&root, &["rev-parse", "--git-path", "index"]) {
        let _ = std::fs::copy(root.join(real.trim()), &index);
    }
    let tree = indexed(&root, &index, &["add", "-A"])
        .and_then(|_| indexed(&root, &index, &["write-tree"]));
    let _ = std::fs::remove_file(&index);
    tree.filter(|t| !t.is_empty())
}

pub fn changes(cwd: &Path, before: &str) -> Option<(String, Vec<RunFile>)> {
    let root = top(cwd)?;
    let after = snapshot(&root)?;
    if after == before {
        return Some((after, Vec::new()));
    }
    let status = git(
        &root,
        &[
            "diff",
            "--no-renames",
            "--name-status",
            "-z",
            before,
            &after,
        ],
    )
    .ok()?;
    let counts = git(
        &root,
        &["diff", "--no-renames", "--numstat", "-z", before, &after],
    )
    .ok()?;
    let mut lines = std::collections::HashMap::new();
    for entry in counts.split('\0').filter(|e| !e.is_empty()) {
        let mut parts = entry.splitn(3, '\t');
        let (Some(a), Some(r), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        lines.insert(
            path.to_string(),
            (a.parse().unwrap_or(0), r.parse().unwrap_or(0)),
        );
    }
    let mut fields = status.split('\0').filter(|e| !e.is_empty());
    let mut files = Vec::new();
    while let (Some(code), Some(path)) = (fields.next(), fields.next()) {
        let kind = match code {
            "A" => ChangeKind::Add,
            "D" => ChangeKind::Delete,
            _ => ChangeKind::Update,
        };
        let (added, removed) = lines.get(path).copied().unwrap_or((0, 0));
        files.push(RunFile {
            path: path.to_string(),
            kind,
            added,
            removed,
        });
    }
    Some((after, files))
}

fn blob(root: &Path, tree: &str, path: &str) -> Option<String> {
    git(
        root,
        &["rev-parse", "-q", "--verify", &format!("{tree}:{path}")],
    )
    .ok()
    .map(|b| b.trim().to_string())
}

pub fn undo(cwd: &Path, before: &str, after: &str, files: &[String]) -> Result<String> {
    let root = top(cwd).ok_or("Not a git repository.")?;
    let now = snapshot(&root).ok_or("Couldn't read the folder's files.")?;
    let (mut restore, mut gone, mut skipped) = (Vec::new(), Vec::new(), Vec::new());
    for path in files {
        if blob(&root, &now, path) != blob(&root, after, path) {
            skipped.push(path.as_str());
        } else if blob(&root, before, path).is_some() {
            restore.push(path.as_str());
        } else {
            gone.push(path.as_str());
        }
    }
    if !restore.is_empty() {
        let source = format!("--source={before}");
        let mut args = vec!["restore", &source, "--worktree", "--"];
        args.extend(restore.iter().copied());
        git(&root, &args)?;
    }
    for path in &gone {
        std::fs::remove_file(root.join(path)).map_err(|e| e.to_string())?;
    }
    let undone = restore.len() + gone.len();
    let what = match undone {
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    };
    Ok(match skipped.as_slice() {
        [] => format!("Put back {what}."),
        [one] => format!("Put back {what}. {one} changed since the run, so it was left alone."),
        many => format!(
            "Put back {what}. {} files changed since the run, so they were left alone.",
            many.len()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    fn read(p: PathBuf) -> String {
        std::fs::read_to_string(p).unwrap().replace("\r\n", "\n")
    }

    #[test]
    fn a_run_lists_its_changes_and_undo_puts_them_back() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        testing::repo(repo);
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(repo.join("gone.txt"), "bye\n").unwrap();
        std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        testing::commit_all(repo, "first");
        std::fs::write(repo.join("draft.txt"), "mine\n").unwrap();
        let index_before = std::fs::read(repo.join(".git/index")).unwrap();

        let before = snapshot(repo).unwrap();
        std::fs::write(repo.join("a.txt"), "one\nthree\nfour\n").unwrap();
        std::fs::write(repo.join("new.txt"), "hi\n").unwrap();
        std::fs::remove_file(repo.join("gone.txt")).unwrap();
        std::fs::create_dir(repo.join("target")).unwrap();
        std::fs::write(repo.join("target/out"), "x").unwrap();

        let (after, files) = changes(repo, &before).unwrap();
        let mut seen: Vec<_> = files
            .iter()
            .map(|f| (f.path.as_str(), f.kind, f.added, f.removed))
            .collect();
        seen.sort_by_key(|s| s.0);
        assert_eq!(
            seen,
            [
                ("a.txt", ChangeKind::Update, 2, 1),
                ("gone.txt", ChangeKind::Delete, 0, 1),
                ("new.txt", ChangeKind::Add, 1, 0),
            ]
        );
        assert_eq!(
            std::fs::read(repo.join(".git/index")).unwrap(),
            index_before
        );

        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(
            undo(repo, &before, &after, &paths).unwrap(),
            "Put back 3 files."
        );
        assert_eq!(read(repo.join("a.txt")), "one\ntwo\n");
        assert_eq!(read(repo.join("gone.txt")), "bye\n");
        assert!(!repo.join("new.txt").exists());
        assert_eq!(read(repo.join("draft.txt")), "mine\n");
        assert_eq!(changes(repo, &before).unwrap().1, []);
    }

    #[test]
    fn undo_leaves_a_file_changed_since_alone() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        testing::repo(repo);
        std::fs::write(repo.join("a.txt"), "a\n").unwrap();
        std::fs::write(repo.join("b.txt"), "b\n").unwrap();
        testing::commit_all(repo, "first");

        let before = snapshot(repo).unwrap();
        std::fs::write(repo.join("a.txt"), "agent\n").unwrap();
        std::fs::write(repo.join("b.txt"), "agent\n").unwrap();
        let (after, _) = changes(repo, &before).unwrap();
        std::fs::write(repo.join("b.txt"), "you\n").unwrap();

        let files = ["a.txt".to_string(), "b.txt".to_string()];
        assert_eq!(
            undo(repo, &before, &after, &files).unwrap(),
            "Put back 1 file. b.txt changed since the run, so it was left alone."
        );
        assert_eq!(read(repo.join("a.txt")), "a\n");
        assert_eq!(read(repo.join("b.txt")), "you\n");
    }

    #[test]
    fn outside_a_repo_there_is_no_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        assert!(snapshot(dir.path()).is_none());
    }
}
