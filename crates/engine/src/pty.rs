// Terminal sessions on portable-pty, copied from src-tauri/src/pty.rs. The coalescer, the
// off-thread child wait, the off-thread drop on kill and `kill_all` carry over unchanged.
// Left behind: the Tauri channel (output goes out as engine events), the mobile tap and replay
// buffer (the phone bridge keeps its own, engine/src/phone), the xterm pause gate (the emulator now runs in-process),
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
    pub env: Vec<(String, String)>,
    /// A command typed into the shell once it first prints, the way the Tauri app launches
    /// agents: as keystrokes, so the user's own shell profile and PATH apply.
    pub input: Option<String>,
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: SharedWriter,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// The shell's process id, where the processes it starts hang from.
    pid: Option<u32>,
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

        let mut cmd = match spawn.shell {
            Some(shell) => CommandBuilder::new(shell),
            None => {
                let mut cmd = CommandBuilder::new(default_shell());
                // PowerShell's copyright banner on every new pane is noise
                if cfg!(windows) {
                    cmd.arg("-NoLogo");
                }
                cmd
            }
        };
        cmd.args(&spawn.args);
        if !spawn.cwd.as_os_str().is_empty() {
            cmd.cwd(&spawn.cwd);
        }
        // GUI-launched apps inherit no TERM, so CLIs (and claude) suppress color without this
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in &spawn.env {
            cmd.env(k, v);
        }

        let mut child = pair.slave.spawn_command(cmd)?;
        // drop the slave right after spawn so the master read sees EOF when the child exits (ConPTY)
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer()?));
        let killer = child.clone_killer();
        let pid = child.process_id();

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
        let mut input = spawn.input.map(|cmd| (writer.clone(), cmd));
        thread::spawn(move || {
            coalesce(rx, |bytes| {
                let _ = data_out.unbounded_send(Event::TerminalOutput { id, bytes });
                // the shell is up enough to buffer keystrokes; it reads them when it is ready
                if let Some((writer, cmd)) = input.take() {
                    let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = w.write_all(format!("{cmd}\r").as_bytes());
                    let _ = w.flush();
                }
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
                pid,
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

    /// Each live session's shell process.
    pub fn pids(&self) -> Vec<(SessionId, u32)> {
        self.sessions()
            .iter()
            .filter_map(|(id, s)| Some((*id, s.pid?)))
            .collect()
    }

    pub fn contains(&self, id: SessionId) -> bool {
        self.sessions().contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.sessions().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The coalescer's state, apart from the clock and the channel so tests can drive it with made-up
/// instants: leading-edge flush for snappy echo, batching under load.
struct Batch {
    acc: Vec<u8>,
    last_flush: Instant,
    interval: Duration,
}

impl Batch {
    fn new(now: Instant) -> Self {
        Self {
            acc: Vec::with_capacity(FLUSH_BYTES),
            last_flush: now,
            interval: Duration::from_millis(FLUSH_MS),
        }
    }

    /// Flush right away if we've been quiet (interactive echo) or hit the size cap; otherwise let
    /// a fast stream keep accumulating until the next tick.
    fn push(&mut self, chunk: &[u8], now: Instant) -> Option<Vec<u8>> {
        self.acc.extend_from_slice(chunk);
        if self.acc.len() >= FLUSH_BYTES || now.duration_since(self.last_flush) >= self.interval {
            return self.take(now);
        }
        None
    }

    /// A tick with nothing new: whatever accumulated goes out.
    fn take(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.acc.is_empty() {
            return None;
        }
        self.last_flush = now;
        let out = std::mem::replace(&mut self.acc, Vec::with_capacity(FLUSH_BYTES));
        Some(out)
    }
}

// Returns when the reader hangs up. With nothing held back it sleeps until output comes; a timed
// wait there woke every quiet terminal sixty times a second for nothing.
fn coalesce(rx: std::sync::mpsc::Receiver<Vec<u8>>, mut emit: impl FnMut(Vec<u8>)) {
    let mut batch = Batch::new(Instant::now());
    loop {
        let next = if batch.acc.is_empty() {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            rx.recv_timeout(batch.interval)
        };
        let out = match next {
            Ok(chunk) => batch.push(&chunk, Instant::now()),
            Err(RecvTimeoutError::Timeout) => batch.take(Instant::now()),
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(rest) = batch.take(Instant::now()) {
                    emit(rest);
                }
                break;
            }
        };
        if let Some(bytes) = out {
            emit(bytes);
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
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut b = Batch::new(t0);
        // a burst inside one frame accumulates
        for i in 0..5 {
            assert_eq!(b.push(b"a", ms(i)), None);
        }
        assert_eq!(b.take(ms(16)), Some(b"aaaaa".to_vec()));
        // after a quiet moment the first byte goes out on its own (keystroke echo)
        assert_eq!(b.push(b"b", ms(80)), Some(b"b".to_vec()));
        for i in 0..5 {
            assert_eq!(b.push(b"c", ms(81 + i)), None);
        }
        assert_eq!(b.take(ms(97)), Some(b"ccccc".to_vec()));
        assert_eq!(b.take(ms(200)), None);
    }

    #[test]
    fn the_size_cap_flushes_without_waiting() {
        let t0 = Instant::now();
        let mut b = Batch::new(t0);
        assert_eq!(
            b.push(&[0; FLUSH_BYTES], t0).map(|v| v.len()),
            Some(FLUSH_BYTES)
        );
        assert_eq!(b.push(&[0; 1], t0), None);
        assert_eq!(b.take(t0).map(|v| v.len()), Some(1));
    }

    #[test]
    fn the_thread_hands_over_every_byte_in_order() {
        let (tx, rx) = sync_channel(16);
        let flushed = thread::spawn(move || {
            let mut out = vec![];
            coalesce(rx, |b| out.push(b));
            out
        });
        for c in [b"a", b"b", b"c"] {
            tx.send(c.to_vec()).unwrap();
        }
        drop(tx);
        assert_eq!(flushed.join().unwrap().concat(), b"abc");
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
