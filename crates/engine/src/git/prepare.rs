// Getting a new worktree ready: the ignored files its repo lists in `.worktreeinclude` (the file
// Claude Code reads too), such as `.env`, which a checkout leaves behind, then the space's setup
// command.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use hyprspace_proto::git::Worktree;

use super::{Result, git};

const PATIENCE: Duration = Duration::from_secs(15 * 60);

/// Copies the repo's `.worktreeinclude` files into `wt`, then runs `setup` there. A repo with no
/// such file, or a space with no command, is ready as it is.
pub fn prepare(wt: &Worktree, setup: Option<&str>) -> Result<()> {
    copy_included(&wt.repo, &wt.path);
    match setup.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cmd) => run(&wt.path, cmd),
        None => Ok(()),
    }
}

fn copy_included(repo: &Path, to: &Path) {
    let list = repo.join(".worktreeinclude");
    if !list.is_file() {
        return;
    }
    let from = list.to_string_lossy();
    let Ok(files) = git(
        repo,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-from",
            &from,
        ],
    ) else {
        return;
    };
    for file in files.split('\0').filter(|f| !f.is_empty()) {
        let dest = to.join(file);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(repo.join(file), dest);
    }
}

fn shell(cmd: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        // cmd parses its own command line, so the command goes in as typed
        c.raw_arg(format!("/D /S /C \"{cmd}\""));
        // a GUI app spawning a console program gets a stray console window without this
        c.creation_flags(0x0800_0000);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = Command::new("sh");
        c.args(["-c", cmd]);
        c
    }
}

fn run(dir: &Path, cmd: &str) -> Result<()> {
    let child = shell(cmd)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't run the setup command: {e}"))?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let out = match rx.recv_timeout(PATIENCE) {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(format!("The setup command failed: {e}")),
        Err(_) => return Err("The setup command was still running after 15 minutes.".into()),
    };
    if out.status.success() {
        return Ok(());
    }
    let said = if out.stderr.iter().any(|b| !b.is_ascii_whitespace()) {
        out.stderr
    } else {
        out.stdout
    };
    let said = String::from_utf8_lossy(&said);
    let tail: Vec<&str> = said.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = tail[tail.len().saturating_sub(3)..].join("\n");
    let code = out
        .status
        .code()
        .map_or("stopped".into(), |c| format!("exited with {c}"));
    Err(format!("The setup command {code}: {tail}"))
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    #[test]
    fn a_new_worktree_gets_the_listed_ignored_files_and_runs_the_setup() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        testing::repo(&repo);
        std::fs::write(repo.join(".gitignore"), ".env\nbig/\n").unwrap();
        std::fs::write(repo.join(".worktreeinclude"), ".env\n").unwrap();
        testing::commit_all(&repo, "first");
        std::fs::write(repo.join(".env"), "KEY=1\n").unwrap();
        std::fs::create_dir(repo.join("big")).unwrap();
        std::fs::write(repo.join("big").join("x"), "x").unwrap();
        let path = dir.path().join("wt");
        git(&repo, &["worktree", "add", "-q", &path.to_string_lossy()]).unwrap();
        let wt = Worktree {
            path: path.clone(),
            branch: "wt".into(),
            base: "main".into(),
            repo,
        };

        prepare(&wt, Some("echo ready> made.txt")).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join(".env")).unwrap(),
            "KEY=1\n"
        );
        assert!(!path.join("big").exists());
        assert!(path.join("made.txt").exists());

        let err = prepare(&wt, Some("echo nope 1>&2 && exit 3")).unwrap_err();
        assert!(
            err.contains("exited with 3") && err.contains("nope"),
            "{err}"
        );
    }
}
