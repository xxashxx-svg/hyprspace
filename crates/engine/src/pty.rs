// Terminal sessions on portable-pty, copied from src-tauri/src/pty.rs. The coalescer, the
// off-thread child wait, the off-thread drop on kill and `kill_all` carry over unchanged.
// Left behind: the Tauri channel (output goes out as engine events), the mobile tap and replay
// buffer (the phone app is redone later), the xterm pause gate (the emulator now runs in-process),
// and the headless query answerer (the UI's emulator answers terminal queries itself).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::{Event, SessionId};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

// Coalescer tuning. Leading-edge: the first bytes after a quiet moment flush at once for snappy
// keystroke echo; only a sustained firehose batches (to FLUSH_MS or FLUSH_BYTES).
//
// FLUSH_MS is one frame, deliberately. The view cannot paint more often than that, so delivering
// faster buys nothing a user can see while costing a wakeup and a parse each time. Echo is
// unaffected: after a quiet moment `last_flush.elapsed()` is already past the interval, so the
// first keystroke still goes out on the spot.
const FLUSH_MS: u64 = 16;
const FLUSH_BYTES: usize = 16 * 1024;
const READ_BUF: usize = 64 * 1024;

/// What to run. `shell: None` means the platform default.
#[derive(Debug, Clone, Default)]
pub struct Spawn {
    pub cwd: PathBuf,
    pub shell: Option<String>,
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: SharedWriter,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default, Clone)]
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<SessionId, Session>>>,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl PtyManager {
    // the guarded data is just a session map, so recovering a poisoned lock is always safe, and
    // one panic elsewhere must not brick PTY I/O for the rest of the session
    fn sessions(&self) -> MutexGuard<'_, HashMap<SessionId, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Spawns the shell and streams its output to `out` as `TerminalOutput`, then one
    /// `TerminalExit`.
    pub fn create(
        &self,
        id: SessionId,
        spawn: Spawn,
        out: UnboundedSender<Event>,
    ) -> anyhow::Result<()> {
        let pair = native_pty_system().openpty(size(spawn.cols, spawn.rows))?;

        let mut cmd = CommandBuilder::new(spawn.shell.unwrap_or_else(default_shell));
        cmd.args(&spawn.args);
        if !spawn.cwd.as_os_str().is_empty() {
            cmd.cwd(&spawn.cwd);
        }
        // GUI-launched apps inherit no TERM, so CLIs (and claude) suppress color without this
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let mut child = pair.slave.spawn_command(cmd)?;
        // drop the slave right after spawn so the master read sees EOF when the child exits (ConPTY)
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer()?));
        let killer = child.clone_killer();

        // reader thread -> bounded channel (blocking = real backpressure, no byte drops) -> coalescer
        let (tx, rx) = sync_channel::<Vec<u8>>(256);
        thread::spawn(move || {
            let mut buf = [0u8; READ_BUF];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let data_out = out.clone();
        thread::spawn(move || {
            coalesce(rx, |bytes| {
                let _ = data_out.unbounded_send(Event::TerminalOutput { id, bytes });
            })
        });

        // child.wait() off-thread: PseudoConsoleClose can block
        thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code() as i32).unwrap_or(-1);
            let _ = out.unbounded_send(Event::TerminalExit { id, code });
        });

        self.sessions().insert(
            id,
            Session {
                master: pair.master,
                writer,
                killer,
            },
        );
        Ok(())
    }

    pub fn write(&self, id: SessionId, data: &[u8]) -> std::io::Result<()> {
        // clone the writer and drop the map lock first, so a slow write never blocks other sessions
        let Some(writer) = self.sessions().get(&id).map(|s| s.writer.clone()) else {
            return Ok(());
        };
        let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
        w.write_all(data)?;
        w.flush()
    }

    pub fn resize(&self, id: SessionId, cols: u16, rows: u16) -> anyhow::Result<()> {
        if let Some(s) = self.sessions().get(&id) {
            s.master.resize(size(cols, rows))?;
        }
        Ok(())
    }

    pub fn kill(&self, id: SessionId) {
        if let Some(mut s) = self.sessions().remove(&id) {
            let _ = s.killer.kill();
            // dropping the Session drops its master PTY, which closes the pseudoconsole. That can
            // BLOCK until the attached process tree detaches, so a slow-to-exit child (an
            // interactive claude) would stall the caller. Drop it off-thread.
            thread::spawn(move || drop(s));
        }
    }

    /// Kill every live session at once. Call on app exit, or the ConPTY host processes
    /// (OpenConsole.exe) orphan and busy-spin at about 8% CPU each. Removing each Session also
    /// drops its master PTY, which closes the pseudoconsole and ends the host.
    pub fn kill_all(&self) {
        for (_, mut s) in self.sessions().drain() {
            let _ = s.killer.kill();
        }
    }

    pub fn len(&self) -> usize {
        self.sessions().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// Leading-edge flush for snappy echo, batching under load. Returns when the reader hangs up.
fn coalesce(rx: std::sync::mpsc::Receiver<Vec<u8>>, mut emit: impl FnMut(Vec<u8>)) {
    let mut acc: Vec<u8> = Vec::with_capacity(FLUSH_BYTES);
    let interval = Duration::from_millis(FLUSH_MS);
    let mut last_flush = Instant::now();
    loop {
        match rx.recv_timeout(interval) {
            Ok(chunk) => {
                acc.extend_from_slice(&chunk);
                // flush right away if we've been quiet (interactive echo) or hit the size cap;
                // otherwise let a fast stream keep accumulating until the next tick
                if acc.len() >= FLUSH_BYTES || last_flush.elapsed() >= interval {
                    emit(std::mem::take(&mut acc));
                    acc.reserve(FLUSH_BYTES);
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if !acc.is_empty() {
                    emit(std::mem::take(&mut acc));
                    acc.reserve(FLUSH_BYTES);
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if !acc.is_empty() {
                    emit(acc);
                }
                break;
            }
        }
    }
}

fn default_shell() -> String {
    if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use futures::channel::mpsc;
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn a_burst_batches_and_a_chunk_after_a_pause_flushes_alone() {
        let (tx, rx) = sync_channel(16);
        let flushed = thread::spawn(move || {
            let mut out = vec![];
            coalesce(rx, |b| out.push(b));
            out
        });
        for _ in 0..5 {
            tx.send(b"a".to_vec()).unwrap();
        }
        thread::sleep(Duration::from_millis(60));
        tx.send(b"b".to_vec()).unwrap();
        for _ in 0..5 {
            tx.send(b"c".to_vec()).unwrap();
        }
        drop(tx);
        let out = flushed.join().unwrap();
        assert_eq!(out.concat(), b"aaaaabccccc");
        assert!(out.contains(&b"b".to_vec()), "{out:?}");
        assert!(out.len() < 11, "{out:?}");
    }

    #[test]
    fn the_size_cap_flushes_without_waiting() {
        let (tx, rx) = sync_channel(16);
        let flushed = thread::spawn(move || {
            let mut out = vec![];
            coalesce(rx, |b| out.push(b.len()));
            out
        });
        tx.send(vec![0; FLUSH_BYTES]).unwrap();
        tx.send(vec![0; 1]).unwrap();
        drop(tx);
        assert_eq!(flushed.join().unwrap(), [FLUSH_BYTES, 1]);
    }

    fn echo() -> Spawn {
        let (shell, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/c", "echo hyprspace-pty"])
        } else {
            ("/bin/sh", vec!["-c", "echo hyprspace-pty"])
        };
        Spawn {
            shell: Some(shell.into()),
            args: args.into_iter().map(String::from).collect(),
            cols: 80,
            rows: 24,
            ..Default::default()
        }
    }

    // Collects output until the marker shows up and the child has exited.
    fn run(ptys: &PtyManager, id: SessionId, spawn: Spawn) -> (String, Option<i32>) {
        let (tx, mut rx) = mpsc::unbounded();
        ptys.create(id, spawn, tx).unwrap();
        let seen = Arc::new(Mutex::new((Vec::new(), None)));
        let sink = seen.clone();
        let answer = ptys.clone();
        thread::spawn(move || {
            while let Some(e) = block_on(rx.next()) {
                let mut s = sink.lock().unwrap();
                match e {
                    Event::TerminalOutput { bytes, .. } => {
                        // ConPTY asks where the cursor is and waits for the answer, which the
                        // UI's emulator gives in the app
                        if bytes.windows(4).any(|w| w == b"[6n") {
                            answer.write(id, b"[1;1R").unwrap();
                        }
                        s.0.extend(bytes)
                    }
                    Event::TerminalExit { code, .. } => s.1 = Some(code),
                    _ => {}
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let (text, code) = {
                let s = seen.lock().unwrap();
                (String::from_utf8_lossy(&s.0).to_string(), s.1)
            };
            if (code.is_some() && text.contains("hyprspace-pty")) || Instant::now() > deadline {
                return (text, code);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_a_command_and_reports_its_exit() {
        let ptys = PtyManager::default();
        let (text, code) = run(&ptys, SessionId(1), echo());
        assert!(text.contains("hyprspace-pty"), "{text:?} {code:?}");
        assert_eq!(code, Some(0));
        ptys.kill_all();
    }

    #[test]
    fn kill_all_ends_every_session() {
        let ptys = PtyManager::default();
        let long = |id| {
            let (shell, args) = if cfg!(windows) {
                ("cmd.exe", vec!["/k"])
            } else {
                ("/bin/sh", vec![])
            };
            let (tx, mut rx) = mpsc::unbounded();
            let spawn = Spawn {
                shell: Some(shell.into()),
                args: args.into_iter().map(String::from).collect(),
                cols: 80,
                rows: 24,
                ..Default::default()
            };
            ptys.create(SessionId(id), spawn, tx).unwrap();
            thread::spawn(move || {
                while let Some(e) = block_on(rx.next()) {
                    if matches!(e, Event::TerminalExit { .. }) {
                        return true;
                    }
                }
                false
            })
        };
        let a = long(1);
        let b = long(2);
        assert_eq!(ptys.len(), 2);
        ptys.kill_all();
        assert!(ptys.is_empty());
        assert!(a.join().unwrap());
        assert!(b.join().unwrap());
    }

    #[test]
    fn writes_to_an_unknown_session_are_ignored() {
        let ptys = PtyManager::default();
        assert!(ptys.write(SessionId(9), b"x").is_ok());
        assert!(ptys.resize(SessionId(9), 10, 10).is_ok());
        ptys.kill(SessionId(9));
    }
}
