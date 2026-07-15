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
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::ipc::Channel;

struct Proc {
    child: Child,
    // stdin sits behind its own lock so `turn` can write WITHOUT holding the process map —
    // a child that stops draining stdin must only block its own turn, never chat_stop/start.
    stdin: Arc<Mutex<Option<ChildStdin>>>,
}

// kill a process tree (cmd → claude) and reap it, closing stdin first so claude can wind down
fn kill_proc(proc: Proc) {
    let Proc { mut child, stdin } = proc;
    // EOF on stdin → claude winds down. take() so a concurrent writer holding the stdin lock
    // just finds it gone instead of us blocking here.
    if let Ok(mut s) = stdin.try_lock() {
        drop(s.take());
    }
    // already gone? just reap it — dodges taskkill racing a reused PID
    if let Ok(Some(_)) = child.try_wait() {
        return;
    }
    #[cfg(windows)]
    {
        let mut tk = Command::new("taskkill");
        tk.args(["/PID", &child.id().to_string(), "/T", "/F"]);
        tk.creation_flags(0x08000000);
        let ok = tk.output().map(|o| o.status.success()).unwrap_or(false);
        // taskkill failed or it's somehow still alive → fall back to a direct kill
        if !ok || matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[derive(Default, Clone)]
pub struct ChatManager {
    procs: Arc<Mutex<HashMap<String, Proc>>>,
}

impl ChatManager {
    // the guarded data is just a process map — recovering a poisoned lock is always safe, and one
    // panic elsewhere must not wedge chat I/O for the rest of the session.
    fn procs(&self) -> MutexGuard<'_, HashMap<String, Proc>> {
        self.procs.lock().unwrap_or_else(|e| e.into_inner())
    }

    // kill + reap the process under this id (closing its stdin first so claude can exit cleanly).
    // remove under a short lock, kill AFTER the guard drops — taskkill can take hundreds of ms
    // and holding the map the whole time would block every other chat op behind it.
    fn reap(&self, id: &str) {
        let proc = self.procs().remove(id);
        if let Some(proc) = proc {
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
        let old = self
            .procs()
            .insert(id, Proc { child, stdin: Arc::new(Mutex::new(Some(stdin))) });
        if let Some(old) = old {
            kill_proc(old);
        }
        Ok(())
    }

    // write one user-message JSON envelope (a single line) to the live process's stdin.
    // clone the stdin handle out of the map and drop the map guard BEFORE the blocking write —
    // a wedged child must not hold the process map hostage (chat_stop needs it to kill him).
    pub fn turn(&self, id: &str, message: String) -> Result<(), String> {
        let stdin = {
            let guard = self.procs();
            guard.get(id).ok_or("no live session")?.stdin.clone()
        };
        let mut slot = stdin.lock().map_err(|_| "stdin lock poisoned".to_string())?;
        let sin = slot.as_mut().ok_or("session closing")?;
        let mut line = message;
        line.push('\n');
        sin.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        sin.flush().map_err(|e| e.to_string())?;
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
