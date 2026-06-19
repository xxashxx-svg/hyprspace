// Drives the `claude` CLI in stream-json mode over piped stdio (NOT a PTY) so we get clean
// JSONL events to render as a chat. Runs on the user's subscription (it just spawns their
// already-logged-in CLI — same as the terminal panes; no API key, no tokens touched).
//
// Persistent session: one long-lived process per thread. We keep its stdin open and feed each
// user turn as a stream-json message (--input-format stream-json), reading a continuous event
// stream back. A turn is "done" when the `result` event arrives; the process stays alive for the
// next turn (no respawn, no --resume between turns). Process exit emits an "exit" sentinel.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::ipc::Channel;

struct Proc {
    child: Child,
    stdin: ChildStdin,
}

// kill a process tree (cmd → claude) and reap it, closing stdin first so claude can wind down
fn kill_proc(proc: Proc) {
    let Proc { mut child, stdin } = proc;
    drop(stdin); // EOF on stdin → claude winds down
    #[cfg(windows)]
    {
        let mut tk = Command::new("taskkill");
        tk.args(["/PID", &child.id().to_string(), "/T", "/F"]);
        tk.creation_flags(0x08000000);
        let _ = tk.output();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[derive(Default)]
pub struct ChatManager {
    procs: Mutex<HashMap<String, Proc>>,
}

impl ChatManager {
    // the guarded data is just a process map — recovering a poisoned lock is always safe, and one
    // panic elsewhere must not wedge chat I/O for the rest of the session.
    fn procs(&self) -> MutexGuard<'_, HashMap<String, Proc>> {
        self.procs.lock().unwrap_or_else(|e| e.into_inner())
    }

    // kill + reap the process under this id (closing its stdin first so claude can exit cleanly)
    fn reap(&self, id: &str) {
        if let Some(proc) = self.procs().remove(id) {
            kill_proc(proc);
        }
    }

    // spawn a persistent `claude <args>` (input-format stream-json), keep stdin open, and stream
    // every stdout line to the channel for the life of the process.
    pub fn start(
        &self,
        id: String,
        cwd: String,
        args: Vec<String>,
        ch: Channel<String>,
    ) -> Result<(), String> {
        self.reap(&id);

        // claude is often a .cmd shim on Windows → go through cmd so PATHEXT resolves it
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/c").arg("claude").args(&args);
            c
        } else {
            let mut c = Command::new("claude");
            c.args(&args);
            c
        };
        if !cwd.is_empty() {
            cmd.current_dir(&cwd);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = child.stderr.take();

        // stderr → JSON-wrapped lines so the UI can surface auth/errors
        if let Some(stderr) = stderr {
            let che = ch.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        let _ = che.send(
                            serde_json::json!({ "type": "stderr", "text": line }).to_string(),
                        );
                    }
                }
            });
        }
        // stdout (claude's JSONL events) → forward verbatim. A turn ends on the "result" event;
        // when the process itself exits (stdout EOF) we emit an "exit" sentinel so the UI knows
        // the session is gone and the next turn must respawn.
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    let _ = ch.send(line);
                }
            }
            let _ = ch.send("{\"type\":\"exit\"}".to_string());
        });

        // if some earlier process was still mapped under this id, reap it so it can't orphan
        if let Some(old) = self.procs().insert(id, Proc { child, stdin }) {
            kill_proc(old);
        }
        Ok(())
    }

    // write one user-message JSON envelope (a single line) to the live process's stdin
    pub fn turn(&self, id: &str, message: String) -> Result<(), String> {
        let mut guard = self.procs();
        let proc = guard.get_mut(id).ok_or("no live session")?;
        let mut line = message;
        line.push('\n');
        proc.stdin
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        proc.stdin.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stop(&self, id: &str) {
        self.reap(id);
    }

    pub fn kill_all(&self) {
        let ids: Vec<String> = self.procs().keys().cloned().collect();
        for id in ids {
            self.reap(&id);
        }
    }
}
