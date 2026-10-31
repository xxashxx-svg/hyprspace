// Isolated worktrees, so an agent can work without colliding with others. They live under
// ~/.hyprspace/worktrees/<repo>-<branch>, outside the repo, so they don't pollute its own status.

use std::path::{Path, PathBuf};
use std::time::Duration;

use hyprspace_proto::git::{Worktree, WorktreeAction};

use super::{Result, create_pr, empty, git};

pub fn create_worktree(cwd: &Path, base: Option<&str>) -> Result<Worktree> {
    let mut token = [0u8; 4];
    getrandom::fill(&mut token).map_err(|e| e.to_string())?;
    let token: String = token.iter().map(|b| format!("{b:02x}")).collect();
    create_in(
        &crate::util::home_dir().join(".hyprspace").join("worktrees"),
        cwd,
        &token,
        base,
    )
}

/// Renames a worktree's placeholder branch after `title`, while nothing outside knows it: it has
/// no upstream yet. A name another branch has gets a number. The folder never moves, since the
/// CLIs keep their conversations by folder.
pub fn name_branch(wt: &Worktree, title: &str) -> Result<String> {
    if !wt.placeholder() {
        return Err(format!("{} already has a name.", wt.branch));
    }
    let upstream = format!("{}@{{upstream}}", wt.branch);
    if git(&wt.path, &["rev-parse", "--verify", "-q", &upstream]).is_ok() {
        return Err(format!("{} is pushed, so it keeps its name.", wt.branch));
    }
    let named = branch_for(title);
    if named == "hs/" {
        return Err("That title makes no branch name.".into());
    }
    let taken = |b: &str| {
        git(
            &wt.repo,
            &["rev-parse", "--verify", "-q", &format!("refs/heads/{b}")],
        )
        .is_ok()
    };
    let mut to = named.clone();
    let mut n = 2;
    while taken(&to) {
        to = format!("{named}-{n}");
        n += 1;
    }
    git(&wt.path, &["branch", "-m", &wt.branch, &to])?;
    Ok(to)
}

fn create_in(root_dir: &Path, cwd: &Path, name: &str, base: Option<&str>) -> Result<Worktree> {
    if empty(cwd) {
        return Err("No workspace folder.".into());
    }
    let root = git(cwd, &["rev-parse", "--show-toplevel"]).map_err(|_| "Not a git repository.")?;
    let repo = PathBuf::from(root.trim());
    let base = base
        .map(str::to_string)
        .or_else(|| current_branch(&repo))
        .ok_or("Check out a branch first.")?;
    let branch = branch_for(name);
    let repo_name = repo
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    let wt = root_dir.join(format!("{repo_name}-{}", branch.replace('/', "-")));
    let made = |path: PathBuf| Worktree {
        path,
        branch: branch.clone(),
        base: base.clone(),
        repo: repo.clone(),
    };

    // idempotent: a stable-named caller that re-runs reuses its worktree instead of erroring.
    // existing dir: reuse as-is; existing branch but no dir: re-attach; else create new.
    if wt.exists() {
        return Ok(made(wt));
    }
    let wt_s = wt.to_string_lossy();
    let branch_exists = git(
        &repo,
        &["rev-parse", "--verify", &format!("refs/heads/{branch}")],
    )
    .is_ok();
    if branch_exists {
        git(&repo, &["worktree", "add", &wt_s, &branch])?;
    } else {
        git(&repo, &["worktree", "add", "-b", &branch, &wt_s, &base])?;
    }
    Ok(made(wt))
}

fn branch_for(name: &str) -> String {
    let mut safe = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = if c.is_alphanumeric() || c == '_' {
            c
        } else {
            '-'
        };
        if !(c == '-' && safe.ends_with('-')) {
            safe.push(c);
        }
    }
    let safe: String = safe.trim_matches('-').chars().take(48).collect();
    format!("hs/{}", safe.trim_end_matches('-'))
}

pub fn branches(cwd: &Path) -> Vec<String> {
    git(cwd, &["branch", "--format=%(refname:short)"])
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|b| !b.is_empty() && !b.starts_with("hs/"))
        .map(str::to_string)
        .collect()
}

fn checkout_of(repo: &Path, branch: &str) -> Option<PathBuf> {
    let list = git(repo, &["worktree", "list", "--porcelain"]).ok()?;
    let want = format!("branch refs/heads/{branch}");
    let mut path = None;
    for line in list.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(p));
        } else if line == want {
            return path;
        }
    }
    None
}

fn current_branch(repo: &Path) -> Option<String> {
    git(repo, &["symbolic-ref", "--short", "-q", "HEAD"])
        .ok()
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
}

