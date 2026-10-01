// Changing a working tree: stage, commit, push, and open a pull request.

use std::path::Path;

use hyprspace_proto::git::{FileOp, PrDefaults};

use super::{NO_GH, Result, empty, gh, git, git_cmd, is_repo};

/// Stage, unstage or discard one file. An empty path means every file, which discard refuses.
pub fn file_op(cwd: &Path, op: FileOp, path: &str) -> Result<()> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    let all = path.is_empty();
    match op {
        FileOp::Stage if all => git(cwd, &["add", "-A"])?,
        FileOp::Stage => git(cwd, &["add", "--", path])?,
        FileOp::Unstage if all => git(cwd, &["reset", "-q"])?,
        FileOp::Unstage => git(cwd, &["reset", "-q", "--", path])?,
        FileOp::Discard if all => return Err("Refusing to discard everything at once.".into()),
        FileOp::Discard => {
            // restore a tracked file to HEAD; if it's untracked (not in HEAD), delete it
            if git(cwd, &["checkout", "HEAD", "--", path]).is_err() {
                let _ = std::fs::remove_file(cwd.join(path));
            }
            String::new()
        }
    };
    Ok(())
}

fn push_current(cwd: &Path) -> Result<()> {
    match git(cwd, &["push"]) {
        Ok(_) => Ok(()),
        Err(e)
            if e.contains("no upstream")
                || e.contains("has no upstream")
                || e.contains("set-upstream") =>
        {
            git(cwd, &["push", "-u", "origin", "HEAD"]).map(|_| ())
        }
        Err(e) => Err(e),
    }
}

/// Commit with `message` and optionally push. `stage_all` stages everything first; otherwise
/// only what's already staged goes in. Returns a line for the UI.
pub fn commit(cwd: &Path, message: &str, push: bool, stage_all: bool) -> Result<String> {
    if !is_repo(cwd) {
        return Err("Not a git repository.".to_string());
    }
    if stage_all {
        git(cwd, &["add", "-A"])?;
    }
    if let Err(e) = git(cwd, &["commit", "-m", message]) {
        return Err(if e.contains("nothing to commit") {
            "Nothing staged to commit.".to_string()
        } else {
            e
        });
    }
    if push {
        push_current(cwd)?;
        return Ok("Changes committed and pushed.".to_string());
    }
    Ok("Changes committed.".to_string())
}

pub fn push(cwd: &Path) -> Result<String> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    push_current(cwd)?;
    Ok("Pushed to remote.".to_string())
}

/// Suggested defaults to pre-fill the pull request dialog: branches, and a title and body from
/// the commits.
pub fn pr_defaults(cwd: &Path) -> Result<PrDefaults> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    let head = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();
    // default base = the remote's HEAD branch, else main, else master
    let base = git(cwd, &["rev-parse", "--abbrev-ref", "origin/HEAD"])
        .ok()
        .map(|s| s.trim().trim_start_matches("origin/").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if git(cwd, &["rev-parse", "--verify", "main"]).is_ok() {
                "main".to_string()
            } else if git(cwd, &["rev-parse", "--verify", "master"]).is_ok() {
                "master".to_string()
            } else {
                "main".to_string()
            }
        });
    // title: the latest commit subject, else the branch name made readable
    let subj = git(cwd, &["log", "-1", "--pretty=%s"])
        .unwrap_or_default()
        .trim()
        .to_string();
    let title = if !subj.is_empty() {
        subj
    } else {
        head.replace(['-', '_'], " ")
    };
    // body: the commit subjects on head but not base, as a bullet list (best-effort)
    let body = git(cwd, &["log", "--pretty=- %s", &format!("{base}..{head}")])
        .unwrap_or_default()
        .trim()
        .to_string();
    // branch list for the base picker (local + remote, deduped, no HEAD)
    let mut branches: Vec<String> = vec![];
    let mut add = |b: &str| {
        if !b.is_empty() && b != "HEAD" && !branches.iter().any(|x| x == b) {
            branches.push(b.to_string());
        }
    };
    if let Ok(out) = git(cwd, &["branch", "--format=%(refname:short)"]) {
        out.lines().for_each(|l| add(l.trim()));
    }
    if let Ok(out) = git(cwd, &["branch", "-r", "--format=%(refname:short)"]) {
        out.lines()
            .for_each(|l| add(l.trim().trim_start_matches("origin/")));
    }
    let pushed = git(
        cwd,
        &["rev-parse", "--abbrev-ref", &format!("{head}@{{upstream}}")],
    )
    .is_ok();
    let on_default = head == base;
    Ok(PrDefaults {
        head,
        base,
        title,
        body,
        branches,
        pushed,
        on_default,
    })
}

