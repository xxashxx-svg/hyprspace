// Terminal sessions that run an agent CLI interactively: the launch command typed into the shell
// (the Tauri app's actions.ts and TerminalPane), Claude's hooks wired back to the session for the
// sidebar's live state, and the composer's prompt typed in once the CLI is ready for it.
//
// What runs in a terminal can change under us: Claude stopped with Ctrl+C and started again by
// hand, or Codex started in its place. Every shell gets its session's hooks file and a `claude`
// that brings it along (ADR 0014), and a watcher reads which agent runs under each shell.
//
// The command is built from fixed flags and catalog ids only. User text never goes into it; the
// prompt goes in as keystrokes after the CLI is up.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::{
    Agent, AgentState, Event, Launch, Permission, SessionId, SubAgent, UsageEvent,
};
use serde_json::Value;

use crate::hooks::{self, Hook};
use crate::pty::{PtyManager, Spawn};
use crate::running::{Processes, Seen};
use crate::sessions::{self, Resume};

// Claude reports through its status line once its TUI is on screen, and its prompt is typed in
// then. Other CLIs give no signal, and keys typed on a guess can land on a startup dialog (Codex
// answered its own "update now?" with the Enter meant for the prompt), so they get the prompt
// as their start argument instead, read from this variable so no user text is ever in the
// command line.
const CLAUDE_BOOT: Duration = Duration::from_secs(30);
const PROMPT_VAR: &str = "HYPRSPACE_PROMPT";
// Enter goes a beat after the text so the TUI settles the typed prompt first
const ENTER_AFTER: Duration = Duration::from_millis(300);
// Codex saves its rollout on the first turn, which can be a while after the pane opens
const CODEX_POLL: Duration = Duration::from_secs(2);
const CODEX_WATCH: Duration = Duration::from_secs(15 * 60);
/// How often the processes under each shell are read for the agent running there.
const WATCH_EVERY: Duration = Duration::from_secs(2);
/// This session's hooks file, for the `claude` wrapper in its shell.
const SETTINGS_VAR: &str = "HYPRSPACE_CLAUDE_SETTINGS";

/// PowerShell's `claude` in every terminal: the real one, with this session's hooks unless the
/// command already names a settings file. Defined after the user's profile, so it is the one
/// that runs. Single quotes only, so it survives being one argument on the command line.
#[cfg(windows)]
const PS_CLAUDE: &str = "function global:claude { $c = Get-Command claude -CommandType Application, ExternalScript -ErrorAction SilentlyContinue | Select-Object -First 1; if (-not $c) { Write-Error 'claude is not installed, or is not on PATH.'; return }; if ($env:HYPRSPACE_CLAUDE_SETTINGS -and $args -notcontains '--settings') { & $c.Source --settings $env:HYPRSPACE_CLAUDE_SETTINGS @args } else { & $c.Source @args } }";

/// Puts the hooks-adding `claude` in front of the real one for this session's shell.
#[cfg(windows)]
fn wrap_claude(spawn: &mut Spawn) {
    if spawn.shell.is_none() {
        spawn
            .args
            .extend(["-NoExit", "-Command", PS_CLAUDE].map(String::from));
    }
}

#[cfg(not(windows))]
fn wrap_claude(spawn: &mut Spawn) {
    if let Some(bin) = hooks::claude_shim(&hooks::hooks_dir()) {
        let path = std::env::var("PATH").unwrap_or_default();
        spawn
            .env
            .push(("PATH".into(), format!("{}:{path}", bin.display())));
    }
}

/// Model ids and efforts come from the catalog, but a custom one could hold anything, so
/// anything past a plain token is quoted (and loses its own quotes).
fn quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('"', ""))
    }
}

/// The shell's way to pass `PROMPT_VAR` as one argument: PowerShell on Windows, a POSIX-style
/// shell (zsh, bash, fish) elsewhere.
fn prompt_arg() -> String {
    if cfg!(windows) {
        format!("$env:{PROMPT_VAR}")
    } else {
        format!("\"${PROMPT_VAR}\"")
    }
}

