// A small JSON-RPC client over `codex app-server`'s stdio: one JSON message per line, responses
// matched to requests by id. Follows zeron's jsonrpc.rs (MIT, see THIRD_PARTY_NOTICES.md).
//
// A reader task resolves responses straight into a pending map, so the session can await a
// request while notifications queue up in order for it.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot};

/// A message from the app server that is not a response, in stdout order.
#[derive(Debug)]
pub(crate) enum Incoming {
    Notification {
        method: String,
        params: Value,
    },
    /// The server asks the client something (approvals) and waits for `respond`.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
}

type Reply = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<Reply>>>>;

pub(crate) struct Rpc {
    next: AtomicI64,
    pending: Pending,
    writer: mpsc::UnboundedSender<String>,
}

impl Rpc {
    /// Starts the reader and writer tasks. The incoming queue closes when the server exits.
    pub fn new(
        stdin: ChildStdin,
        stdout: ChildStdout,
    ) -> (Self, mpsc::UnboundedReceiver<Incoming>) {
        let (writer, lines) = mpsc::unbounded_channel();
        tokio::spawn(write_loop(stdin, lines));
        let pending = Pending::default();
        let (tx, incoming) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(stdout, pending.clone(), tx));
        let rpc = Self {
            next: AtomicI64::new(1),
            pending,
            writer,
        };
        (rpc, incoming)
    }

    /// Writes the request now and returns its reply. Dropping the future leaves the request on
    /// the wire, which is how a fire-and-forget `turn/interrupt` goes out.
    pub fn request(&self, method: &str, params: Value) -> impl Future<Output = Reply> + 'static {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let method = method.to_string();
        async move {
            rx.await
                .unwrap_or_else(|_| Err("codex exited before answering".into()))
                .map_err(|e| format!("{method}: {e}"))
        }
    }

    pub fn notify(&self, method: &str) {
        self.send(json!({ "jsonrpc": "2.0", "method": method }));
    }

    pub fn respond(&self, id: &Value, result: Value) {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    pub fn respond_error(&self, id: &Value, message: &str) {
        let error = json!({ "code": -32601, "message": message });
        self.send(json!({ "jsonrpc": "2.0", "id": id, "error": error }));
    }

    fn send(&self, message: Value) {
        // a closed writer means the server is gone, which the reader reports
        let _ = self.writer.send(message.to_string());
    }
}

async fn write_loop(mut stdin: ChildStdin, mut lines: mpsc::UnboundedReceiver<String>) {
    while let Some(line) = lines.recv().await {
        let wrote = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        };
        if wrote.await.is_err() {
            return;
        }
    }
}

async fn read_loop(stdout: ChildStdout, pending: Pending, tx: mpsc::UnboundedSender<Incoming>) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(mut v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let method = v["method"].as_str().map(str::to_string);
        let params = v["params"].take();
        let incoming = match (method, v.get("id").cloned()) {
            (Some(method), Some(id)) => Incoming::Request { id, method, params },
            (Some(method), None) => Incoming::Notification { method, params },
            (None, Some(id)) => {
                let reply = match v.get("error") {
                    Some(e) => Err(e["message"]
                        .as_str()
                        .unwrap_or("request failed")
                        .to_string()),
                    None => Ok(v["result"].take()),
                };
                let waiter = id.as_i64().and_then(|id| {
                    pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&id)
                });
                if let Some(waiter) = waiter {
                    let _ = waiter.send(reply);
                }
                continue;
            }
            (None, None) => continue,
        };
        if tx.send(incoming).is_err() {
            break;
        }
    }
    // the server is gone: every waiting request fails instead of hanging
    pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
