// Runs ONE agent turn for the Loops engine: spawns the provider CLI (e.g. `claude -p …`) with
// structured argv, pipes the prompt over stdin (so a long/multiline prompt never has to survive
// shell quoting), and streams stdout+stderr back as log lines. The frontend loop runner calls this
// once per iteration. Fresh-run loops use this; continue-session loops reuse chat.rs instead.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use tauri::ipc::Channel;

#[derive(Default)]
pub struct AgentManager {
    procs: Mutex<HashMap<String, Child>>,
}

// frontend sentinel: a log line equal to this means the turn's process ended
const EXIT_MARK: &str = "\u{0}__agent_exit__";

impl AgentManager {
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

    // run one turn: `args` is the full argv (args[0] = program, e.g. "claude"); the prompt is fed
    // on stdin and then stdin is closed so the headless run processes it and exits.
    pub fn start(
        &self,
        id: String,
        cwd: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        prompt: String,
        ch: Channel<String>,
    ) -> Result<(), String> {
        self.reap(&id);
        if args.is_empty() {
            return Err("no command".into());
        }

        // Windows: providers are .cmd shims, so go through `cmd /c <argv…>`. Elsewhere spawn directly.
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/c");
            for a in &args {
                c.arg(a);
            }
            c
        } else {
            let mut c = Command::new(&args[0]);
            for a in &args[1..] {
                c.arg(a);
            }
            c
        };
        if !cwd.is_empty() {
            cmd.current_dir(&cwd);
        }
        for (k, v) in &env {
            cmd.env(k, v);
        }
        // behave like a double-click: don't inherit a hardened cwd-exclusion (the lualink lesson)
        cmd.env_remove("NoDefaultCurrentDirectoryInExePath");
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;

        // feed the prompt, then drop the handle to send EOF
        if let Some(mut sin) = child.stdin.take() {
            std::thread::spawn(move || {
                let _ = sin.write_all(prompt.as_bytes());
            });
        }

        let stdout = child.stdout.take();
        if let Some(err) = child.stderr.take() {
            let che = ch.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    let _ = che.send(line);
                }
            });
        }
        // stdout EOF ≈ the turn ending → emit the exit sentinel so the runner advances the loop
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
