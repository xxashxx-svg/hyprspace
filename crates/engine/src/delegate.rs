use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_harness::Mcp;
use hyprspace_proto::{Agent, Delegation, Event, SessionId};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct Delegates(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    port: Option<u16>,
    tokens: HashMap<String, SessionId>,
    waiting: HashMap<u64, mpsc::Sender<(bool, String)>>,
    next: u64,
}

impl Delegates {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn mcp(&self, id: SessionId, events: &UnboundedSender<Event>) -> Option<Mcp> {
        let command = std::env::current_exe().ok()?;
        let port = self.port(events)?;
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).ok()?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        self.lock().tokens.insert(token.clone(), id);
        Some(Mcp {
            command,
            args: vec!["mcp".into(), port.to_string(), token],
        })
    }

    pub fn answer(&self, request: u64, ok: bool, text: String) {
        if let Some(tx) = self.lock().waiting.remove(&request) {
            let _ = tx.send((ok, text));
        }
    }

    fn port(&self, events: &UnboundedSender<Event>) -> Option<u16> {
        if let Some(port) = self.lock().port {
            return Some(port);
        }
        let listener = TcpListener::bind("127.0.0.1:0").ok()?;
        let port = listener.local_addr().ok()?.port();
        self.lock().port = Some(port);
        let (me, events) = (self.clone(), events.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (me, events) = (me.clone(), events.clone());
                std::thread::spawn(move || me.serve(stream, &events));
            }
        });
        Some(port)
    }

    fn serve(&self, mut s: TcpStream, events: &UnboundedSender<Event>) {
        let Some(body) = crate::hooks::read_request(&mut s) else {
            return;
        };
        let _ = s.set_read_timeout(None);
        let v: Value = serde_json::from_str(&body).unwrap_or_default();
        let parent = self
            .lock()
            .tokens
            .get(v["token"].as_str().unwrap_or_default())
            .copied();
        let (ok, text) = match (parent, ask(&v)) {
            (None, _) => (false, "This session can't hand out tasks.".into()),
            (_, Err(why)) => (false, why),
            (Some(parent), Ok(ask)) => {
                let (tx, rx) = mpsc::channel();
                let request = {
                    let mut inner = self.lock();
                    inner.next += 1;
                    let request = inner.next;
                    inner.waiting.insert(request, tx);
                    request
                };
                let _ = events.unbounded_send(Event::Delegate {
                    request,
                    parent,
                    ask,
                });
                rx.recv()
                    .unwrap_or((false, "HyprSpace closed before the task finished.".into()))
            }
        };
        let body = json!({ "ok": ok, "text": text }).to_string();
        let _ = write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
}

fn ask(v: &Value) -> Result<Delegation, String> {
    let agent = match v["agent"].as_str() {
        Some("claude") => Agent::Claude,
        Some("codex") => Agent::Codex,
        _ => return Err("agent has to be claude or codex.".into()),
    };
    let task = v["task"].as_str().unwrap_or_default().trim().to_string();
    if task.is_empty() {
        return Err("task can't be empty.".into());
    }
    let opt = |k: &str| {
        v[k].as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Ok(Delegation {
        agent,
        model: opt("model"),
        effort: opt("effort"),
        title: opt("title"),
        task,
    })
}

const TOOL: &str = "Hand a task to another coding agent and wait for its final reply. It runs in its own HyprSpace thread in this folder with this thread's permissions, and the user can watch and answer it there. It doesn't see this conversation, so put everything it needs in the task. Use it to bring in a different agent or model, like a plan from one and the build from another. Several calls at once run in parallel.";

fn tools() -> Value {
    json!({ "tools": [{
        "name": "delegate",
        "description": TOOL,
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent": { "type": "string", "enum": ["claude", "codex"] },
                "task": { "type": "string", "description": "What to do, with all the context it needs." },
                "model": { "type": "string", "description": "A model the agent's CLI takes, like opus or sonnet for Claude, gpt-5.5 for Codex. Leave out for its default." },
                "effort": { "type": "string", "description": "low, medium, high, xhigh or max. Leave out for the model's default." },
                "title": { "type": "string", "description": "A short name for the thread." }
            },
            "required": ["agent", "task"]
        }
    }]})
}

pub fn run(port: u16, token: &str) {
    let out = Arc::new(Mutex::new(std::io::stdout()));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        match msg["method"].as_str().unwrap_or_default() {
            "initialize" => {
                let version = msg["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2025-06-18");
                reply(
                    &out,
                    json!({ "jsonrpc": "2.0", "id": id, "result": {
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "hyprspace", "version": env!("CARGO_PKG_VERSION") }
                    }}),
                );
            }
            "ping" => reply(&out, json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
            "tools/list" => reply(
                &out,
                json!({ "jsonrpc": "2.0", "id": id, "result": tools() }),
            ),
            "tools/call" => {
                let mut body = msg["params"]["arguments"].clone();
                if !body.is_object() {
                    body = json!({});
                }
                body["token"] = json!(token);
                let out = out.clone();
                std::thread::spawn(move || {
                    let (ok, text) = call(port, &body.to_string());
                    reply(
                        &out,
                        json!({ "jsonrpc": "2.0", "id": id, "result": {
                            "content": [{ "type": "text", "text": text }],
                            "isError": !ok
                        }}),
                    );
                });
            }
            _ => reply(
                &out,
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Unknown method" } }),
            ),
        }
    }
}

fn reply(out: &Mutex<std::io::Stdout>, msg: Value) {
    let mut out = out.lock().unwrap_or_else(|e| e.into_inner());
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

fn call(port: u16, body: &str) -> (bool, String) {
    let lost = (
        false,
        "Lost HyprSpace before the task finished.".to_string(),
    );
    let Ok(mut s) = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], port))) else {
        return (false, "HyprSpace isn't running.".into());
    };
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if s.write_all(req.as_bytes()).is_err() {
        return lost;
    }
    let mut resp = String::new();
    if s.read_to_string(&mut resp).is_err() {
        return lost;
    }
    let Some((_, body)) = resp.split_once("\r\n\r\n") else {
        return lost;
    };
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    match (v["ok"].as_bool(), v["text"].as_str()) {
        (Some(ok), Some(text)) => (ok, text.to_string()),
        _ => lost,
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[test]
    fn a_call_waits_for_the_ui_and_gets_its_answer() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let d = Delegates::default();
        let mcp = d.mcp(SessionId(4), &tx).unwrap();
        let (port, token) = (mcp.args[1].parse().unwrap(), mcp.args[2].clone());
        let body = json!({ "token": token, "agent": "codex", "task": "Build it", "model": "" });
        let waiting = std::thread::spawn(move || call(port, &body.to_string()));
        let Some(Event::Delegate {
            request,
            parent,
            ask,
        }) = futures::executor::block_on(rx.next())
        else {
            panic!("no delegation");
        };
        assert_eq!(parent, SessionId(4));
        assert_eq!(
            (ask.agent, ask.model, ask.task.as_str()),
            (Agent::Codex, None, "Build it")
        );
        d.answer(request, true, "Built.".into());
        assert_eq!(waiting.join().unwrap(), (true, "Built.".to_string()));

        let bad = json!({ "token": "nope", "agent": "codex", "task": "x" }).to_string();
        assert!(!call(port, &bad).0);
    }
}
