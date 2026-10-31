use std::path::Path;

use hyprspace_proto::folder::DiffScope;

use super::checkpoint::{snapshot, top};
use super::{Result, git};

pub fn scoped_diff(cwd: &Path, path: &str, scope: &DiffScope) -> Result<String> {
    let root = top(cwd).ok_or("This folder is not a git repository.")?;
    let (from, to) = match scope {
        DiffScope::Run { before, after } => {
            if git(&root, &["cat-file", "-e", before]).is_err()
                || git(&root, &["cat-file", "-e", after]).is_err()
            {
                return Err("Git no longer has this run's snapshots.".into());
            }
            (before.clone(), after.clone())
        }
        DiffScope::Uncommitted => (head(&root)?, now(&root)?),
        DiffScope::Branch => {
            let (_, base) =
                base(&root).ok_or("This branch has no other branch to compare with.")?;
            let fork = git(&root, &["merge-base", &base, "HEAD"])
                .map_err(|_| format!("This branch shares no history with {base}."))?;
            (fork.trim().to_string(), now(&root)?)
        }
    };
    let mut args = vec!["diff", "--no-renames", &from, &to];
    if !path.is_empty() {
        args.extend(["--", path]);
    }
    git(&root, &args)
}

pub fn base(cwd: &Path) -> Option<(String, String)> {
    let name = git(
        cwd,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()
    .and_then(|r| r.trim().strip_prefix("origin/").map(str::to_string))
    .or_else(|| {
        ["main", "master"]
            .into_iter()
            .find(|b| has(cwd, &format!("refs/heads/{b}")))
            .map(str::to_string)
    })?;
    let on = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
    if on.trim() == name {
        return None;
    }
    // branches start from the local one, which may be ahead of the remote's
    let reference = if has(cwd, &format!("refs/heads/{name}")) {
        name.clone()
    } else {
        format!("origin/{name}")
    };
    Some((name, reference))
}

fn has(cwd: &Path, reference: &str) -> bool {
    git(cwd, &["rev-parse", "--verify", "--quiet", reference]).is_ok()
}

fn head(root: &Path) -> Result<String> {
    match git(root, &["rev-parse", "--verify", "--quiet", "HEAD^{tree}"]) {
        Ok(tree) => Ok(tree.trim().to_string()),
        Err(_) => git(root, &["mktree"]).map(|t| t.trim().to_string()),
    }
}

fn now(root: &Path) -> Result<String> {
    snapshot(root).ok_or_else(|| "Couldn't read the folder's files.".to_string())
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    fn run(repo: &Path, path: &str, scope: &DiffScope) -> String {
        scoped_diff(repo, path, scope)
            .unwrap()
            .replace("\r\n", "\n")
    }

    #[test]
    fn each_scope_covers_more() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        testing::repo(repo);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        testing::commit_all(repo, "first");
        assert_eq!(base(repo), None);

        git(repo, &["checkout", "-q", "-b", "feature"]).unwrap();
        std::fs::write(repo.join("b.txt"), "branch\n").unwrap();
        testing::commit_all(repo, "second");
        std::fs::write(repo.join("c.txt"), "loose\n").unwrap();

        let before = snapshot(repo).unwrap();
        std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
        let after = snapshot(repo).unwrap();
        std::fs::write(repo.join("a.txt"), "one\ntwo\nthree\n").unwrap();

        let ran = DiffScope::Run { before, after };
        let d = run(repo, "", &ran);
        assert!(d.contains("+two") && !d.contains("+three"), "{d}");
        assert!(!d.contains("c.txt"), "{d}");

        let d = run(repo, "", &DiffScope::Uncommitted);
        assert!(d.contains("+three") && d.contains("+loose"), "{d}");
        assert!(!d.contains("+branch"), "{d}");

        assert_eq!(base(repo), Some(("main".into(), "main".into())));
        let d = run(repo, "", &DiffScope::Branch);
        assert!(d.contains("+branch") && d.contains("+loose"), "{d}");

        let d = run(repo, "b.txt", &DiffScope::Branch);
        assert!(d.contains("+branch") && !d.contains("+loose"), "{d}");
    }

    #[test]
    fn a_repo_with_no_commits_diffs_against_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        testing::repo(repo);
        std::fs::write(repo.join("a.txt"), "new\n").unwrap();
        assert!(run(repo, "", &DiffScope::Uncommitted).contains("+new"));
        assert!(scoped_diff(repo, "", &DiffScope::Branch).is_err());
    }

    #[test]
    fn a_lost_snapshot_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        testing::repo(repo);
        let gone = "0123456789012345678901234567890123456789".to_string();
        let scope = DiffScope::Run {
            before: gone.clone(),
            after: gone,
        };
        assert_eq!(
            scoped_diff(repo, "", &scope).unwrap_err(),
            "Git no longer has this run's snapshots."
        );
    }
}