/// The command typed into the shell for `run`. `settings` is Claude's hooks file,
/// `has_transcript` whether Claude already saved the conversation `run.resume` names, and
/// `prompt` whether a CLI other than Claude starts with the prompt in `PROMPT_VAR`.
pub fn command(
    run: &Launch,
    settings: Option<&Path>,
    has_transcript: bool,
    prompt: bool,
) -> String {
    let mut out = vec![run.agent.cli().to_string()];
    let model = run.model.as_deref().filter(|m| !m.is_empty());
    let effort = run.effort.as_deref().filter(|e| !e.is_empty());
    match run.agent {
        Agent::Claude => {
            if let Some(path) = settings {
                out.push(format!("--settings \"{}\"", path.display()));
            }
            // the thread owns its conversation id: claim it the first time, resume it after
            if let Some(id) = run.resume.as_deref() {
                let flag = if has_transcript {
                    "--resume"
                } else {
                    "--session-id"
                };
                out.push(format!("{flag} {}", quote(id)));
            }
            match run.permission {
                Permission::Plan => out.push("--permission-mode plan".into()),
                Permission::Ask => {}
                Permission::Auto => out.push("--permission-mode acceptEdits".into()),
                Permission::Bypass => out.push("--dangerously-skip-permissions".into()),
            }
            if let Some(m) = model {
                out.push(format!("--model {}", quote(m)));
            }
            if let Some(e) = effort {
                out.push(format!("--effort {}", quote(e)));
            }
        }
        Agent::Codex => {
            if let Some(id) = run.resume.as_deref() {
                out.push(format!("resume {}", quote(id)));
            }
            // the presets in docs/adr/0004-harness-protocol.md
            out.push(
                match run.permission {
                    Permission::Plan => "--sandbox read-only --ask-for-approval never",
                    Permission::Ask => "--sandbox read-only --ask-for-approval on-request",
                    Permission::Auto => "--sandbox workspace-write --ask-for-approval on-request",
                    Permission::Bypass => "--dangerously-bypass-approvals-and-sandbox",
                }
                .into(),
            );
            if let Some(m) = model {
                out.push(format!("-m {}", quote(m)));
            }
            // bare, not model_reasoning_effort="high": PowerShell has no \" escape, so the quoted
            // form split into two arguments there (the Tauri app learned this the hard way)
            if let Some(e) = effort {
                out.push(format!(
                    "-c {}",
                    quote(&format!("model_reasoning_effort={e}"))
                ));
            }
        }
        Agent::Gemini => {
            if run.permission == Permission::Bypass {
                out.push("--yolo".into());
            }
            if let Some(m) = model {
                out.push(format!("-m {}", quote(m)));
            }
            if prompt {
                out.push(format!("-i {}", prompt_arg()));
            }
        }
    }
    if prompt && run.agent == Agent::Codex {
        out.push(prompt_arg());
    }
    out.join(" ")
}

/// The state after one hook, following the Tauri app's stores/agentStatus.ts.
pub fn next_state(cur: AgentState, payload: &Value) -> AgentState {
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match event {
        // startup, resume, /clear, a compaction: whatever was running is void. This is what
        // ends a /clear, which submits a prompt but never produces a turn for Stop to close.
        "SessionStart" => AgentState::Idle,
        "UserPromptSubmit" => AgentState::Working,
        // Claude sends this for a real block ("needs your permission to use Bash") and for an
        // idle nudge a minute after a turn ("waiting for your input"), which asks nothing
        "Notification" => {
            let msg = payload.get("message").and_then(Value::as_str);
            if msg.is_some_and(|m| m.to_lowercase().contains("waiting for your input")) {
                cur
            } else {
                AgentState::Waiting
            }
        }
        "Stop" => AgentState::Done,
        "PreToolUse" => AgentState::Working,
        // approving a permission produces no hook of its own; the tool running is the sign
        "PostToolUse" if cur == AgentState::Waiting => AgentState::Working,
        _ => cur,
    }
}

