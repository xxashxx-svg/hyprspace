// PTY sessions on portable-pty, copied from src-tauri/src/pty.rs and cut down for the spike:
// the coalescer, the off-thread child wait and `kill_all` on exit carry over; per-session kill, the Tauri channel, the mobile tap and the xterm pause gate don't. Output goes to a
// plain channel the terminal view drains.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

// Leading-edge coalescer: the first bytes after a quiet moment flush at once for snappy echo, a
// sustained firehose batches to one frame (16ms) or 16K. Same tuning as the Tauri app.
const FLUSH_MS: u64 = 16;
const FLUSH_BYTES: usize = 16 * 1024;
const READ_BUF: usize = 64 * 1024;

pub enum PtyEvent {
    Data(Vec<u8>),
    Exit(i32),
}

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: SharedWriter,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

#[derive(Default, Clone)]
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<u64, Session>>>,
}

impl PtyManager {
    // the guarded data is just a session map, so recovering a poisoned lock is always safe
    fn sessions(&self) -> MutexGuard<'_, HashMap<u64, Session>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn create(
        &self,
        id: u64,
        cwd: &str,
        cols: u16,
        rows: u16,
        out: UnboundedSender<PtyEvent>,
    ) -> anyhow::Result<()> {
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(default_shell());
        if !cwd.is_empty() {
            cmd.cwd(cwd);
        }
        // GUI-launched apps inherit no TERM, so CLIs suppress color without this
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for k in crate::claude::SESSION_ENV {
            cmd.env_remove(k);
        }

        let mut child = pair.slave.spawn_command(cmd)?;
        // drop the slave right after spawn so the master read sees EOF when the child exits (ConPTY)
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer: SharedWriter = Arc::new(Mutex::new(pair.master.take_writer()?));
        let killer = child.clone_killer();

        // reader thread -> bounded channel (blocking = backpressure, no byte drops) -> coalescer
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
            let mut acc: Vec<u8> = Vec::with_capacity(FLUSH_BYTES);
            let interval = Duration::from_millis(FLUSH_MS);
            let mut last_flush = Instant::now();
            let emit = |batch: Vec<u8>| {
                let _ = data_out.unbounded_send(PtyEvent::Data(batch));
            };
            loop {
                match rx.recv_timeout(interval) {
                    Ok(chunk) => {
                        acc.extend_from_slice(&chunk);
                        if acc.len() >= FLUSH_BYTES || last_flush.elapsed() >= interval {
                            emit(std::mem::take(&mut acc));
                            last_flush = Instant::now();
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if !acc.is_empty() {
                            emit(std::mem::take(&mut acc));
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

        // child.wait() off-thread: PseudoConsoleClose can block
        thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code() as i32).unwrap_or(-1);
            let _ = out.unbounded_send(PtyEvent::Exit(code));
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

    pub fn write(&self, id: u64, data: &[u8]) {
        // clone the writer and drop the map lock first, so a slow write never blocks other sessions
        let Some(writer) = self.sessions().get(&id).map(|s| s.writer.clone()) else {
            return;
        };
        let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
        let _ = w.write_all(data).and_then(|_| w.flush());
    }

    pub fn resize(&self, id: u64, cols: u16, rows: u16) {
        if let Some(s) = self.sessions().get(&id) {
            let _ = s.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    // On app exit: otherwise ConPTY hosts (OpenConsole.exe) orphan and busy-spin.
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
