// Runs a startup task as a BACKGROUND process (no PTY/pane) and streams its stdout+stderr to the
// frontend as log lines. Used for the "Run in background" option so a dev server / watcher runs
// without taking a terminal pane, while its output is still viewable in the Services panel.
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::ipc::Channel;

#[derive(Default, Clone)]
pub struct ServiceManager {
    procs: Arc<Mutex<HashMap<String, Child>>>,
}

// frontend sentinel: a log line equal to this means the process ended
const EXIT_MARK: &str = "\u{0}__service_exit__";

// a chatty dev server can emit thousands of lines/sec; one Channel::send per line means one
// webview IPC hop per line. batch lines for up to this long (or count) and send them joined
// with '\n' — the frontend splits. keystroke-latency doesn't matter for background logs.
const BATCH_MS: u64 = 30;
const BATCH_LINES: usize = 256;

fn spawn_line_batcher(ch: Channel<String>) -> SyncSender<String> {
    let (tx, rx) = sync_channel::<String>(4096);
    std::thread::spawn(move || {
        let mut buf: Vec<String> = Vec::new();
        let flush = |buf: &mut Vec<String>| {
            if !buf.is_empty() {
                let _ = ch.send(buf.join("\n"));
                buf.clear();
            }
        };
        loop {
            match rx.recv_timeout(Duration::from_millis(BATCH_MS)) {
                Ok(line) => {
                    buf.push(line);
                    if buf.len() >= BATCH_LINES {
                        flush(&mut buf);
                    }
                }
                Err(RecvTimeoutError::Timeout) => flush(&mut buf),
                Err(RecvTimeoutError::Disconnected) => {
                    flush(&mut buf);
                    break;
                }
            }
        }
    });
    tx
}

impl ServiceManager {
    fn procs(&self) -> MutexGuard<'_, HashMap<String, Child>> {
        self.procs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn kill(&self, mut child: Child) {
        // already exited? just reap it — dodges the PID-reuse window before taskkill runs
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

    // remove under a short lock, kill after the guard drops — taskkill blocks for the whole
    // process-tree teardown and must not hold the map (or every other service op) hostage
    fn reap(&self, id: &str) {
        let child = self.procs().remove(id);
        if let Some(child) = child {
            self.kill(child);
        }
    }

    // spawn `<shell> -Command "<command>"` in the background, streaming each output line to the channel
    pub fn start(
        &self,
        id: String,
        cwd: String,
        command: String,
        env: HashMap<String, String>,
        ch: Channel<String>,
    ) -> Result<(), String> {
        self.reap(&id);

        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-Command", &command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &command]);
            c
        };
        if !cwd.is_empty() {
            cmd.current_dir(&cwd);
        }
        for (k, v) in &env {
            cmd.env(k, v);
        }
        // run dropped scripts like a double-click: don't inherit a hardened cwd-exclusion that would
        // stop a .bat from finding an .exe sitting next to it (e.g. `cd /d <dir> & app.exe`)
        cmd.env_remove("NoDefaultCurrentDirectoryInExePath");
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // both streams feed one batcher so ordering is preserved and IPC is coalesced
        let batch = spawn_line_batcher(ch);
        if let Some(err) = stderr {
            let btx = batch.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if btx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        // stdout EOF ≈ process ending → emit the exit sentinel so the UI marks it stopped
        std::thread::spawn(move || {
            if let Some(out) = stdout {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if batch.send(line).is_err() {
                        break;
                    }
                }
            }
            let _ = batch.send(EXIT_MARK.to_string());
        });

        let old = self.procs().insert(id, child);
        if let Some(old) = old {
            self.kill(old);
        }
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
