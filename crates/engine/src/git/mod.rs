// Git for the dock, the composer and worktrees, copied from src-tauri/src/devtools. Every call
// shells out to the user's own git and blocks, so callers run these off the UI thread.
// Errors are user-facing strings.

mod commit;
mod setup;
mod status;
mod worktree;

use std::path::Path;
use std::process::Command;

pub use commit::{commit, create_pr, file_op, pr_defaults, push};
pub use setup::{clone, create_project_dir, init, init_repo};
pub use status::{branch_info, changes, diff, is_repo, root};
pub use worktree::{create_worktree, remove_worktree};

pub type Result<T> = std::result::Result<T, String>;

/// A `git` Command. Windows apps launched from a shortcut or an installer can carry a stale PATH
/// without git on it, and then every git call fails with a bare "program not found". So when
/// PATH has no git, fall back to where Git for Windows installs itself.
fn git_cmd() -> Command {
    #[cfg(windows)]
    {
        let on_path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join("git.exe").is_file()))
            .unwrap_or(false);
        if !on_path {
            let mut known = vec![std::path::PathBuf::from(
                r"C:\Program Files\Git\cmd\git.exe",
            )];
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                known.push(Path::new(&local).join(r"Programs\Git\cmd\git.exe"));
            }
            if let Some(p) = known.into_iter().find(|p| p.is_file()) {
                let mut cmd = Command::new(p);
                crate::util::no_window(&mut cmd);
                return cmd;
            }
        }
    }
    let mut cmd = Command::new("git");
    crate::util::no_window(&mut cmd);
    cmd
}

fn not_found(e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        "git is not installed, or is not on PATH.".to_string()
    } else {
        e.to_string()
    }
}

/// Run git in `cwd` and return stdout, or git's own error text.
fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = git_cmd()
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(not_found)?;
    if !out.status.success() {
        // most failures explain themselves on stderr, but some (`nothing to commit`) use stdout
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn empty(cwd: &Path) -> bool {
    cwd.as_os_str().is_empty()
}

/// The GitHub CLI, for pull requests and new repositories.
fn gh(cwd: &Path) -> Command {
    let mut cmd = Command::new("gh");
    cmd.current_dir(cwd);
    crate::util::no_window(&mut cmd);
    cmd
}

const NO_GH: &str = "GitHub CLI (gh) not found. Install it from cli.github.com.";

#[cfg(test)]
pub(crate) mod testing {
    use std::path::Path;

    /// A fresh repo with a committer set, so tests don't depend on the machine's git config.
    pub fn repo(dir: &Path) {
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.name", "Test"],
            &["config", "user.email", "test@example.com"],
            &["config", "commit.gpgsign", "false"],
        ] {
            super::git(dir, args).unwrap();
        }
    }

    /// A bare repo standing in for a remote, so pushes never leave the temp folder.
    pub fn bare(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        super::git(dir, &["init", "-q", "--bare", "-b", "main"]).unwrap();
    }

    /// Adds `remote` as origin and pushes the current branch there with tracking.
    pub fn push_upstream(dir: &Path, remote: &Path) {
        let url = remote.to_string_lossy();
        super::git(dir, &["remote", "add", "origin", &url]).unwrap();
        super::git(dir, &["push", "-q", "-u", "origin", "HEAD"]).unwrap();
    }

    pub fn commit_all(dir: &Path, msg: &str) {
        super::git(dir, &["add", "-A"]).unwrap();
        super::git(dir, &["commit", "-q", "-m", msg]).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn errors_carry_gits_own_message() {
        let dir = tempfile::tempdir().unwrap();
        let err = git(dir.path(), &["rev-parse", "--show-toplevel"]).unwrap_err();
        assert!(err.contains("not a git repository"), "{err}");
    }

    #[test]
    fn runs_in_the_given_folder() {
        let dir = tempfile::tempdir().unwrap();
        testing::repo(dir.path());
        let top = git(dir.path(), &["rev-parse", "--show-toplevel"]).unwrap();
        let top = PathBuf::from(top.trim());
        assert_eq!(
            top.canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }
}
