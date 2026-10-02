// The Claude harness: the user's own `claude` binary in headless stream-json mode, the same
// invocation zeron's harness uses (MIT, see THIRD_PARTY_NOTICES.md). Inference stays on the
// user's subscription through their CLI; no SDK, no API key, no token.
//
// One process per session. Each prompt is a stdin user line; a prompt sent mid-run is a steer
// line the CLI folds in at its next step. Tool approvals arrive as `can_use_tool` control
// requests (--permission-prompt-tool stdio) and wait until the user answers.

mod resume;
mod tools;
mod wire;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use hyprspace_proto::{Agent, Launch, Permission, Prompt, RunEvent, RunStatus};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

use crate::spawn::{Tail, exit_message, spawn};
use crate::{Emit, Harness, Input, Session};

pub struct Claude {
    program: PathBuf,
    config: Option<PathBuf>,
    patience: Duration,
}

impl Default for Claude {
    fn default() -> Self {
        Self {
            program: PathBuf::from("claude"),
            config: None,
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
        let proc = spawn(&self.program, &args(&launch), &cwd)?;
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
    input: u64,
    output: u64,
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
    /// Approval requests waiting on the user, with the tool input to hand back on allow.
    approvals: HashMap<String, Value>,
    /// Tool calls of the main thread that have no result yet.
    open_tools: HashSet<String>,
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
                    Ok(Some(line)) => self.line(&line),
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
            Input::Answer { request, allow } => {
                if let Some(input) = self.approvals.remove(&request) {
                    self.write(wire::answer_line(&request, input, allow)).await;
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
            self.run = Some(Run {
                since: Instant::now(),
                steers: VecDeque::new(),
                interrupted: false,
                held: None,
                kill_at: None,
                input: 0,
                output: 0,
            });
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

    fn line(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            return;
        };
        // the CLI is still producing, so a held run end keeps waiting
        if let Some((_, at)) = self.run.as_mut().and_then(|r| r.held.as_mut()) {
            *at = Instant::now() + self.patience;
        }
        // subagent traffic carries the spawning tool's id; it belongs to that subagent's own
        // transcript, never the main reply
        let main = v["parent_tool_use_id"].is_null();
        match v["type"].as_str().unwrap_or_default() {
            "system" if v["subtype"] == "init" && !self.started => {
                self.started = true;
                let cwd = v["cwd"].as_str().map(PathBuf::from);
                (self.emit)(RunEvent::Started {
                    agent: Agent::Claude,
                    model: text(&v["model"]),
                    thread: text(&v["session_id"]),
                    cwd: cwd.unwrap_or_else(|| self.cwd.clone()),
                });
            }
            "stream_event" if main && v["event"]["type"] == "content_block_delta" => {
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
            "assistant" if main => {
                for block in blocks(&v) {
                    if block["type"] == "tool_use" {
                        let id = text(&block["id"]);
                        self.open_tools.insert(id.clone());
                        let tool = tools::decode(
                            block["name"].as_str().unwrap_or_default(),
                            &block["input"],
                        );
                        (self.emit)(RunEvent::Tool { id, tool });
                    }
                }
                if let Some(code) = v["error"].as_str() {
                    (self.emit)(RunEvent::Error {
                        message: error_text(code),
                    });
                }
            }
            "user" if main => {
                for block in blocks(&v) {
                    if block["type"] == "tool_result" {
                        let id = text(&block["tool_use_id"]);
                        self.open_tools.remove(&id);
                        (self.emit)(RunEvent::ToolDone {
                            id,
                            ok: !block["is_error"].as_bool().unwrap_or(false),
                            output: tools::result_text(&block["content"]),
                        });
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
                self.approvals
                    .insert(request.clone(), body["input"].clone());
                (self.emit)(RunEvent::Approval {
                    request,
                    tool,
                    reason: body["decision_reason"].as_str().map(str::to_string),
                });
            }
            "control_cancel_request" => {
                self.approvals
                    .remove(v["request_id"].as_str().unwrap_or_default());
            }
            "rate_limit_event" if v["rate_limit_info"]["status"] == "rejected" => {
                (self.emit)(RunEvent::Error {
                    message: "Claude's usage limit was reached. Try again after it resets.".into(),
                });
            }
            "result" => self.result(v),
            _ => {}
        }
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

    fn result(&mut self, v: Value) {
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

#[cfg(test)]
mod tests {
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
