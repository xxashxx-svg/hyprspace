// In-process PTY backend: spawn shells/commands via portable-pty, stream coalesced
// raw bytes to the frontend over a per-session Tauri Channel. Slice 1 keeps it in one
// module; later slices split it into pty/{session,coalescer,spawn,env}.rs per the blueprint.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
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

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, Session>>,
}

impl PtyManager {
    // the guarded data is just a session map — recovering a poisoned lock is always safe, and one
    // panic elsewhere must not brick PTY I/O for the rest of the session.
    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
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
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let killer = child.clone_killer();

        // reader thread -> bounded channel (blocking = real backpressure, no byte drops) -> coalescer
        let (tx, rx) = sync_channel::<Vec<u8>>(256);
        thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; READ_BUF];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
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
        thread::spawn(move || {
            let mut acc: Vec<u8> = Vec::with_capacity(FLUSH_BYTES);
            let interval = Duration::from_millis(FLUSH_MS);
            let mut last_flush = Instant::now();
            loop {
                match rx.recv_timeout(interval) {
                    Ok(chunk) => {
                        acc.extend_from_slice(&chunk);
                        // flush right away if we've been quiet (interactive echo) or hit the size cap;
                        // otherwise let a fast stream keep accumulating until the next tick.
                        if acc.len() >= FLUSH_BYTES || last_flush.elapsed() >= interval {
                            let _ = data_ch.send(InvokeResponseBody::Raw(std::mem::take(&mut acc)));
                            acc.reserve(FLUSH_BYTES);
                            last_flush = Instant::now();
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if !acc.is_empty() {
                            let _ = data_ch.send(InvokeResponseBody::Raw(std::mem::take(&mut acc)));
                            acc.reserve(FLUSH_BYTES);
                            last_flush = Instant::now();
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if !acc.is_empty() {
                            let _ = data_ch.send(InvokeResponseBody::Raw(acc));
                        }
                        break;
                    }
                }
            }
        });

        // wait thread: off-thread child.wait() (PseudoConsoleClose can block), then emit exit
        let exit_ch = on_event.clone();
        thread::spawn(move || {
            let code = match child.wait() {
                Ok(status) => status.exit_code() as i32,
                Err(_) => -1,
            };
            send_ctrl(&exit_ch, &Control::Exit { code });
        });

        self.sessions()
            .insert(id, Session { master: pair.master, writer, killer });
        Ok(())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), String> {
        let mut g = self.sessions();
        let s = g.get_mut(id).ok_or("no such session")?;
        s.writer.write_all(data).map_err(|e| e.to_string())?;
        s.writer.flush().map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let g = self.sessions();
        let s = g.get(id).ok_or("no such session")?;
        s.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())
    }

    pub fn kill(&self, id: &str) -> Result<(), String> {
        if let Some(mut s) = self.sessions().remove(id) {
            let _ = s.killer.kill();
        }
        Ok(())
    }

    // Kill every live session at once — called on app exit so the ConPTY host processes
    // (OpenConsole.exe) don't orphan and busy-spin at ~8% CPU each. Removing each Session also
    // drops its master PTY, which closes the pseudoconsole (ClosePseudoConsole) and ends the host.
    pub fn kill_all(&self) {
        for (_, mut s) in self.sessions().drain() {
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