/// "Edit sync.rs", "Bash cargo check", "lualink run_lua": a tool and its most telling argument.
/// MCP tools arrive as `mcp__<server>__<tool>`, an id rather than a name.
fn tool_label(tool: &str, input: &Value) -> String {
    let name = match tool.strip_prefix("mcp__").and_then(|t| t.split_once("__")) {
        Some((server, tool)) => format!("{server} {tool}"),
        None => tool.to_string(),
    };
    let text = |k: &str| input.get(k).and_then(Value::as_str).map(str::trim);
    let file = ["file_path", "path", "notebook_path"]
        .into_iter()
        .find_map(text)
        .and_then(|p| p.rsplit(['/', '\\']).find(|s| !s.is_empty()));
    let arg = file.or_else(|| {
        ["command", "pattern", "query", "description"]
            .into_iter()
            .find_map(text)
            .filter(|s| !s.is_empty())
    });
    match arg {
        Some(arg) => format!("{name} {}", trim(arg, 44)),
        None => name,
    }
}

/// One line, at most `n` characters.
fn trim(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}...", one.chars().take(n - 3).collect::<String>())
    } else {
        one
    }
}

/// What the agent is doing and which subagents run after one hook, following the Tauri app's
/// stores/agentStatus.ts. `now` is in ms since the epoch.
pub fn next_activity(
    doing: &Option<String>,
    subs: &[SubAgent],
    payload: &Value,
    now: u64,
) -> (Option<String>, Vec<SubAgent>) {
    let text = |k: &str| payload.get(k).and_then(Value::as_str).unwrap_or_default();
    let mut doing = doing.clone();
    let mut next = subs.to_vec();
    match text("hook_event_name") {
        "SessionStart" => {
            doing = None;
            next.clear();
        }
        "UserPromptSubmit" => doing = Some("Thinking".into()),
        "Notification" => {
            let msg = text("message");
            if !msg.to_lowercase().contains("waiting for your input") {
                doing = Some(trim(
                    if msg.is_empty() {
                        "Waiting for you"
                    } else {
                        msg
                    },
                    60,
                ));
            }
        }
        "Stop" => {
            // what it concluded, rather than just that it stopped
            doing = Some(trim(text("last_assistant_message"), 80)).filter(|s| !s.is_empty());
        }
        "PreToolUse" => {
            let tool = text("tool_name");
            let input = payload.get("tool_input").unwrap_or(&Value::Null);
            // "Agent" on current Claude, "Task" on older builds
            if tool == "Agent" || tool == "Task" {
                let label = first_text(input, &["description", "subagent_type"]);
                doing = Some(format!("Delegating {}", trim(&label, 44)));
                // shows at once; Claude's own list replaces it at the next Stop or SubagentStop
                next.push(SubAgent {
                    id: format!("pending-{now}-{}", next.len()),
                    label,
                    started: now,
                });
            } else if !tool.is_empty() {
                doing = Some(tool_label(tool, input));
            }
        }
        _ => {}
    }
    // Claude's own list of what still runs rides along on Stop and SubagentStop. It beats
    // guessing from start and stop hooks: SubagentStop fires while a backgrounded subagent is
    // still running. An empty list means nothing is. The list holds every kind of background
    // work, shells, monitors and artifact watches too; only subagents get a card. An entry with
    // no type comes from a Claude that predates the field, and counts.
    if let Some(tasks) = payload.get("background_tasks").and_then(Value::as_array) {
        next = tasks
            .iter()
            .filter(|t| t.get("status").and_then(Value::as_str) == Some("running"))
            .filter(|t| {
                t.get("type")
                    .and_then(Value::as_str)
                    .is_none_or(|k| k == "subagent")
            })
            .filter_map(|t| {
                let id = t
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())?;
                // its start stays put, so its age doesn't reset with every list
                let started = subs.iter().find(|s| s.id == id).map_or(now, |s| s.started);
                Some(SubAgent {
                    id: id.to_string(),
                    label: first_text(t, &["description", "agent_type"]),
                    started,
                })
            })
            .collect();
    }
    (doing, next)
}

/// The first of `keys` with text in it, or "Subagent".
fn first_text(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|k| v.get(k).and_then(Value::as_str))
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("Subagent")
        .to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A terminal session the hooks can name.
struct Tracked {
    id: SessionId,
    state: AgentState,
    /// Typed in at Claude's first sign of life.
    prompt: Option<String>,
    doing: Option<String>,
    subs: Vec<SubAgent>,
}

