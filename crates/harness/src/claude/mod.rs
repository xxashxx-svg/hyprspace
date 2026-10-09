// The Claude harness: the user's own `claude` binary in headless stream-json mode, the same
// invocation zeron's harness uses (MIT, see THIRD_PARTY_NOTICES.md). Inference stays on the
// user's subscription through their CLI; no SDK, no API key, no token.
//
// One process per session. Each prompt is a stdin user line; a prompt sent mid-run is a steer
// line the CLI folds in at its next step. Tool approvals arrive as `can_use_tool` control
// requests (--permission-prompt-tool stdio) and wait until the user answers.
//
// Subagents (the Agent tool) send their own traffic tagged with the spawning call's id. It goes
// out as `Subagent*` events under that id, never into the main reply. A subagent the CLI runs in
// the background answers its call at once with a launch note, so that call stays open until the
// subagent's `task_notification`, and the CLI then starts a run of its own to read the result.

mod resume;
mod tools;
mod wire;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use hyprspace_proto::{Agent, Launch, Permission, Prompt, RunEvent, RunStatus, Tool};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

use crate::spawn::{Tail, exit_message, spawn};
use crate::{Emit, Harness, Input, Session};

pub struct Claude {
    program: PathBuf,
    config: Option<PathBuf>,
    patience: Duration,
    mcp: Option<crate::Mcp>,
}

impl Default for Claude {
    fn default() -> Self {
        Self {
            program: PathBuf::from("claude"),
            config: None,
            mcp: None,
            patience: Duration::from_secs(5),
        }
    }
}

impl Claude {
    /// Runs `program` instead of `claude` from PATH (the tests' fake CLI).
    pub fn with_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    pub fn with_mcp(mut self, mcp: Option<crate::Mcp>) -> Self {
        self.mcp = mcp;
        self
    }

    /// Reads saved conversations from `dir` instead of `~/.claude`.
    pub fn with_config_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config = Some(dir.into());
        self
    }

    /// How long a quiet CLI gets: to take a steer before a held run end is released, and to
    /// stop after an interrupt before it is killed.
    pub fn with_patience(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }
}

fn args(launch: &Launch) -> Vec<String> {
    let mut out: Vec<String> = [
        "--print",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        // the CLI refuses stream-json output under --print without it
        "--verbose",
        "--include-partial-messages",
        // echoes each stdin message once taken, which is how a steer is known to have landed
        "--replay-user-messages",
        // newer models stream no readable thinking unless a summary is asked for
        "--thinking-display",
        "summarized",
        "--permission-prompt-tool",
        "stdio",
    ]
    .map(String::from)
    .into();
    match launch.permission {
        Permission::Plan => out.extend(["--permission-mode".into(), "plan".into()]),
        Permission::Ask => out.extend(["--permission-mode".into(), "default".into()]),
        Permission::Auto => out.extend(["--permission-mode".into(), "acceptEdits".into()]),
        Permission::Bypass => out.push("--dangerously-skip-permissions".into()),
    }
    if let Some(model) = &launch.model {
        out.extend(["--model".into(), model.clone()]);
    }
    if let Some(effort) = &launch.effort {
        out.extend(["--effort".into(), effort.clone()]);
    }
    if let Some(thread) = &launch.resume {
        out.extend(["--resume".into(), thread.clone()]);
    }
    out
}

impl Harness for Claude {
    fn agent(&self) -> Agent {
        Agent::Claude
    }

    fn start(&self, launch: Launch, emit: Emit) -> io::Result<Session> {
        let origin = launch.resume.as_deref().and_then(|thread| {
            let config = self.config.clone().or_else(resume::config_dir)?;
            resume::origin(&config, thread)
        });
        let cwd = origin.unwrap_or_else(|| launch.cwd.clone());
        let mut args = args(&launch);
        if let Some(path) = self.mcp.as_ref().and_then(mcp_config) {
            args.extend(["--mcp-config".into(), path.to_string_lossy().into_owned()]);
        }
        let proc = spawn(&self.program, &args, &cwd)?;
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = Actor {
            child: proc.child,
            stdin: proc.stdin,
            stderr: proc.stderr,
            emit,
            cwd,
            patience: self.patience,
            started: false,
            run: None,
            approvals: HashMap::new(),
            open_tools: HashSet::new(),
            agents: HashMap::new(),
            nested: HashMap::new(),
            context: 0,
            window: None,
        };
        let stdout = BufReader::new(proc.stdout).lines();
        Ok(Session::new(tx, tokio::spawn(actor.serve(stdout, rx))))
    }
}

