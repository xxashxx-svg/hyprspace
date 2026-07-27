// In-process PTY backend: spawn shells/commands via portable-pty, stream coalesced
// raw bytes to the frontend over a per-session Tauri Channel. Slice 1 keeps it in one
// module; later slices split it into pty/{session,coalescer,spawn,env}.rs per the blueprint.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};

// coalescer tuning — leading-edge: the first bytes after a quiet moment flush immediately for
// snappy keystroke echo; only a sustained firehose batches (to <= FLUSH_MS or FLUSH_BYTES).
const FLUSH_MS: u64 = 4;
const FLUSH_BYTES: usize = 16 * 1024;
const READ_BUF: usize = 64 * 1024;
// per-session replay buffer for the mobile bridge: a phone that subscribes mid-session needs
// enough recent bytes to redraw a screen. one screen of a TUI is a few KB, so 64K is generous.
const SCROLLBACK_BYTES: usize = 64 * 1024;

// control messages go over the SAME channel as Json; raw output goes as Raw (ArrayBuffer in JS)
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[allow(dead_code)] // Error variant reserved for spawn/runtime failures (used in later slices)
enum Control {
    Exit { code: i32 },
    Error { message: String },
}

fn send_ctrl(ch: &Channel<InvokeResponseBody>, c: &Control) {
    if let Ok(s) = serde_json::to_string(c) {
        let _ = ch.send(InvokeResponseBody::Json(s));
    }
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

// xterm flow control: when the frontend's write buffer backs up it pauses us; the reader thread
// parks here instead of reading, the kernel PTY buffer fills, and the child blocks on write —
// real end-to-end backpressure instead of unbounded queueing in the webview.
struct PauseGate {
    paused: Mutex<bool>,
    cv: Condvar,
}

impl PauseGate {
    fn new() -> Arc<Self> {
        Arc::new(PauseGate { paused: Mutex::new(false), cv: Condvar::new() })
    }
    fn set(&self, v: bool) {
        *self.paused.lock().unwrap_or_else(|e| e.into_inner()) = v;
        self.cv.notify_all();
    }
    // block while paused; returns immediately when running
    fn wait_if_paused(&self) {
        let mut p = self.paused.lock().unwrap_or_else(|e| e.into_inner());
        while *p {
            p = self.cv.wait(p).unwrap_or_else(|e| e.into_inner());
        }
    }
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: SharedWriter, // shared so the reader thread can answer terminal queries (headless mode)
    killer: Box<dyn ChildKiller + Send + Sync>,
    gate: Arc<PauseGate>,
    replay: Replay, // recent bytes + current size, so a mobile peer can redraw on subscribe
}

/// What the bridge (mobile mirror) hears about a session. Data is already coalesced.
pub enum Tap<'a> {
    Data(&'a [u8]),
    Size(u16, u16),
    Exit(i32),
}

pub type TapFn = Arc<dyn Fn(&str, Tap<'_>) + Send + Sync>;

/// Rolling tail of a session's output plus its live size. Cheap enough to always keep — it's one
/// extend_from_slice on bytes we're already copying for the frontend channel.
#[derive(Clone, Default)]
struct Replay {
    buf: Arc<Mutex<Vec<u8>>>,
    size: Arc<Mutex<(u16, u16)>>,
}

impl Replay {
    fn push(&self, chunk: &[u8]) {
        let mut b = self.buf.lock().unwrap_or_else(|e| e.into_inner());
        b.extend_from_slice(chunk);
        if b.len() > SCROLLBACK_BYTES {
            // drop the oldest half rather than one byte at a time, and cut at the next newline so
            // the replay doesn't start mid-escape-sequence and paint garbage on the phone
            let mut cut = b.len() - SCROLLBACK_BYTES / 2;
            if let Some(nl) = b[cut..].iter().position(|&c| c == b'\n') {
                cut += nl + 1;
            }
            b.drain(..cut);
        }
    }
}

// Headless PTY mode (no xterm): a TUI like claude's emits terminal queries on startup — cursor
// position (`ESC[6n`), device attributes (`ESC[c` / `ESC[>c`), status (`ESC[5n`) — and HANGS waiting
// for the reply that a real terminal emulator would send. We answer them here so it can boot. We
// also auto-confirm the one-time folder-trust prompt as a fallback (--dangerously-skip-permissions
// normally skips it). `trust_sent` is per-session so we only confirm once.
fn answer_terminal_queries(chunk: &[u8], writer: &SharedWriter, trust_sent: &mut bool) {
    // queries only occur around TUI startup; once trust is confirmed, a chunk with no ESC
    // byte can't contain one — skip the utf8 conversion + substring scans on the firehose
    if *trust_sent && !chunk.contains(&0x1b) {
        return;
    }
    let s = String::from_utf8_lossy(chunk);
    let mut resp: Vec<u8> = Vec::new();
    if s.contains("\x1b[6n") {
        resp.extend_from_slice(b"\x1b[1;1R");
    }
    if s.contains("\x1b[5n") {
        resp.extend_from_slice(b"\x1b[0n");
    }
    if s.contains("\x1b[>c") || s.contains("\x1b[>0c") {
        resp.extend_from_slice(b"\x1b[>0;10;1c");
    } else if s.contains("\x1b[c") || s.contains("\x1b[0c") {
        resp.extend_from_slice(b"\x1b[?1;2c");
    }
    if !*trust_sent && (s.contains("trust this folder") || s.contains("Is this a project")) {
        *trust_sent = true;
        resp.extend_from_slice(b"\r");
    }
    if !resp.is_empty() {
        if let Ok(mut w) = writer.lock() {
            let _ = w.write_all(&resp);
            let _ = w.flush();
        }
    }
}

#[derive(Default, Clone)]
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    tap: Arc<Mutex<Option<TapFn>>>,
}

impl PtyManager {
    // the guarded data is just a session map — recovering a poisoned lock is always safe, and one
    // panic elsewhere must not brick PTY I/O for the rest of the session.
    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Register the single listener that mirrors every session's output (the mobile bridge).
    pub fn set_tap(&self, f: TapFn) {
        *self.tap.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    fn tapped(&self) -> Option<TapFn> {
        self.tap.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Recent output + current size for a session — what a phone needs to draw the pane on subscribe.
    pub fn replay(&self, id: &str) -> Option<(Vec<u8>, u16, u16)> {
        let r = self.sessions().get(id)?.replay.clone();
        let buf = r.buf.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let (cols, rows) = *r.size.lock().unwrap_or_else(|e| e.into_inner());
        Some((buf, cols, rows))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        id: String,
        cwd: String,
        shell: Option<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
        cols: u16,
        rows: u16,
        on_event: Channel<InvokeResponseBody>,
        auto_respond: bool, // headless callers (the Loops claude-hooks PTY) answer TUI queries themselves
    ) -> Result<(), String> {
        let sys = native_pty_system();
        let pair = sys
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;

        let prog = shell.unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(&prog);
        for a in &args {
            cmd.arg(a);
        }
        if !cwd.is_empty() {
            cmd.cwd(&cwd);
        }
        // advertise a color-capable terminal — GUI-launched apps inherit no TERM, so
        // CLIs (and Claude) suppress color on macOS/Linux without this. Caller env wins.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in &env {
            cmd.env(k, v);
        }

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        // drop the slave right after spawn so the master read sees EOF when the child exits (ConPTY)
        drop(pair.slave);

        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| e.to_string())?));
        let killer = child.clone_killer();

        // reader thread -> bounded channel (blocking = real backpressure, no byte drops) -> coalescer
        let (tx, rx) = sync_channel::<Vec<u8>>(256);
        let resp_writer = if auto_respond { Some(writer.clone()) } else { None };
        let gate = PauseGate::new();
        let reader_gate = gate.clone();
        thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; READ_BUF];
            let mut trust_sent = false;
            loop {
                reader_gate.wait_if_paused();
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Some(w) = &resp_writer {
                            answer_terminal_queries(&buf[..n], w, &mut trust_sent);
                        }
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // coalescer thread: leading-edge flush (snappy echo) + batching under load
        let data_ch = on_event.clone();
        let replay = Replay { buf: Arc::new(Mutex::new(Vec::new())), size: Arc::new(Mutex::new((cols, rows))) };
        let co_replay = replay.clone();
        let co_tap = self.tapped();
        let co_id = id.clone();
        thread::spawn(move || {
            let mut acc: Vec<u8> = Vec::with_capacity(FLUSH_BYTES);
            let interval = Duration::from_millis(FLUSH_MS);
            let mut last_flush = Instant::now();
            // one place to fan a flushed batch out: frontend channel, replay tail, mobile bridge
            let emit = |batch: Vec<u8>| {
                co_replay.push(&batch);
                if let Some(t) = &co_tap {
                    t(&co_id, Tap::Data(&batch));
                }
                let _ = data_ch.send(InvokeResponseBody::Raw(batch));
            };
            loop {
                match rx.recv_timeout(interval) {
                    Ok(chunk) => {
                        acc.extend_from_slice(&chunk);
                        // flush right away if we've been quiet (interactive echo) or hit the size cap;
                        // otherwise let a fast stream keep accumulating until the next tick.
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
        });

        // wait thread: off-thread child.wait() (PseudoConsoleClose can block), then emit exit
        let exit_ch = on_event.clone();
        let exit_tap = self.tapped();
        let exit_id = id.clone();
        thread::spawn(move || {
            let code = match child.wait() {
                Ok(status) => status.exit_code() as i32,
                Err(_) => -1,
            };
            if let Some(t) = &exit_tap {
                t(&exit_id, Tap::Exit(code));
            }
            send_ctrl(&exit_ch, &Control::Exit { code });
        });

        self.sessions()
            .insert(id, Session { master: pair.master, writer, killer, gate, replay });
        Ok(())
    }

    // frontend flow control: xterm's write buffer crossed its high-water mark → stop reading the
    // PTY until resumed. Unknown ids are fine (session died while the pause was in flight).
    pub fn set_paused(&self, id: &str, paused: bool) {
        if let Some(s) = self.sessions().get(id) {
            s.gate.set(paused);
        }
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), String> {
        // clone the Arc and drop the sessions lock before writing, so a slow write never blocks
        // other PTY ops (and the reader thread can answer queries concurrently).
        let writer = {
            let g = self.sessions();
            g.get(id).ok_or("no such session")?.writer.clone()
        };
        let mut w = writer.lock().map_err(|_| "writer lock poisoned".to_string())?;
        w.write_all(data).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        {
            let g = self.sessions();
            let s = g.get(id).ok_or("no such session")?;
            s.master
                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                .map_err(|e| e.to_string())?;
            *s.replay.size.lock().unwrap_or_else(|e| e.into_inner()) = (cols, rows);
        }
        // mobile mirrors the desktop's geometry rather than resizing the PTY itself, so it needs to
        // hear about this. notify outside the sessions lock — the tap fans out to network peers.
        if let Some(t) = self.tapped() {
            t(id, Tap::Size(cols, rows));
        }
        Ok(())
    }

    pub fn kill(&self, id: &str) -> Result<(), String> {
        if let Some(mut s) = self.sessions().remove(id) {
            s.gate.set(false); // wake a paused reader so it can see EOF and exit
            let _ = s.killer.kill();
            // dropping the Session drops its master PTY → ClosePseudoConsole, which can BLOCK until
            // the attached process tree detaches. kill_pty runs on the UI thread, so do the drop
            // off-thread or a slow-to-exit child (e.g. an interactive claude) freezes the app.
            std::thread::spawn(move || drop(s));
        }
        Ok(())
    }

    // Kill every live session at once — called on app exit so the ConPTY host processes
    // (OpenConsole.exe) don't orphan and busy-spin at ~8% CPU each. Removing each Session also
    // drops its master PTY, which closes the pseudoconsole (ClosePseudoConsole) and ends the host.
    pub fn kill_all(&self) {
        for (_, mut s) in self.sessions().drain() {
            s.gate.set(false);
            let _ = s.killer.kill();
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
