// The Codex harness: the user's own `codex app-server`, JSON-RPC over stdio, the interface the
// Codex IDE extension uses. Follows zeron's codex harness (MIT, see THIRD_PARTY_NOTICES.md),
// checked against codex-cli 0.159.3.
//
// One app server and one thread per session. A prompt starts a turn (`turn/start`); one sent
// mid-turn joins it (`turn/steer`). Approvals arrive as server requests and wait for the user.

mod items;
mod rpc;

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use hyprspace_proto::{Agent, Answer, Launch, Permission, Prompt, RunEvent, RunStatus, Tool};
use serde_json::{Value, json};
use tokio::process::Child;
use tokio::sync::mpsc;

use crate::spawn::{Tail, exit_message, spawn};
use crate::{Emit, Harness, Input, Session};
use rpc::{Incoming, Rpc};

pub struct Codex {
    program: PathBuf,
    patience: Duration,
    mcp: Option<crate::Mcp>,
}

impl Default for Codex {
    fn default() -> Self {
        Self {
            program: PathBuf::from("codex"),
            patience: Duration::from_secs(5),
            mcp: None,
        }
    }
}

impl Codex {
    /// Runs `program app-server` instead of `codex` from PATH (the tests' fake CLI).
    pub fn with_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    pub fn with_mcp(mut self, mcp: Option<crate::Mcp>) -> Self {
        self.mcp = mcp;
        self
    }

    /// How long an interrupted turn gets to stop before the app server is killed.
    pub fn with_patience(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }
}

/// The approval policy and sandbox for each permission, named after Codex's own presets: Read
/// Only, Agent and Full Access. Plan has no Codex mode, so it reads and never asks.
fn policy(permission: Permission) -> (&'static str, &'static str) {
    match permission {
        Permission::Plan => ("never", "read-only"),
        Permission::Ask => ("on-request", "read-only"),
        Permission::Auto => ("on-request", "workspace-write"),
        Permission::Bypass => ("never", "danger-full-access"),
    }
}

/// `thread/start` or `thread/resume` and its params.
fn thread_request(launch: &Launch, mcp: Option<&crate::Mcp>) -> (&'static str, Value) {
    let (approval, sandbox) = policy(launch.permission);
    let mut p = json!({ "approvalPolicy": approval, "sandbox": sandbox });
    if let Some(mcp) = mcp {
        p["config"] = json!({ "mcp_servers.hyprspace": {
            "command": mcp.command,
            "args": mcp.args,
            "tool_timeout_sec": 86400,
        }});
    }
    if let Some(model) = &launch.model {
        p["model"] = json!(model);
    }
    match &launch.resume {
        // a resumed thread keeps the folder it was started in
        Some(thread) => {
            p["threadId"] = json!(thread);
            // history is not drawn yet, and a long thread's turns run to megabytes
            p["excludeTurns"] = json!(true);
            ("thread/resume", p)
        }
        None => {
            p["cwd"] = json!(launch.cwd);
            ("thread/start", p)
        }
    }
}

/// Prompts as app-server input: each one's text, then its images as local files.
fn input(prompts: &[Prompt]) -> Value {
    let mut out = Vec::new();
    for p in prompts {
        out.push(json!({ "type": "text", "text": p.text, "text_elements": [] }));
        for image in &p.images {
            out.push(json!({ "type": "localImage", "path": image }));
        }
    }
    Value::Array(out)
}

impl Harness for Codex {
    fn agent(&self) -> Agent {
        Agent::Codex
    }

    fn start(&self, launch: Launch, emit: Emit) -> io::Result<Session> {
        let proc = spawn(&self.program, &["app-server".to_string()], &launch.cwd)?;
        let (rpc, incoming) = Rpc::new(proc.stdin, proc.stdout);
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = Actor {
            child: proc.child,
            stderr: proc.stderr,
            rpc,
            emit,
            launch,
            patience: self.patience,
            thread: String::new(),
            run: None,
            approvals: HashMap::new(),
            mcp: self.mcp.clone(),
            tools: HashMap::new(),
            streamed: HashSet::new(),
            last_text: None,
            last_thought: None,
        };
        Ok(Session::new(tx, tokio::spawn(actor.serve(incoming, rx))))
    }
}

