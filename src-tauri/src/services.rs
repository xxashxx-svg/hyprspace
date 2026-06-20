// Runs a startup task as a BACKGROUND process (no PTY/pane) and streams its stdout+stderr to the
// frontend as log lines. Used for the "Run in background" option so a dev server / watcher runs
// without taking a terminal pane, while its output is still viewable in the Services panel.
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::ipc::Channel;

#[derive(Default)]
pub struct ServiceManager {
    procs: Mutex<HashMap<String, Child>>,
}

// frontend sentinel: a log line equal to this means the process ended
const EXIT_MARK: &str = "\u{0}__service_exit__";

impl ServiceManager {
    fn procs(&self) -> MutexGuard<'_, HashMap<String, Child>> {
        self.procs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn kill(&self, mut child: Child) {
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

    fn reap(&self, id: &str) {
        if let Some(child) = self.procs().remove(id) {
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

        if let Some(err) = stderr {
            let che = ch.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = che.send(line);
                }
            });
        }
        // stdout EOF ≈ process ending → emit the exit sentinel so the UI marks it stopped
        std::thread::spawn(move || {
            if let Some(out) = stdout {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let _ = ch.send(line);
                }
            }
            let _ = ch.send(EXIT_MARK.to_string());
        });

        if let Some(old) = self.procs().insert(id, child) {
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
