// Isolated worktrees, so an agent can work without colliding with others. They live under
// ~/.hyprspace/worktrees/<repo>-<branch>, outside the repo, so they don't pollute its own status.

use std::path::{Path, PathBuf};

use super::{Result, empty, git};

/// Create (or reuse) a worktree for `name` off the repo containing `cwd`, on branch `hs/<name>`.
pub fn create_worktree(cwd: &Path, name: &str) -> Result<PathBuf> {
    create_in(
        &crate::util::home_dir().join(".hyprspace").join("worktrees"),
        cwd,
        name,
    )
}

fn create_in(root_dir: &Path, cwd: &Path, name: &str) -> Result<PathBuf> {
    if empty(cwd) {
        return Err("No workspace folder.".into());
    }
    let root = git(cwd, &["rev-parse", "--show-toplevel"]).map_err(|_| "Not a git repository.")?;
    let branch = branch_for(name);
    let repo_name = Path::new(root.trim())
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    let wt = root_dir.join(format!("{repo_name}-{}", branch.replace('/', "-")));

    // idempotent: a stable-named caller that re-runs reuses its worktree instead of erroring.
    // existing dir: reuse as-is; existing branch but no dir: re-attach; else create new.
    if wt.exists() {
        return Ok(wt);
    }
    let wt_s = wt.to_string_lossy();
    let branch_exists = git(
        cwd,
        &["rev-parse", "--verify", &format!("refs/heads/{branch}")],
    )
    .is_ok();
    if branch_exists {
        git(cwd, &["worktree", "add", &wt_s, &branch])?;
    } else {
        git(cwd, &["worktree", "add", "-b", &branch, &wt_s, "HEAD"])?;
    }
    Ok(wt)
}

fn branch_for(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("hs/{}", safe.trim_matches('-'))
}

pub fn remove_worktree(cwd: &Path, path: &Path) -> Result<()> {
    git(
        cwd,
        &["worktree", "remove", "--force", &path.to_string_lossy()],
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    #[test]
    fn branch_names_are_safe() {
        assert_eq!(branch_for("Fix login!"), "hs/Fix-login");
        assert_eq!(branch_for("a/b"), "hs/a-b");
    }

    #[test]
    fn creates_reuses_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        testing::repo(&repo);
        std::fs::write(repo.join("a.txt"), "hi\n").unwrap();
        testing::commit_all(&repo, "first");

        let trees = dir.path().join("trees");
        let wt = create_in(&trees, &repo, "task one").unwrap();
        assert!(wt.join("a.txt").exists());
        assert!(wt.ends_with("repo-hs-task-one"));
        assert_eq!(create_in(&trees, &repo, "task one").unwrap(), wt);

        remove_worktree(&repo, &wt).unwrap();
        assert!(!wt.exists());
        // the branch survives removal, so the next create re-attaches it
        let again = create_in(&trees, &repo, "task one").unwrap();
        assert!(again.join("a.txt").exists());
        assert!(create_in(&trees, dir.path(), "x").is_err());
    }
}
