// Dev cockpit backend, split into focused submodules so several agents can work in parallel.
// Shared helpers (git, home_dir, read_json) live here; submodules pull them via `use super::*`.
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use serde_json::Value;

mod git;
mod sessions;
mod live_usage;
mod worktree;
mod project;
mod fs;
mod providers;
mod usage;
mod skills;

pub use git::*;
pub use sessions::*;
pub use live_usage::*;
pub use worktree::*;
pub use project::*;
pub use fs::*;
pub use providers::*;
pub use usage::*;
pub use skills::*;

// ---- shared helpers ----

// A `git` Command. Windows apps launched from a shortcut or an installer can carry a stale PATH
// without git on it, and then every git call fails with a bare "program not found". So when
// PATH has no git, fall back to where Git for Windows installs itself.
pub(crate) fn git_cmd() -> Command {
    #[cfg(windows)]
    {
        let on_path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join("git.exe").is_file()))
            .unwrap_or(false);
        if !on_path {
            let mut known = vec![std::path::PathBuf::from(r"C:\Program Files\Git\cmd\git.exe")];
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                known.push(std::path::Path::new(&local).join(r"Programs\Git\cmd\git.exe"));
            }
            if let Some(p) = known.into_iter().find(|p| p.is_file()) {
                return Command::new(p);
            }
        }
    }
    Command::new("git")
}

fn git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = git_cmd();
    cmd.arg("-C").arg(cwd).args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW — don't flash a console on each git call
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "git is not installed, or is not on PATH.".to_string()
        } else {
            e.to_string()
        }
    })?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// Changed files in the active workspace's repo, with +/- line counts where git has them.
// git on a large repo can take seconds, and sync Tauri commands run on the UI thread — so the
// dock's 4s poll would freeze the window. Push the work onto a blocking thread.

fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default(),
    )
}

// run "<cli> --version". On Windows go through `cmd /c` so a .cmd/.ps1 shim on PATH resolves —
// but pass the args separately (never build a shell string) so `cli` can't be misread as syntax.

fn read_json(path: std::path::PathBuf) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

// pull the (unverified) payload out of a JWT — we only read display claims, never trust it