struct Run {
    since: Instant,
    turn: Option<String>,
    interrupted: bool,
    kill_at: Option<Instant>,
    /// Prompts whose steer came too late for the turn; they start the next turn of this run.
    queued: Vec<Prompt>,
    /// The last agent message, the run's final reply.
    reply: String,
    wrote: bool,
    input: u64,
    output: u64,
}

struct Actor {
    child: Child,
    stderr: Tail,
    rpc: Rpc,
    emit: Emit,
    launch: Launch,
    patience: Duration,
    thread: String,
    run: Option<Run>,
    /// Approval requests waiting on the user, with the JSON-RPC id to answer.
    approvals: HashMap<String, (Value, bool)>,
    mcp: Option<crate::Mcp>,
    /// Tool items in flight. A file-change approval names only the item, so its edits come
    /// from here.
    tools: HashMap<String, Tool>,
    /// Agent messages that streamed deltas, so their completed text is not repeated.
    streamed: HashSet<String>,
    last_text: Option<String>,
    last_thought: Option<(String, u64)>,
}

impl Actor {
    async fn serve(
        mut self,
        mut incoming: mpsc::UnboundedReceiver<Incoming>,
        mut rx: mpsc::UnboundedReceiver<Input>,
    ) {
        if let Err(e) = self.open().await {
            let message = match tokio::time::timeout(self.patience, self.child.wait()).await {
                Ok(_) => exit_message("codex", &mut self.child, &self.stderr).await,
                Err(_) => format!("Codex could not start the conversation: {e}"),
            };
            (self.emit)(RunEvent::Failed { message });
            return;
        }
        loop {
            let kill_at = self.run.as_ref().and_then(|r| r.kill_at);
            tokio::select! {
                msg = incoming.recv() => match msg {
                    Some(Incoming::Notification { method, params }) => {
                        self.notification(&method, params).await
                    }
                    Some(Incoming::Request { id, method, params }) => {
                        self.request(id, &method, params)
                    }
                    None => break,
                },
                input = rx.recv() => match input {
                    Some(input) => self.input(input).await,
                    None => return,
                },
                _ = sleep_until(kill_at), if kill_at.is_some() => {
                    let run = self.run.take().expect("kill_at belongs to a run");
                    self.finished(run, RunStatus::Interrupted, None);
                    (self.emit)(RunEvent::Failed {
                        message: "Codex did not stop when asked, so the session was closed.".into(),
                    });
                    return;
                }
            }
        }
        let message = exit_message("codex", &mut self.child, &self.stderr).await;
        if let Some(run) = self.run.take() {
            let status = if run.interrupted {
                RunStatus::Interrupted
            } else {
                RunStatus::Failed
            };
            self.finished(run, status, Some(message.clone()));
        }
        (self.emit)(RunEvent::Failed { message });
    }

    /// The handshake, then a new or resumed thread.
    async fn open(&mut self) -> Result<(), String> {
        let hello = json!({
            "clientInfo": {
                "name": "hyprspace",
                "title": "HyprSpace",
                "version": env!("CARGO_PKG_VERSION"),
            },
            // turn/steer is part of the experimental surface
            "capabilities": { "experimentalApi": true },
        });
        self.rpc.request("initialize", hello).await?;
        self.rpc.notify("initialized");
        let (method, params) = thread_request(&self.launch, self.mcp.as_ref());
        let opened = self.rpc.request(method, params).await?;
        self.thread = text(&opened["thread"]["id"]);
        let cwd = opened["cwd"].as_str().map(PathBuf::from);
        (self.emit)(RunEvent::Started {
            agent: Agent::Codex,
            model: text(&opened["model"]),
            thread: self.thread.clone(),
            cwd: cwd.unwrap_or_else(|| self.launch.cwd.clone()),
        });
        Ok(())
    }