/// Open a GitHub pull request through the gh CLI and return its URL. Pushes the branch first when
/// asked, so gh never tries to prompt about where to push.
pub fn create_pr(
    cwd: &Path,
    title: &str,
    body: &str,
    base: &str,
    draft: bool,
    push: bool,
) -> Result<String> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    if push {
        let head = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string();
        if !head.is_empty() {
            // best-effort; gh reports a clear error if the branch still isn't there
            let _ = git_cmd()
                .current_dir(cwd)
                .args(["push", "-u", "origin", &head])
                .output();
        }
    }
    let mut cmd = gh(cwd);
    cmd.args(["pr", "create"]);
    if !title.trim().is_empty() {
        cmd.args(["--title", title]);
    }
    cmd.args(["--body", body]);
    if !base.trim().is_empty() {
        cmd.args(["--base", base]);
    }
    if draft {
        cmd.arg("--draft");
    }
    let out = cmd.output().map_err(|_| NO_GH.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "Couldn't create the pull request.".to_string()
        } else {
            err
        });
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if url.is_empty() {
        String::from_utf8_lossy(&out.stderr).trim().to_string()
    } else {
        url
    })
}

#[cfg(test)]
mod tests {
    use super::super::{changes, testing};
    use super::*;

    #[test]
    fn stage_commit_and_discard() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        assert_eq!(
            commit(cwd, "x", false, true).unwrap_err(),
            "Not a git repository."
        );
        testing::repo(cwd);
        std::fs::write(cwd.join("a.txt"), "one\n").unwrap();
        file_op(cwd, FileOp::Stage, "a.txt").unwrap();
        assert_eq!(changes(cwd).unwrap()[0].status, "A ");
        assert_eq!(
            commit(cwd, "first", false, false).unwrap(),
            "Changes committed."
        );
        assert_eq!(
            commit(cwd, "again", false, false).unwrap_err(),
            "Nothing staged to commit."
        );

        std::fs::write(cwd.join("a.txt"), "changed\n").unwrap();
        std::fs::write(cwd.join("junk.txt"), "x").unwrap();
        assert!(file_op(cwd, FileOp::Discard, "").is_err());
        file_op(cwd, FileOp::Discard, "a.txt").unwrap();
        file_op(cwd, FileOp::Discard, "junk.txt").unwrap();
        assert_eq!(changes(cwd).unwrap(), []);
        assert_eq!(
            std::fs::read_to_string(cwd.join("a.txt"))
                .unwrap()
                .replace('\r', ""),
            "one\n"
        );
    }

    #[test]
    fn pr_defaults_read_the_branch_and_commits() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        testing::repo(cwd);
        std::fs::write(cwd.join("a.txt"), "one\n").unwrap();
        testing::commit_all(cwd, "first");
        git(cwd, &["checkout", "-q", "-b", "fix-thing"]).unwrap();
        std::fs::write(cwd.join("a.txt"), "two\n").unwrap();
        testing::commit_all(cwd, "Fix the thing");

        let d = pr_defaults(cwd).unwrap();
        assert_eq!(d.head, "fix-thing");
        assert_eq!(d.base, "main");
        assert_eq!(d.title, "Fix the thing");
        assert_eq!(d.body, "- Fix the thing");
        assert!(d.branches.contains(&"main".to_string()));
        assert!(!d.pushed);
        assert!(!d.on_default);
    }
}
