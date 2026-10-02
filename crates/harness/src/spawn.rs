// Starting an agent CLI as a child with piped stdio, and keeping the end of its stderr so an
// unexpected exit can say why (zeron's StderrTail, MIT, see THIRD_PARTY_NOTICES.md).

use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::SESSION_ENV;

pub(crate) struct Proc {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: Tail,
}

fn command(program: &Path, args: &[String], cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    for k in SESSION_ENV {
        cmd.env_remove(k);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // a GUI app spawning a console program gets a stray console window without this
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

/// Spawns `program args` in `cwd`. On Windows a bare name that is not an .exe on PATH (npm
/// installs CLIs as .cmd shims, which only cmd.exe resolves) is retried through `cmd /c`.
pub(crate) fn spawn(program: &Path, args: &[String], cwd: &Path) -> io::Result<Proc> {
    let child = match command(program, args, cwd).spawn() {
        Err(e)
            if cfg!(windows)
                && e.kind() == io::ErrorKind::NotFound
                && program.parent() == Some(Path::new("")) =>
        {
            let mut shim = vec!["/c".to_string(), program.to_string_lossy().into_owned()];
            shim.extend_from_slice(args);
            command(Path::new("cmd"), &shim, cwd).spawn()?
        }
        other => other?,
    };
    let mut child = child;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = Tail::default();
    let err = child.stderr.take().expect("piped stderr");
    let tail = stderr.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(err).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tail.push(&line);
        }
    });
    Ok(Proc {
        child,
        stdin,
        stdout,
        stderr,
    })
}

/// The last few lines a child wrote to stderr.
#[derive(Clone, Default)]
pub(crate) struct Tail(Arc<Mutex<VecDeque<String>>>);

impl Tail {
    const KEEP: usize = 6;

    fn push(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let mut tail = self.0.lock().unwrap_or_else(|e| e.into_inner());
        tail.push_back(line.chars().take(500).collect());
        while tail.len() > Self::KEEP {
            tail.pop_front();
        }
    }

    fn text(&self) -> String {
        let tail = self.0.lock().unwrap_or_else(|e| e.into_inner());
        tail.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

/// What the user reads when a CLI exits on its own: the status and the end of its stderr.
pub(crate) async fn exit_message(name: &str, child: &mut Child, stderr: &Tail) -> String {
    // the stderr reader may still be draining the last lines
    let status = tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
        .await
        .ok()
        .and_then(Result::ok);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    describe(name, status, &stderr.text())
}

fn describe(name: &str, status: Option<ExitStatus>, stderr: &str) -> String {
    let how = match status.and_then(|s| s.code()) {
        Some(code) => format!("{name} exited with code {code}"),
        None => format!("{name} stopped"),
    };
    if stderr.is_empty() {
        format!("{how}.")
    } else {
        format!("{how}: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_last_lines() {
        let tail = Tail::default();
        for n in 0..10 {
            tail.push(&format!("line {n}"));
        }
        tail.push("   ");
        let text = tail.text();
        assert!(text.starts_with("line 4"), "{text}");
        assert!(text.ends_with("line 9"));
        assert_eq!(describe("claude", None, ""), "claude stopped.");
        assert_eq!(describe("codex", None, "boom"), "codex stopped: boom");
    }
}