    fn turn_params(&self, prompts: &[Prompt]) -> Value {
        let mut p = json!({
            "threadId": self.thread,
            "input": input(prompts),
            // without it codex thinks in silence: no reasoning summary streams
            "summary": "auto",
        });
        if let Some(effort) = &self.launch.effort {
            p["effort"] = json!(effort);
        }
        p
    }

    async fn start_turn(&mut self, prompts: &[Prompt]) -> Result<(), String> {
        let started = self
            .rpc
            .request("turn/start", self.turn_params(prompts))
            .await?;
        if let Some(run) = self.run.as_mut() {
            run.turn = started["turn"]["id"].as_str().map(str::to_string);
        }
        Ok(())
    }

    async fn input(&mut self, input: Input) {
        match input {
            Input::Send(prompt) => self.send(prompt).await,
            Input::Interrupt => {
                let Some(run) = self.run.as_mut().filter(|r| !r.interrupted) else {
                    return;
                };
                run.interrupted = true;
                run.kill_at = Some(Instant::now() + self.patience);
                if let Some(turn) = run.turn.clone() {
                    let params = json!({ "threadId": self.thread, "turnId": turn });
                    drop(self.rpc.request("turn/interrupt", params));
                }
            }
            Input::Answer { request, answer } => {
                if let Some((id, elicit)) = self.approvals.remove(&request) {
                    let reply = match (elicit, answer) {
                        (true, Answer::Deny) => json!({ "action": "decline" }),
                        (true, _) => json!({ "action": "accept", "content": {} }),
                        (false, Answer::Allow) => json!({ "decision": "accept" }),
                        (false, Answer::AllowAlways) => json!({ "decision": "acceptForSession" }),
                        (false, Answer::Deny) => json!({ "decision": "decline" }),
                    };
                    self.rpc.respond(&id, reply);
                }
            }
        }
    }

    async fn send(&mut self, prompt: Prompt) {
        let Some(run) = self.run.as_mut() else {
            self.run = Some(Run {
                since: Instant::now(),
                turn: None,
                interrupted: false,
                kill_at: None,
                queued: Vec::new(),
                reply: String::new(),
                wrote: false,
                input: 0,
                output: 0,
            });
            if let Err(e) = self.start_turn(std::slice::from_ref(&prompt)).await {
                let run = self.run.take().expect("set above");
                self.finished(run, RunStatus::Failed, Some(e));
            }
            return;
        };
        let Some(turn) = run.turn.clone() else {
            run.queued.push(prompt);
            return;
        };
        let params = json!({
            "threadId": self.thread,
            "expectedTurnId": turn,
            "input": input(std::slice::from_ref(&prompt)),
        });
        match self.rpc.request("turn/steer", params).await {
            Ok(_) => (self.emit)(RunEvent::Steered),
            // most often the turn finished between the send and the steer: the prompt starts
            // the next turn of this run when the end arrives
            Err(_) => {
                if let Some(run) = self.run.as_mut() {
                    run.queued.push(prompt);
                }
            }
        }
    }

    fn request(&mut self, id: Value, method: &str, params: Value) {
        let tool = match method {
            "item/commandExecution/requestApproval" => Tool::Command {
                command: text(&params["command"]),
            },
            "item/fileChange/requestApproval" => self
                .tools
                .get(params["itemId"].as_str().unwrap_or_default())
                .cloned()
                .unwrap_or(Tool::Edit { changes: vec![] }),
            "mcpServer/elicitation/request" => Tool::Mcp {
                server: text(&params["serverName"]),
                tool: String::new(),
                input: text(&params["message"]),
            },
            // anything else must still get an answer, or the turn waits forever
            _ => {
                let message = format!("HyprSpace does not answer {method} yet.");
                self.rpc.respond_error(&id, &message);
                return;
            }
        };
        let request = key(&id);
        let elicit = matches!(tool, Tool::Mcp { .. });
        self.approvals.insert(request.clone(), (id, elicit));
        (self.emit)(RunEvent::Approval {
            request,
            tool,
            reason: params["reason"].as_str().map(str::to_string),
            always: !elicit,
        });
    }