struct Run {
    since: Instant,
    /// Steers written but not yet echoed back by the CLI.
    steers: VecDeque<String>,
    interrupted: bool,
    /// A `result` that came while steers were pending: a `now` steer ends the turn it cuts
    /// into, but the run goes on. Released after `patience` of quiet if no other result comes.
    held: Option<(Value, Instant)>,
    /// When an ignored interrupt gives up and kills the CLI.
    kill_at: Option<Instant>,
    heard: bool,
    input: u64,
    output: u64,
    limited: bool,
}

impl Run {
    fn new() -> Self {
        Self {
            since: Instant::now(),
            steers: VecDeque::new(),
            interrupted: false,
            held: None,
            kill_at: None,
            heard: false,
            input: 0,
            output: 0,
            limited: false,
        }
    }
}

struct Actor {
    child: Child,
    stdin: ChildStdin,
    stderr: Tail,
    emit: Emit,
    cwd: PathBuf,
    patience: Duration,
    started: bool,
    run: Option<Run>,
    /// Approval requests waiting on the user, with the tool input to hand back on allow and
    /// the permission rules the CLI suggested for "always allow".
    approvals: HashMap<String, (Value, Value)>,
    /// Tool calls of the main thread that have no result yet.
    open_tools: HashSet<String>,
    /// The main thread's Agent calls, true once their `ToolDone` went out.
    agents: HashMap<String, bool>,
    /// Agent calls a subagent made, to the main thread's call they report under.
    nested: HashMap<String, String>,
    /// Tokens the main thread's latest message saw and wrote.
    context: u64,
    /// The main model's context window, from the latest `result`.
    window: Option<u64>,
}

type Lines = tokio::io::Lines<BufReader<tokio::process::ChildStdout>>;