impl Tracked {
    fn new(id: SessionId, prompt: Option<String>) -> Self {
        Self {
            id,
            state: AgentState::Idle,
            prompt,
            doing: None,
            subs: Vec::new(),
        }
    }
}

struct Listener {
    port: u16,
    exe: PathBuf,
}

#[derive(Clone)]
pub struct Terminals {
    ptys: PtyManager,
    tx: UnboundedSender<Event>,
    /// Hook token to session. Tokens carry the process id so two app instances sharing the
    /// hooks folder never clean up each other's files.
    tracked: Arc<Mutex<HashMap<String, Tracked>>>,
    /// Started with the first terminal, so a run of the app without one opens no port.
    listener: Arc<Mutex<Option<Listener>>>,
    /// The agent last seen in each session, from its processes or its hooks.
    seen: Arc<Mutex<HashMap<SessionId, Seen>>>,
}

fn token(id: SessionId) -> String {
    format!("hs{}-{}", std::process::id(), id.0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Types `prompt`, then Enter a beat later.
fn type_prompt(ptys: &PtyManager, id: SessionId, prompt: &str) {
    let _ = ptys.write(id, prompt.as_bytes());
    thread::sleep(ENTER_AFTER);
    let _ = ptys.write(id, b"\r");
}

impl Terminals {
    pub fn new(ptys: PtyManager, tx: UnboundedSender<Event>) -> Self {
        let this = Self {
            ptys,
            tx,
            tracked: Arc::default(),
            listener: Arc::default(),
            seen: Arc::default(),
        };
        let watcher = this.clone();
        thread::spawn(move || watcher.watch());
        this
    }

    /// Reads which agent runs under each shell, for as long as the app runs. Nothing is read
    /// while there are no terminals.
    fn watch(&self) {
        let mut procs = Processes::new();
        loop {
            thread::sleep(WATCH_EVERY);
            let shells = self.ptys.pids();
            lock(&self.seen).retain(|id, _| shells.iter().any(|(s, _)| s == id));
            if shells.is_empty() {
                continue;
            }
            for (id, seen) in procs.agents(&shells) {
                self.saw(id, seen);
            }
        }
    }

    /// Reports the agent under a shell when it is not the one seen there last.
    fn saw(&self, id: SessionId, now: Option<Seen>) {
        {
            let mut seen = lock(&self.seen);
            if seen.get(&id).map(|s| s.agent) == now.as_ref().map(|s| s.agent) {
                return;
            }
            match &now {
                Some(s) => seen.insert(id, s.clone()),
                None => seen.remove(&id),
            };
        }
        if now.is_none() {
            // the agent quit, perhaps mid-turn: whatever it was doing is over
            if let Some(t) = lock(&self.tracked).get_mut(&token(id)) {
                t.state = AgentState::Idle;
                t.doing = None;
                t.subs.clear();
            }
        }
        let _ = self.tx.unbounded_send(Event::TerminalAgent {
            id,
            agent: now.as_ref().map(|s| s.agent),
            model: now.as_ref().and_then(|s| s.model.clone()),
        });
        if let Some(resume) = now.and_then(|s| s.resume) {
            let _ = self
                .tx
                .unbounded_send(Event::TerminalConversation { id, resume });
        }
    }

    /// Claude's hooks say Claude runs in this session, on which conversation and, from its
    /// status line, which model: news when it was started by hand, or switched with /model.
    fn heard(&self, id: SessionId, conversation: Option<&str>, model: Option<&str>) {
        let (agent_news, resume) = {
            let mut seen = lock(&self.seen);
            let mut news = false;
            let s = seen.entry(id).or_insert_with(|| {
                news = true;
                Seen {
                    agent: Agent::Claude,
                    model: None,
                    resume: None,
                }
            });
            if s.agent != Agent::Claude {
                *s = Seen {
                    agent: Agent::Claude,
                    model: None,
                    resume: None,
                };
                news = true;
            }
            if let Some(m) = model.filter(|m| s.model.as_deref() != Some(*m)) {
                s.model = Some(m.to_string());
                news = true;
            }
            let resume = conversation
                .filter(|c| s.resume.as_deref() != Some(*c))
                .map(String::from);
            if let Some(c) = &resume {
                s.resume = Some(c.clone());
            }
            (news.then(|| s.model.clone()), resume)
        };
        if let Some(model) = agent_news {
            let _ = self.tx.unbounded_send(Event::TerminalAgent {
                id,
                agent: Some(Agent::Claude),
                model,
            });
        }
        if let Some(resume) = resume {
            let _ = self
                .tx
                .unbounded_send(Event::TerminalConversation { id, resume });
        }
    }

    pub fn ptys(&self) -> &PtyManager {
        &self.ptys
    }

    /// The hook listener's port and the binary hooks run, starting the listener if needed.
    fn listener(&self) -> Option<(u16, PathBuf)> {
        let mut slot = lock(&self.listener);
        if slot.is_none() {
            let exe = std::env::current_exe().ok()?;
            let this = self.clone();
            let port = hooks::listen(move |hook| this.on_hook(hook)).ok()?;
            *slot = Some(Listener { port, exe });
        }
        slot.as_ref().map(|l| (l.port, l.exe.clone()))
    }

    pub fn open(
        &self,
        id: SessionId,
        cwd: PathBuf,
        size: (u16, u16),
        run: Option<Launch>,
        prompt: Option<String>,
    ) -> anyhow::Result<()> {
        let mut spawn = Spawn {
            cwd: cwd.clone(),
            cols: size.0,
            rows: size.1,
            ..Default::default()
        };
        let prompt = prompt.filter(|p| !p.trim().is_empty());
        let claude = run.as_ref().is_some_and(|r| r.agent == Agent::Claude);
        // every shell gets this session's hooks, so a claude started in it by hand reports too
        let settings = self.listener().and_then(|(port, exe)| {
            hooks::write_settings(&hooks::hooks_dir(), port, &exe, &token(id))
        });
        if let Some(path) = &settings {
            spawn
                .env
                .push((SETTINGS_VAR.into(), path.display().to_string()));
            wrap_claude(&mut spawn);
        }
        // a full repaint every frame keeps a resize from leaving stale rows behind; claude only
        // turns it on by itself for background sessions on Windows
        spawn
            .env
            .push(("CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT".into(), "1".into()));
        lock(&self.tracked).insert(
            token(id),
            Tracked::new(id, prompt.clone().filter(|_| claude)),
        );
        if let Some(run) = &run {
            let mut has_transcript = false;
            if claude && let Some(rid) = run.resume.as_deref() {
                has_transcript = sessions::resume_mode(&cwd, rid) == Resume::Resume;
            }
            let as_arg = !claude && prompt.is_some();
            if let Some(p) = prompt.as_ref().filter(|_| as_arg) {
                spawn.env.push((PROMPT_VAR.into(), p.clone()));
            }
            let settings = settings.as_deref().filter(|_| claude);
            spawn.input = Some(command(run, settings, has_transcript, as_arg));
        }
        let new_codex = run
            .as_ref()
            .is_some_and(|r| r.agent == Agent::Codex && r.resume.is_none());
        // the conversations already in this folder, so the new one stands out once Codex saves it
        let before = new_codex.then(|| {
            sessions::list("codex", &cwd)
                .into_iter()
                .map(|s| s.id)
                .collect()
        });
        self.ptys.create(id, spawn, self.tx.clone())?;
        if let Some(before) = before {
            self.find_codex_conversation(id, cwd, before);
        }
        if run.is_some_and(|r| r.agent == Agent::Claude) && prompt.is_some() {
            self.give_up_on_prompt(id);
        }
        Ok(())
    }

    /// Codex can't be handed a conversation id the way Claude can, so watch its rollouts for the
    /// one this session starts and report it. Gives up when the session ends or after a while.
    fn find_codex_conversation(&self, id: SessionId, cwd: PathBuf, known: HashSet<String>) {
        let tx = self.tx.clone();
        let ptys = self.ptys.clone();
        thread::spawn(move || {
            for _ in 0..(CODEX_WATCH.as_secs() / CODEX_POLL.as_secs()) {
                thread::sleep(CODEX_POLL);
                if !ptys.contains(id) {
                    return;
                }
                let fresh = sessions::list("codex", &cwd)
                    .into_iter()
                    .filter(|s| !known.contains(&s.id))
                    .max_by_key(|s| s.modified);
                if let Some(s) = fresh {
                    let _ = tx.unbounded_send(Event::TerminalConversation { id, resume: s.id });
                    return;
                }
            }
        });
    }

    /// A Claude that never comes up keeps its prompt untyped, and says so.
    fn give_up_on_prompt(&self, id: SessionId) {
        let this = self.clone();
        thread::spawn(move || {
            thread::sleep(CLAUDE_BOOT);
            let dropped = lock(&this.tracked)
                .get_mut(&token(id))
                .and_then(|t| t.prompt.take());
            if let Some(prompt) = dropped {
                let _ = this.tx.unbounded_send(Event::Failed {
                    id,
                    message: format!(
                        "Claude did not start, so the prompt was not typed in: {prompt}"
                    ),
                });
            }
        });
    }

    fn on_hook(&self, hook: Hook) {
        let (body, payload) = match &hook {
            Hook::Agent(v) => (v, v.get("payload")),
            Hook::StatusLine(v) => (v, None),
        };
        let Some(key) = body.get("session").and_then(Value::as_str) else {
            return;
        };
        let conversation = payload
            .and_then(|p| p.get("session_id"))
            .or_else(|| body.pointer("/statusLine/session_id"))
            .and_then(Value::as_str)
            .map(String::from);
        let model = body
            .pointer("/statusLine/model/id")
            .and_then(Value::as_str)
            .map(String::from);
        let (id, changed, activity, prompt) = {
            let mut tracked = lock(&self.tracked);
            let Some(t) = tracked.get_mut(key) else {
                return;
            };
            let next = payload.map_or(t.state, |p| next_state(t.state, p));
            let changed = (next != t.state).then_some(next);
            t.state = next;
            let mut activity = None;
            if let Some(p) = payload {
                let (doing, subs) = next_activity(&t.doing, &t.subs, p, now_ms());
                if doing != t.doing || subs != t.subs {
                    t.doing = doing.clone();
                    t.subs = subs.clone();
                    activity = Some((doing, subs));
                }
            }
            // the status line draws once the TUI is on screen and reading input; SessionStart
            // can fire before that
            let ready = matches!(hook, Hook::StatusLine(_));
            (
                t.id,
                changed,
                activity,
                if ready { t.prompt.take() } else { None },
            )
        };
        self.heard(id, conversation.as_deref(), model.as_deref());
        if let Some(state) = changed {
            let _ = self.tx.unbounded_send(Event::AgentState { id, state });
        }
        if let Some((doing, subs)) = activity {
            let _ = self
                .tx
                .unbounded_send(Event::AgentActivity { id, doing, subs });
        }
        if let Hook::StatusLine(v) = &hook {
            let report = crate::usage::status::report(&v["statusLine"]);
            let _ = self
                .tx
                .unbounded_send(Event::Usage(UsageEvent::StatusLine { id, report }));
        }
        if let Some(prompt) = prompt {
            let ptys = self.ptys.clone();
            thread::spawn(move || type_prompt(&ptys, id, &prompt));
        }
    }

    pub fn close(&self, id: SessionId) {
        if lock(&self.tracked).remove(&token(id)).is_some() {
            hooks::cleanup(&hooks::hooks_dir(), &token(id));
        }
        self.ptys.kill(id);
    }

    /// Every session dies with the app: ConPTY hosts orphan otherwise.
    pub fn shutdown(&self) {
        for (key, _) in lock(&self.tracked).drain() {
            hooks::cleanup(&hooks::hooks_dir(), &key);
        }
        self.ptys.kill_all();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(agent: Agent) -> Launch {
        Launch::new(agent, "/w")
    }

    #[test]
    fn claude_claims_its_id_then_resumes_it() {
        let mut r = run(Agent::Claude);
        r.resume = Some("0b6e1f9c-1111-4222-8333-944445555666".into());
        let settings = Path::new("/h/.hyprspace/agent-hooks/hs1-2.json");
        assert_eq!(
            command(&r, Some(settings), false, false),
            format!(
                "claude --settings \"{}\" --session-id 0b6e1f9c-1111-4222-8333-944445555666",
                settings.display()
            )
        );
        assert!(command(&r, None, true, false).starts_with("claude --resume 0b6e1f9c-"));
        assert_eq!(command(&run(Agent::Claude), None, false, false), "claude");
    }

    #[test]
    fn flags_follow_permission_model_and_effort() {
        let mut r = run(Agent::Claude);
        r.permission = Permission::Auto;
        r.model = Some("claude-opus-5-5".into());
        r.effort = Some("high".into());
        assert_eq!(
            command(&r, None, false, false),
            "claude --permission-mode acceptEdits --model claude-opus-5-5 --effort high"
        );
        r.permission = Permission::Bypass;
        assert!(command(&r, None, false, false).contains("--dangerously-skip-permissions"));

        let mut c = run(Agent::Codex);
        c.resume = Some("th-1".into());
        c.model = Some("gpt-6-luna".into());
        c.effort = Some("low".into());
        assert_eq!(
            command(&c, None, false, false),
            "codex resume th-1 --sandbox read-only --ask-for-approval on-request -m gpt-6-luna -c \"model_reasoning_effort=low\""
        );

        let mut g = run(Agent::Gemini);
        g.permission = Permission::Bypass;
        g.model = Some("gemini-3-pro".into());
        assert_eq!(
            command(&g, None, false, false),
            "gemini --yolo -m gemini-3-pro"
        );
    }

    #[test]
    fn other_clis_take_the_prompt_from_the_environment_not_the_command() {
        let var = prompt_arg();
        let c = command(&run(Agent::Codex), None, false, true);
        assert!(c.ends_with(&format!(" {var}")), "{c}");
        let g = command(&run(Agent::Gemini), None, false, true);
        assert!(g.ends_with(&format!("-i {var}")), "{g}");
        assert!(var.contains(PROMPT_VAR));
    }

    #[test]
    fn odd_model_ids_are_quoted_and_cannot_break_out() {
        let mut r = run(Agent::Claude);
        r.model = Some("my model\"; rm -rf".into());
        assert_eq!(
            command(&r, None, false, false),
            "claude --model \"my model; rm -rf\""
        );
    }

    #[test]
    fn hooks_drive_the_state_like_the_tauri_app() {
        let hook = |name: &str| json!({ "hook_event_name": name });
        let mut s = AgentState::Idle;
        s = next_state(s, &hook("UserPromptSubmit"));
        assert_eq!(s, AgentState::Working);
        s = next_state(
            s,
            &json!({ "hook_event_name": "Notification", "message": "Claude needs your permission to use Bash" }),
        );
        assert_eq!(s, AgentState::Waiting);
        assert_eq!(next_state(s, &hook("PreToolUse")), AgentState::Working);
        s = next_state(s, &hook("PostToolUse"));
        assert_eq!(s, AgentState::Working);
        s = next_state(s, &hook("Stop"));
        assert_eq!(s, AgentState::Done);
        // the idle nudge asks nothing
        let nudge = json!({ "hook_event_name": "Notification", "message": "Claude is waiting for your input" });
        assert_eq!(next_state(s, &nudge), AgentState::Done);
        assert_eq!(next_state(s, &hook("SessionStart")), AgentState::Idle);
        assert_eq!(next_state(s, &hook("SubagentStop")), AgentState::Done);
    }

    #[test]
    fn hooks_say_what_it_does_and_which_subagents_run() {
        let step = |doing: &Option<String>, subs: &[SubAgent], p: Value, now| {
            next_activity(doing, subs, &p, now)
        };
        let (doing, subs) = step(
            &None,
            &[],
            json!({ "hook_event_name": "PreToolUse", "tool_name": "Edit",
                    "tool_input": { "file_path": "C:/w/src/sync.rs" } }),
            1,
        );
        assert_eq!(doing.as_deref(), Some("Edit sync.rs"));
        assert!(subs.is_empty());
        let mcp = json!({ "hook_event_name": "PreToolUse", "tool_name": "mcp__lualink__run_lua",
                          "tool_input": {} });
        assert_eq!(
            step(&None, &[], mcp, 1).0.as_deref(),
            Some("lualink run_lua")
        );

        // a delegation shows at once, then Claude's own list takes over and keeps its start
        let (doing, subs) = step(
            &doing,
            &subs,
            json!({ "hook_event_name": "PreToolUse", "tool_name": "Agent",
                    "tool_input": { "description": "Find the bug" } }),
            2,
        );
        assert_eq!(doing.as_deref(), Some("Delegating Find the bug"));
        assert_eq!(subs.len(), 1);
        let listed = json!({ "hook_event_name": "SubagentStop", "background_tasks": [
            { "id": "t1", "type": "subagent", "status": "running", "description": "Find the bug" },
            { "id": "t2", "status": "completed", "description": "Old one" },
            // other background work is no subagent
            { "id": "t3", "type": "shell", "status": "running", "description": "cargo build" },
            { "id": "t4", "type": "monitor_ws", "status": "running",
              "description": "live updates for artifact https://claude.ai/artifact/x" }
        ]});
        let (_, subs) = step(&doing, &subs, listed.clone(), 3);
        assert_eq!(subs.len(), 1);
        assert_eq!((subs[0].id.as_str(), subs[0].started), ("t1", 3));
        let (_, again) = step(&doing, &subs, listed, 9);
        assert_eq!(again[0].started, 3);

        // a stop says what it concluded and its empty list clears the subagents
        let (doing, subs) = step(
            &doing,
            &subs,
            json!({ "hook_event_name": "Stop", "last_assistant_message": "Fixed it.\n\nDone",
                    "background_tasks": [] }),
            10,
        );
        assert_eq!(doing.as_deref(), Some("Fixed it. Done"));
        assert!(subs.is_empty());
        // the idle nudge asks nothing
        let nudge = json!({ "hook_event_name": "Notification",
                            "message": "Claude is waiting for your input" });
        assert_eq!(step(&doing, &subs, nudge, 11).0, doing);
    }

    #[test]
    fn hooks_for_a_tracked_session_report_its_state_and_type_its_prompt() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let terms = Terminals::new(PtyManager::default(), tx);
        let id = SessionId(5);
        lock(&terms.tracked).insert(token(id), Tracked::new(id, Some("hi".into())));
        terms.on_hook(Hook::Agent(json!({
            "session": token(id),
            "payload": { "hook_event_name": "UserPromptSubmit", "session_id": "c1" }
        })));
        // a hook means claude runs here, on that conversation, even one started by hand
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::TerminalAgent {
                id,
                agent: Some(Agent::Claude),
                model: None
            })
        );
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::TerminalConversation {
                id,
                resume: "c1".into()
            })
        );
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::AgentState {
                id,
                state: AgentState::Working
            })
        );
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::AgentActivity {
                id,
                doing: Some("Thinking".into()),
                subs: vec![]
            })
        );
        // the prompt waits for the status line, the sign the TUI reads input
        assert!(lock(&terms.tracked)[&token(id)].prompt.is_some());
        terms.on_hook(Hook::StatusLine(json!({
            "session": token(id),
            "statusLine": { "rate_limits": { "five_hour": { "used_percentage": 30 } } }
        })));
        assert!(lock(&terms.tracked)[&token(id)].prompt.is_none());
        // and its limits go to the meter
        match rx.try_recv().ok() {
            Some(Event::Usage(UsageEvent::StatusLine { id: got, report })) => {
                assert_eq!(got, id);
                assert_eq!(report.windows[0].percent, 30.0);
            }
            other => panic!("unexpected {other:?}"),
        }
        // /model shows in the status line
        terms.on_hook(Hook::StatusLine(json!({
            "session": token(id),
            "statusLine": { "session_id": "c1", "model": { "id": "claude-sonnet-5-5" } }
        })));
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::TerminalAgent {
                id,
                agent: Some(Agent::Claude),
                model: Some("claude-sonnet-5-5".into())
            })
        );
        while rx.try_recv().is_ok() {}
        // a hook for someone else's session changes nothing
        terms.on_hook(Hook::StatusLine(json!({ "session": "other" })));
        assert!(rx.try_recv().is_err());
    }
}