    async fn notification(&mut self, method: &str, params: Value) {
        // subagents run as their own threads on the same server; their traffic is not ours
        if params["threadId"]
            .as_str()
            .is_some_and(|t| !t.is_empty() && t != self.thread)
        {
            return;
        }
        match method {
            "turn/started" => {
                if let Some(run) = self.run.as_mut() {
                    run.turn = params["turn"]["id"].as_str().map(str::to_string);
                }
            }
            "item/agentMessage/delta" => {
                let item = text(&params["itemId"]);
                self.streamed.insert(item.clone());
                self.text(item, text(&params["delta"]));
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                let index = params["summaryIndex"]
                    .as_u64()
                    .or(params["contentIndex"].as_u64())
                    .unwrap_or(0);
                let part = (text(&params["itemId"]), index);
                // parts carry no separator of their own, and two summaries would run together
                if self.last_thought.as_ref().is_some_and(|p| *p != part) {
                    (self.emit)(RunEvent::Thinking {
                        text: "\n\n".into(),
                    });
                }
                self.last_thought = Some(part);
                (self.emit)(RunEvent::Thinking {
                    text: text(&params["delta"]),
                });
            }
            "item/started" => {
                let item = &params["item"];
                if let Some(tool) = items::tool(item) {
                    let id = text(&item["id"]);
                    self.tools.insert(id.clone(), tool.clone());
                    (self.emit)(RunEvent::Tool { id, tool });
                }
            }
            "item/completed" => self.completed(&params["item"]),
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"];
                let last = &usage["last"];
                if let Some(run) = self.run.as_mut() {
                    run.input += last["inputTokens"].as_u64().unwrap_or(0);
                    run.output += last["outputTokens"].as_u64().unwrap_or(0);
                }
                // the latest turn's total is what sits in the window, as Codex's own TUI counts it
                if let (Some(used), Some(window)) = (
                    last["totalTokens"].as_u64(),
                    usage["modelContextWindow"].as_u64(),
                ) {
                    (self.emit)(RunEvent::Context { used, window });
                }
            }
            "serverRequest/resolved" => {
                self.approvals.remove(&key(&params["requestId"]));
            }
            "error" if !params["willRetry"].as_bool().unwrap_or(false) => {
                (self.emit)(RunEvent::Error {
                    message: readable(&params["error"]["message"]),
                });
                if limited(&params["error"]) {
                    (self.emit)(RunEvent::Limited { resets: None });
                }
            }
            "turn/completed" => self.turn_completed(&params["turn"]).await,
            _ => {}
        }
    }

    fn text(&mut self, item: String, delta: String) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        // codex sends several messages per turn with no separator between them
        if run.wrote && self.last_text.as_ref() != Some(&item) {
            (self.emit)(RunEvent::Text {
                text: "\n\n".into(),
            });
        }
        if self.last_text.as_ref() != Some(&item) {
            run.reply.clear();
        }
        run.wrote = true;
        run.reply.push_str(&delta);
        self.last_text = Some(item);
        (self.emit)(RunEvent::Text { text: delta });
    }

    fn completed(&mut self, item: &Value) {
        let id = text(&item["id"]);
        if item["type"] == "agentMessage" {
            if !self.streamed.remove(&id) {
                self.text(id, text(&item["text"]));
            }
            return;
        }
        let Some(tool) = items::tool(item) else {
            return;
        };
        self.tools.remove(&id);
        // the completed item has the final diff and output
        (self.emit)(RunEvent::Tool {
            id: id.clone(),
            tool,
        });
        let (ok, output) = items::outcome(item);
        (self.emit)(RunEvent::ToolDone { id, ok, output });
    }

    async fn turn_completed(&mut self, turn: &Value) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let status = turn["status"].as_str().unwrap_or("completed");
        if status == "completed" && !run.interrupted && !run.queued.is_empty() {
            let queued = std::mem::take(&mut run.queued);
            run.turn = None;
            match self.start_turn(&queued).await {
                Ok(()) => {
                    for _ in &queued {
                        (self.emit)(RunEvent::Steered);
                    }
                    return;
                }
                Err(e) => {
                    let run = self.run.take().expect("checked above");
                    self.finished(run, RunStatus::Failed, Some(e));
                    return;
                }
            }
        }
        let run = self.run.take().expect("checked above");
        let error = turn["error"]["message"]
            .as_str()
            .map(|_| readable(&turn["error"]["message"]));
        let status = match status {
            _ if run.interrupted => RunStatus::Interrupted,
            "interrupted" => RunStatus::Interrupted,
            "failed" => RunStatus::Failed,
            _ => RunStatus::Done,
        };
        self.finished(run, status, error);
    }

    fn finished(&mut self, run: Run, status: RunStatus, error: Option<String>) {
        self.approvals.clear();
        self.tools.clear();
        self.streamed.clear();
        self.last_text = None;
        self.last_thought = None;
        (self.emit)(RunEvent::Usage {
            input: run.input,
            output: run.output,
        });
        (self.emit)(RunEvent::Finished {
            status,
            ms: run.since.elapsed().as_millis() as u64,
            text: run.reply,
            error,
        });
    }
}