pub fn finish(wt: &Worktree, action: WorktreeAction, message: &str) -> Result<String> {
    match action {
        WorktreeAction::Merge => merge(wt, message),
        WorktreeAction::PullRequest => pull_request(wt, message),
        WorktreeAction::Remove => remove(wt, message),
    }
}

fn save_work(wt: &Worktree, message: &str) -> Result<()> {
    if !wt.path.exists() || git(&wt.path, &["status", "--porcelain"])?.trim().is_empty() {
        return Ok(());
    }
    git(&wt.path, &["add", "-A"])?;
    git(&wt.path, &["commit", "-q", "-m", message]).map(|_| ())
}

fn ahead(wt: &Worktree) -> Result<u32> {
    let range = format!("{}..{}", wt.base, wt.branch);
    let count = git(&wt.repo, &["rev-list", "--count", &range])?;
    Ok(count.trim().parse().unwrap_or(0))
}

fn merging(repo: &Path) -> bool {
    git(repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok()
}

fn merge(wt: &Worktree, message: &str) -> Result<String> {
    save_work(wt, message)?;
    if ahead(wt)? == 0 {
        return Ok(format!("{} has nothing new to merge.", wt.branch));
    }
    let Some(into) = checkout_of(&wt.repo, &wt.base) else {
        return Err(format!(
            "{} isn't checked out anywhere. Check it out to merge into it.",
            wt.base
        ));
    };
    if merging(&into) {
        let folder = into
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        return Err(format!(
            "{folder} has a merge in progress. Finish it first."
        ));
    }
    match git(&into, &["merge", "--no-ff", "--no-edit", &wt.branch]) {
        Ok(_) => Ok(format!("Merged {} into {}.", wt.branch, wt.base)),
        Err(e) => {
            if merging(&into) {
                let _ = git(&into, &["merge", "--abort"]);
                return Err(format!(
                    "{} conflicts with {}, so nothing was merged.",
                    wt.branch, wt.base
                ));
            }
            Err(e)
        }
    }
}

fn pull_request(wt: &Worktree, message: &str) -> Result<String> {
    save_work(wt, message)?;
    if ahead(wt)? == 0 {
        return Err(format!(
            "{} has no changes to open a pull request for.",
            wt.branch
        ));
    }
    let range = format!("{}..HEAD", wt.base);
    let body = git(&wt.path, &["log", "--pretty=- %s", &range]).unwrap_or_default();
    create_pr(&wt.path, message, body.trim(), &wt.base, false, true)
}

fn remove(wt: &Worktree, message: &str) -> Result<String> {
    if wt.path.exists() {
        save_work(wt, message)?;
        let path = wt.path.to_string_lossy();
        // the agent that ran there may take a moment to let go of the folder on Windows
        let mut tries = 0;
        while let Err(e) = git(&wt.repo, &["worktree", "remove", "--force", &path]) {
            tries += 1;
            if tries == 5 {
                return Err(e);
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    let _ = git(&wt.repo, &["worktree", "prune"]);
    let home = checkout_of(&wt.repo, &wt.base).unwrap_or_else(|| wt.repo.clone());
    let kept = git(&home, &["branch", "-d", &wt.branch]).is_err()
        && git(
            &wt.repo,
            &[
                "rev-parse",
                "--verify",
                &format!("refs/heads/{}", wt.branch),
            ],
        )
        .is_ok();
    Ok(if kept {
        format!(
            "Removed the worktree. {} is kept because it isn't merged.",
            wt.branch
        )
    } else {
        "Removed the worktree and its branch.".into()
    })
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        testing::repo(&repo);
        std::fs::write(repo.join("a.txt"), "hi\n").unwrap();
        testing::commit_all(&repo, "first");
        let trees = dir.path().join("trees");
        (dir, repo, trees)
    }

    fn branches(repo: &Path) -> String {
        git(repo, &["branch", "--format=%(refname:short)"]).unwrap()
    }

    #[test]
    fn branch_names_are_safe() {
        assert_eq!(branch_for("Fix login!"), "hs/fix-login");
        assert_eq!(branch_for("a/b  c"), "hs/a-b-c");
        assert_eq!(branch_for(&"x".repeat(80)).len(), 3 + 48);
    }

    #[test]
    fn creates_from_the_checked_out_branch_and_reuses() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "task one", None).unwrap();
        assert!(wt.path.join("a.txt").exists());
        assert!(wt.path.ends_with("repo-hs-task-one"));
        assert_eq!(
            (wt.branch.as_str(), wt.base.as_str()),
            ("hs/task-one", "main")
        );
        assert_eq!(create_in(&trees, &repo, "task one", None).unwrap(), wt);
        assert!(create_in(&trees, trees.parent().unwrap(), "x", None).is_err());
    }

    #[test]
    fn a_placeholder_branch_takes_the_title_once_and_the_folder_stays() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "0a1b2c3d", None).unwrap();
        assert!(wt.placeholder());
        assert!(wt.path.ends_with("repo-hs-0a1b2c3d"));
        create_in(&trees, &repo, "draft autosave", None).unwrap();
        assert_eq!(
            name_branch(&wt, "Draft autosave!").unwrap(),
            "hs/draft-autosave-2"
        );
        assert!(branches(&repo).contains("hs/draft-autosave-2"));
        assert!(wt.path.exists());
        let named = Worktree {
            branch: "hs/draft-autosave-2".into(),
            ..wt
        };
        assert!(!named.placeholder());
        assert!(name_branch(&named, "Something else").is_err());

        let pushed = create_in(&trees, &repo, "1234abcd", None).unwrap();
        git(&pushed.path, &["branch", "-u", "main"]).unwrap();
        assert!(name_branch(&pushed, "Anything").is_err());
        assert!(branches(&repo).contains("hs/1234abcd"));
    }

    #[test]
    fn a_merge_commits_the_work_and_brings_it_home() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "add b", None).unwrap();
        std::fs::write(wt.path.join("b.txt"), "b\n").unwrap();
        assert_eq!(
            finish(&wt, WorktreeAction::Merge, "Add b").unwrap(),
            "Merged hs/add-b into main."
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("b.txt")).unwrap().trim(),
            "b"
        );
        assert_eq!(
            finish(&wt, WorktreeAction::Merge, "Add b").unwrap(),
            "hs/add-b has nothing new to merge."
        );

        assert_eq!(
            finish(&wt, WorktreeAction::Remove, "Add b").unwrap(),
            "Removed the worktree and its branch."
        );
        assert!(!wt.path.exists());
        assert!(!branches(&repo).contains("hs/add-b"));
    }

    #[test]
    fn a_merge_waits_for_the_base_branch_and_backs_out_of_conflicts() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "edit a", None).unwrap();
        std::fs::write(wt.path.join("a.txt"), "theirs\n").unwrap();

        git(&repo, &["switch", "-q", "-c", "other"]).unwrap();
        let err = finish(&wt, WorktreeAction::Merge, "Edit a").unwrap_err();
        assert_eq!(
            err,
            "main isn't checked out anywhere. Check it out to merge into it."
        );
        git(&repo, &["switch", "-q", "main"]).unwrap();

        std::fs::write(repo.join("a.txt"), "ours\n").unwrap();
        testing::commit_all(&repo, "ours");
        let err = finish(&wt, WorktreeAction::Merge, "Edit a").unwrap_err();
        assert_eq!(err, "hs/edit-a conflicts with main, so nothing was merged.");
        assert!(!merging(&repo));
        assert_eq!(
            std::fs::read_to_string(repo.join("a.txt")).unwrap().trim(),
            "ours"
        );
    }

    #[test]
    fn removing_unmerged_work_keeps_it_on_the_branch() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "half done", None).unwrap();
        std::fs::write(wt.path.join("c.txt"), "c\n").unwrap();
        assert_eq!(
            finish(&wt, WorktreeAction::Remove, "Half done").unwrap(),
            "Removed the worktree. hs/half-done is kept because it isn't merged."
        );
        assert!(!wt.path.exists());
        let kept = git(&repo, &["show", "hs/half-done:c.txt"]).unwrap();
        assert_eq!(kept, "c\n");
    }

    #[test]
    fn a_worktree_off_another_branch_merges_where_that_branch_is_checked_out() {
        let (dir, repo, trees) = setup();
        git(&repo, &["switch", "-q", "-c", "rewrite"]).unwrap();
        let main = dir.path().join("main");
        git(
            &repo,
            &["worktree", "add", "-q", &main.to_string_lossy(), "main"],
        )
        .unwrap();
        assert_eq!(branches(&repo).lines().count(), 2);

        let wt = create_in(&trees, &repo, "from main", Some("main")).unwrap();
        assert_eq!(wt.base, "main");
        assert_eq!(super::branches(&repo), ["main", "rewrite"]);
        std::fs::write(wt.path.join("d.txt"), "d\n").unwrap();
        assert_eq!(
            finish(&wt, WorktreeAction::Merge, "Add d").unwrap(),
            "Merged hs/from-main into main."
        );
        assert!(main.join("d.txt").exists());
        assert!(!repo.join("d.txt").exists());
        assert_eq!(
            finish(&wt, WorktreeAction::Remove, "Add d").unwrap(),
            "Removed the worktree and its branch."
        );
    }

    #[test]
    fn a_pull_request_needs_commits() {
        let (_dir, repo, trees) = setup();
        let wt = create_in(&trees, &repo, "nothing", None).unwrap();
        assert_eq!(
            finish(&wt, WorktreeAction::PullRequest, "Nothing").unwrap_err(),
            "hs/nothing has no changes to open a pull request for."
        );
    }
}