impl Actor {
    async fn serve(mut self, mut lines: Lines, mut rx: mpsc::UnboundedReceiver<Input>) {
        loop {
            let wake = self.run.as_ref().and_then(|r| {
                let held = r.held.as_ref().map(|(_, at)| *at);
                held.into_iter().chain(r.kill_at).min()
            });
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {
                        let resend = self.first_word();
                        self.line(&line);
                        if resend {
                            self.write(wire::interrupt_line("hs-interrupt")).await;
                        }
                    }
                    _ => break,
                },
                input = rx.recv() => match input {
                    Some(input) => self.input(input).await,
                    None => return,
                },
                _ = sleep_until(wake), if wake.is_some() => {
                    if self.wake() {
                        return;
                    }
                }
            }
        }
        let mut message = exit_message("claude", &mut self.child, &self.stderr).await;
        if message.contains("No conversation found") {
            message = format!(
                "Claude could not find that conversation in {}.",
                self.cwd.display()
            );
        }
        if let Some(run) = self.run.take() {
            let status = if run.interrupted {
                RunStatus::Interrupted
            } else {
                RunStatus::Failed
            };
            self.finished(run, status, String::new(), Some(message.clone()));
        }
        (self.emit)(RunEvent::Failed { message });
    }

    async fn write(&mut self, line: String) {
        // a dead child shows up as stdout EOF, which ends the session, so a failed write is
        // not reported twice
        let _ = async {
            self.stdin.write_all(line.as_bytes()).await?;
            self.stdin.write_all(b"\n").await?;
            self.stdin.flush().await
        }
        .await;
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
                self.write(wire::interrupt_line("hs-interrupt")).await;
            }
            Input::Answer {
                request,
                answer,
                answers,
            } => {
                if let Some((mut input, rules)) = self.approvals.remove(&request) {
                    if !answers.is_empty() && input.is_object() {
                        let map: serde_json::Map<String, Value> = answers
                            .into_iter()
                            .map(|(q, a)| (q, Value::String(a)))
                            .collect();
                        input["answers"] = Value::Object(map);
                    }
                    self.write(wire::answer_line(&request, input, rules, answer))
                        .await;
                }
            }
        }
    }

    async fn send(&mut self, prompt: Prompt) {
        let content = wire::content(&prompt);
        let now = self.open_tools.is_empty();
        let line = if let Some(run) = self.run.as_mut() {
            let uuid = uuid::Uuid::new_v4().to_string();
            run.steers.push_back(uuid.clone());
            wire::steer_line(content, &uuid, now)
        } else {
            self.run = Some(Run::new());
            wire::user_line(content)
        };
        self.write(line).await;
    }

    /// A deadline passed. True when the session is over.
    fn wake(&mut self) -> bool {
        let now = Instant::now();
        let Some(run) = self.run.as_mut() else {
            return false;
        };
        if run.kill_at.is_some_and(|at| at <= now) {
            let run = self.run.take().expect("checked above");
            self.finished(run, RunStatus::Interrupted, String::new(), None);
            (self.emit)(RunEvent::Failed {
                message: "Claude did not stop when asked, so the session was closed.".into(),
            });
            // dropping the actor kills the child
            return true;
        }
        if let Some((result, _)) = run.held.take_if(|(_, at)| *at <= now) {
            // the steers were absorbed into the turn that just ended
            for _ in run.steers.drain(..) {
                (self.emit)(RunEvent::Steered);
            }
            self.result(result);
        }
        false
    }

    // a CLI still starting up can drop an interrupt written before it said anything
    fn first_word(&mut self) -> bool {
        let patience = self.patience;
        let Some(run) = self.run.as_mut().filter(|r| !r.heard) else {
            return false;
        };
        run.heard = true;
        if run.interrupted {
            run.kill_at = Some(Instant::now() + patience);
        }
        run.interrupted
    }

    fn line(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            return;
        };
        // the CLI is still producing, so a held run end keeps waiting
        if let Some((_, at)) = self.run.as_mut().and_then(|r| r.held.as_mut()) {
            *at = Instant::now() + self.patience;
        }
        // subagent traffic carries the spawning tool's id; it belongs to that subagent's own
        // transcript, never the main reply. Control requests stay here whatever they carry: an
        // approval a subagent asks for still waits on the user.
        let kind = v["type"].as_str().unwrap_or_default();
        if let Some(parent) = v["parent_tool_use_id"].as_str()
            && matches!(kind, "assistant" | "user" | "stream_event")
        {
            self.subagent(parent, &v);
            return;
        }
        match kind {
            "system" if v["subtype"] == "init" && !self.started => {
                self.started = true;
                let cwd = v["cwd"].as_str().map(PathBuf::from);
                (self.emit)(RunEvent::Started {
                    agent: Agent::Claude,
                    model: text(&v["model"]),
                    thread: text(&v["session_id"]),
                    cwd: cwd.unwrap_or_else(|| self.cwd.clone()),
                });
                let names = commands(&v);
                if !names.is_empty() {
                    (self.emit)(RunEvent::Commands { names });
                }
            }
            // the CLI starts every turn with an init; one with no run live is a turn it took
            // on its own, after a background subagent finished
            "system" if v["subtype"] == "init" && self.run.is_none() => {
                self.run = Some(Run::new());
                (self.emit)(RunEvent::Woke);
            }
            "system" if v["subtype"] == "task_notification" => self.notified(&v),
            "stream_event" if v["event"]["type"] == "content_block_delta" => {
                let delta = &v["event"]["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => (self.emit)(RunEvent::Text {
                        text: text(&delta["text"]),
                    }),
                    Some("thinking_delta") => (self.emit)(RunEvent::Thinking {
                        text: text(&delta["thinking"]),
                    }),
                    _ => {}
                }
            }
            // text already streamed as deltas; the full message adds the tool calls
            "assistant" => {
                for block in blocks(&v) {
                    if block["type"] == "tool_use" {
                        let id = text(&block["id"]);
                        self.open_tools.insert(id.clone());
                        let tool = tools::decode(
                            block["name"].as_str().unwrap_or_default(),
                            &block["input"],
                        );
                        if matches!(tool, Tool::Agent { .. }) {
                            self.agents.entry(id.clone()).or_insert(false);
                        }
                        (self.emit)(RunEvent::Tool { id, tool });
                    }
                }
                if let Some(code) = v["error"].as_str() {
                    (self.emit)(RunEvent::Error {
                        message: error_text(code),
                    });
                    if code == "rate_limit" {
                        self.limited(None);
                    }
                }
                if let Some(used) = context_used(&v["message"]["usage"]) {
                    self.context = used;
                    if let Some(window) = self.window {
                        (self.emit)(RunEvent::Context { used, window });
                    }
                }
            }
            "user" => {
                for block in blocks(&v) {
                    if block["type"] == "tool_result" {
                        let id = text(&block["tool_use_id"]);
                        self.open_tools.remove(&id);
                        let ok = !block["is_error"].as_bool().unwrap_or(false);
                        let content = &block["content"];
                        let output = match self.agents.get_mut(&id) {
                            None => tools::result_text(content),
                            // its task_notification already said how it ended
                            Some(true) => continue,
                            // a background subagent is still working; it ends by notification
                            Some(_) if ok && tools::launched(&v, content) => continue,
                            Some(done) => {
                                *done = true;
                                tools::report(content)
                            }
                        };
                        (self.emit)(RunEvent::ToolDone { id, ok, output });
                    }
                }
                self.replayed(v["uuid"].as_str());
            }
            "control_request" if v["request"]["subtype"] == "can_use_tool" => {
                let request = text(&v["request_id"]);
                let body = &v["request"];
                let tool = tools::decode(
                    body["tool_name"].as_str().unwrap_or_default(),
                    &body["input"],
                );
                // the CLI offers "always allow" by sending the rules it would save for it
                let rules = body["permission_suggestions"].clone();
                let always = rules.as_array().is_some_and(|r| !r.is_empty());
                self.approvals
                    .insert(request.clone(), (body["input"].clone(), rules));
                (self.emit)(RunEvent::Approval {
                    request,
                    tool,
                    reason: body["decision_reason"].as_str().map(str::to_string),
                    always,
                });
            }
            "control_cancel_request" => {
                self.approvals
                    .remove(v["request_id"].as_str().unwrap_or_default());
            }
            "rate_limit_event" if v["rate_limit_info"]["status"] == "rejected" => {
                self.limited(resets(&v["rate_limit_info"]["resetsAt"]));
            }
            "result" => self.result(v),
            _ => {}
        }
    }

    /// A line from a subagent: its tool calls and results, and the text it wrote. The CLI
    /// streams no partial text for subagents, so text comes whole with the assistant message.
    fn subagent(&mut self, parent: &str, v: &Value) {
        let parent = self
            .nested
            .get(parent)
            .cloned()
            .unwrap_or_else(|| parent.to_string());
        match v["type"].as_str().unwrap_or_default() {
            "assistant" => {
                for block in blocks(v) {
                    match block["type"].as_str() {
                        Some("text") if !text(&block["text"]).trim().is_empty() => {
                            (self.emit)(RunEvent::SubagentText {
                                parent: parent.clone(),
                                text: text(&block["text"]),
                            })
                        }
                        Some("tool_use") => {
                            let id = text(&block["id"]);
                            let tool = tools::decode(
                                block["name"].as_str().unwrap_or_default(),
                                &block["input"],
                            );
                            if matches!(tool, Tool::Agent { .. }) {
                                self.nested.insert(id.clone(), parent.clone());
                            }
                            (self.emit)(RunEvent::SubagentTool {
                                parent: parent.clone(),
                                id,
                                tool,
                            });
                        }
                        _ => {}
                    }
                }
            }
            // its text blocks are the prompt it was given, already on the Agent call
            "user" => {
                for block in blocks(v) {
                    if block["type"] == "tool_result" {
                        (self.emit)(RunEvent::SubagentToolDone {
                            parent: parent.clone(),
                            id: text(&block["tool_use_id"]),
                            ok: !block["is_error"].as_bool().unwrap_or(false),
                            output: tools::result_text(&block["content"]),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// A background task ended. For a subagent this is the only word of it finishing, and
    /// `summary` holds its report. Background shell commands end the same way; those are not
    /// Agent calls and are left alone.
    fn notified(&mut self, v: &Value) {
        let id = text(&v["tool_use_id"]);
        let Some(done) = self.agents.get_mut(&id).filter(|d| !**d) else {
            return;
        };
        let ok = match v["status"].as_str().unwrap_or_default() {
            "completed" => true,
            "failed" | "killed" | "stopped" | "cancelled" => false,
            // not an ending
            _ => return,
        };
        *done = true;
        (self.emit)(RunEvent::ToolDone {
            id,
            ok,
            output: text(&v["summary"]).trim().to_string(),
        });
    }

    /// The CLI echoed a user message. One of ours confirms that steer and every earlier one,
    /// since the CLI only echoes the last of several quick steers.
    fn replayed(&mut self, uuid: Option<&str>) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let Some(at) = uuid.and_then(|u| run.steers.iter().position(|s| s == u)) else {
            return;
        };
        for _ in run.steers.drain(..=at) {
            (self.emit)(RunEvent::Steered);
        }
    }

    fn limited(&mut self, resets: Option<u64>) {
        if let Some(run) = self.run.as_mut()
            && !run.limited
        {
            run.limited = true;
            (self.emit)(RunEvent::Limited { resets });
        }
    }

    fn result(&mut self, v: Value) {
        // the window size only comes with a result, so the first turn's ring waits for it
        if let Some(window) = context_window(&v["modelUsage"]) {
            self.window = Some(window);
            if self.context > 0 {
                (self.emit)(RunEvent::Context {
                    used: self.context,
                    window,
                });
            }
        }
        let Some(run) = self.run.as_mut() else {
            return;
        };
        run.input += v["usage"]["input_tokens"].as_u64().unwrap_or(0);
        run.output += v["usage"]["output_tokens"].as_u64().unwrap_or(0);
        if !run.steers.is_empty() && !run.interrupted {
            run.held = Some((v, Instant::now() + self.patience));
            return;
        }
        let run = self.run.take().expect("checked above");
        let ok = v["subtype"] == "success" && !v["is_error"].as_bool().unwrap_or(false);
        let (status, error) = if run.interrupted {
            (RunStatus::Interrupted, None)
        } else if ok {
            (RunStatus::Done, None)
        } else {
            (RunStatus::Failed, Some(result_error(&v)))
        };
        self.finished(run, status, text(&v["result"]), error);
    }

    fn finished(&mut self, run: Run, status: RunStatus, text: String, error: Option<String>) {
        self.open_tools.clear();
        self.approvals.clear();
        (self.emit)(RunEvent::Usage {
            input: run.input,
            output: run.output,
        });
        (self.emit)(RunEvent::Finished {
            status,
            ms: run.since.elapsed().as_millis() as u64,
            text,
            error,
        });
    }
}

/// What a message's usage puts in the context window: everything it read, cached or not, and
/// what it wrote. None when the message carries no usage.
fn context_used(usage: &Value) -> Option<u64> {
    let n = |k: &str| usage[k].as_u64();
    let input = n("input_tokens")?;
    Some(
        input
            + n("cache_creation_input_tokens").unwrap_or(0)
            + n("cache_read_input_tokens").unwrap_or(0)
            + n("output_tokens").unwrap_or(0),
    )
}

/// The main model's window from a result's `modelUsage`. A subagent on another model shows up
/// there too, so the model that read the most is taken as the main one.
fn context_window(models: &Value) -> Option<u64> {
    let read = |m: &Value| {
        [
            "inputTokens",
            "cacheReadInputTokens",
            "cacheCreationInputTokens",
        ]
        .iter()
        .map(|k| m[*k].as_u64().unwrap_or(0))
        .sum::<u64>()
    };
    models.as_object()?.values().max_by_key(|m| read(m))?["contextWindow"].as_u64()
}

async fn sleep_until(at: Option<Instant>) {
    if let Some(at) = at {
        tokio::time::sleep_until(at.into()).await;
    }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn blocks(v: &Value) -> &[Value] {
    v["message"]["content"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
}

fn mcp_config(mcp: &crate::Mcp) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("hyprspace-mcp");
    std::fs::create_dir_all(&dir).ok()?;
    let name = mcp.args.last()?;
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let path = dir.join(format!("{name}.json"));
    let config = json!({ "mcpServers": { "hyprspace": {
        "type": "stdio",
        "command": mcp.command,
        "args": mcp.args,
    }}});
    std::fs::write(&path, config.to_string()).ok()?;
    Some(path)
}

fn resets(v: &Value) -> Option<u64> {
    let t = v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))?;
    Some(if t < 100_000_000_000 { t * 1000 } else { t })
}

// The terse codes an assistant frame carries when a turn fails before any reply.
fn error_text(code: &str) -> String {
    match code {
        "authentication_failed" => {
            "Claude could not sign in. Run claude once to sign in again.".into()
        }
        "billing_error" => "Claude reported a billing problem with the plan.".into(),
        "rate_limit" => "Claude's usage limit was reached. Try again after it resets.".into(),
        "overloaded" => "Claude is overloaded. Try again shortly.".into(),
        "model_not_found" => "Claude does not have the selected model.".into(),
        other => format!("Claude returned an error: {other}"),
    }
}

// The CLI also puts its own `[ede_diagnostic]` bookkeeping in `errors`; those mean nothing to a
// user, so only the rest is shown.
fn result_error(v: &Value) -> String {
    let errors: Vec<String> = v["errors"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|e| {
            e.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| e.to_string())
        })
        .filter(|e| !e.contains("[ede_diagnostic]"))
        .collect();
    if !errors.is_empty() {
        return errors.join("; ");
    }
    match v["subtype"].as_str() {
        Some("error_max_turns") => "The run hit the maximum number of turns.".into(),
        Some("error_max_budget_usd") => "The run hit its cost budget.".into(),
        _ => "The run ended with an error.".into(),
    }
}

fn commands(init: &Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for list in [&init["slash_commands"], &init["skills"]] {
        for item in list.as_array().into_iter().flatten() {
            let name = item
                .as_str()
                .or_else(|| item["name"].as_str())
                .unwrap_or_default();
            let name = name.trim_start_matches('/');
            if !name.is_empty() && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}
#[cfg(test)]
mod tests {
    #[test]
    fn limit_resets_read_as_ms() {
        use serde_json::json;
        assert_eq!(
            super::resets(&json!(1_760_000_000)),
            Some(1_760_000_000_000)
        );
        assert_eq!(
            super::resets(&json!(1_760_000_000_000u64)),
            Some(1_760_000_000_000)
        );
        assert_eq!(super::resets(&json!(null)), None);
    }

    use super::*;

    #[test]
    fn args_carry_model_effort_permission_and_resume() {
        let mut launch = Launch::new(Agent::Claude, "/w");
        let a = args(&launch);
        assert!(a.windows(2).any(|w| w == ["--permission-mode", "default"]));
        assert!(a.contains(&"--verbose".to_string()));
        assert!(!a.contains(&"--model".to_string()));
        launch.model = Some("claude-opus-5-5".into());
        launch.effort = Some("high".into());
        launch.permission = Permission::Bypass;
        launch.resume = Some("abc".into());
        let a = args(&launch);
        assert!(a.windows(2).any(|w| w == ["--model", "claude-opus-5-5"]));
        assert!(a.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(a.windows(2).any(|w| w == ["--resume", "abc"]));
        assert!(a.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!a.contains(&"--permission-mode".to_string()));
    }

    #[test]
    fn context_counts_cache_and_takes_the_main_models_window() {
        let usage: Value = serde_json::from_str(
            r#"{"input_tokens":9,"cache_creation_input_tokens":13137,"cache_read_input_tokens":17943,"output_tokens":3}"#,
        )
        .unwrap();
        assert_eq!(context_used(&usage), Some(31092));
        assert_eq!(context_used(&Value::Null), None);
        let models: Value = serde_json::from_str(
            r#"{"claude-haiku-4-5":{"inputTokens":50,"cacheReadInputTokens":10,"contextWindow":200000},
                "claude-opus-5-5":{"inputTokens":9,"cacheReadInputTokens":17943,"cacheCreationInputTokens":13137,"contextWindow":1000000}}"#,
        )
        .unwrap();
        assert_eq!(context_window(&models), Some(1_000_000));
        assert_eq!(context_window(&Value::Null), None);
    }

    #[test]
    fn result_errors_skip_diagnostics() {
        let v: Value = serde_json::from_str(
            r#"{"subtype":"error_during_execution","errors":["[ede_diagnostic] x","real"]}"#,
        )
        .unwrap();
        assert_eq!(result_error(&v), "real");
        let v: Value =
            serde_json::from_str(r#"{"subtype":"error_max_turns","errors":[]}"#).unwrap();
        assert_eq!(result_error(&v), "The run hit the maximum number of turns.");
    }
}