/// A JSON-RPC id as the approval's request string.
fn key(id: &Value) -> String {
    id.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| id.to_string())
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn limited(error: &Value) -> bool {
    let info = &error["codexErrorInfo"];
    info == "usageLimitExceeded"
        || info.get("usageLimitExceeded").is_some()
        || text(&error["message"])
            .to_lowercase()
            .contains("usage limit")
}

/// An error message, unwrapped when codex passes the API's JSON error body through as is.
fn readable(message: &Value) -> String {
    let raw = text(message);
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or(raw)
}

async fn sleep_until(at: Option<Instant>) {
    if let Some(at) = at {
        tokio::time::sleep_until(at.into()).await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn usage_limits_are_told_apart() {
        use serde_json::json;
        assert!(super::limited(
            &json!({ "message": "x", "codexErrorInfo": "usageLimitExceeded" })
        ));
        assert!(super::limited(
            &json!({ "message": "You've hit your usage limit. Try again at 3:05 PM." })
        ));
        assert!(!super::limited(
            &json!({ "message": "stream disconnected", "codexErrorInfo": "other" })
        ));
    }

    use super::*;

    #[test]
    fn permissions_map_to_codex_presets() {
        let mut launch = Launch::new(Agent::Codex, "/w");
        let (method, p) = thread_request(&launch, None);
        assert_eq!(method, "thread/start");
        assert_eq!(p["approvalPolicy"], "on-request");
        assert_eq!(p["sandbox"], "read-only");
        assert_eq!(p["cwd"], "/w");
        assert!(p.get("model").is_none());
        launch.permission = Permission::Bypass;
        launch.model = Some("gpt-5.5".into());
        launch.resume = Some("th-1".into());
        let (method, p) = thread_request(&launch, None);
        assert_eq!(method, "thread/resume");
        assert_eq!(p["threadId"], "th-1");
        assert_eq!(p["sandbox"], "danger-full-access");
        assert_eq!(p["model"], "gpt-5.5");
        assert!(p.get("cwd").is_none());
    }

    #[test]
    fn input_puts_images_after_their_text() {
        let v = input(&[
            Prompt {
                text: "look".into(),
                images: vec![PathBuf::from("/a.png")],
            },
            Prompt::text("and this"),
        ]);
        let kinds: Vec<&str> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["type"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["text", "localImage", "text"]);
        assert_eq!(v[1]["path"], "/a.png");
        assert_eq!(key(&json!(7)), "7");
        assert_eq!(key(&json!("r-1")), "r-1");
        // codex 0.159.3 passes a rejected model's API error through as JSON text
        let api = json!(r#"{"type":"error","status":400,"error":{"message":"Not supported."}}"#);
        assert_eq!(readable(&api), "Not supported.");
        assert_eq!(readable(&json!("plain")), "plain");
    }
}
