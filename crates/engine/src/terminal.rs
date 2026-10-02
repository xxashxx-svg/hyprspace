// Terminal sessions that run an agent CLI interactively: the launch command typed into the shell
// (the Tauri app's actions.ts and TerminalPane), Claude's hooks wired back to the session for the
// sidebar's live state, and the composer's prompt typed in once the CLI is ready for it.
//
// The command is built from fixed flags and catalog ids only. User text never goes into it; the
// prompt goes in as keystrokes after the CLI is up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::{Agent, AgentState, Event, Launch, Permission, SessionId};
use serde_json::Value;

use crate::hooks::{self, Hook};
use crate::pty::{PtyManager, Spawn};
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

/// A terminal session the hooks can name.
struct Tracked {
    id: SessionId,
    state: AgentState,
    /// Typed in at Claude's first sign of life.
    prompt: Option<String>,
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
    /// Started with the first Claude terminal, so a run of the app without one opens no port.
    listener: Arc<Mutex<Option<Listener>>>,
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
        Self {
            ptys,
            tx,
            tracked: Arc::default(),
            listener: Arc::default(),
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
        if let Some(run) = &run {
            let mut settings = None;
            let mut has_transcript = false;
            if run.agent == Agent::Claude {
                if let Some((port, exe)) = self.listener() {
                    settings = hooks::write_settings(&hooks::hooks_dir(), port, &exe, &token(id));
                }
                if let Some(rid) = run.resume.as_deref() {
                    has_transcript = sessions::resume_mode(&cwd, rid) == Resume::Resume;
                }
                // a full repaint every frame keeps a resize from leaving stale rows behind;
                // claude only turns it on by itself for background sessions on Windows
                spawn
                    .env
                    .push(("CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT".into(), "1".into()));
                lock(&self.tracked).insert(
                    token(id),
                    Tracked {
                        id,
                        state: AgentState::Idle,
                        prompt: prompt.clone(),
                    },
                );
            }
            let as_arg = run.agent != Agent::Claude && prompt.is_some();
            if let Some(p) = prompt.as_ref().filter(|_| as_arg) {
                spawn.env.push((PROMPT_VAR.into(), p.clone()));
            }
            spawn.input = Some(command(run, settings.as_deref(), has_transcript, as_arg));
        }
        self.ptys.create(id, spawn, self.tx.clone())?;
        if run.is_some_and(|r| r.agent == Agent::Claude) && prompt.is_some() {
            self.give_up_on_prompt(id);
        }
        Ok(())
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
        let (id, changed, prompt) = {
            let mut tracked = lock(&self.tracked);
            let Some(t) = tracked.get_mut(key) else {
                return;
            };
            let next = payload.map_or(t.state, |p| next_state(t.state, p));
            let changed = (next != t.state).then_some(next);
            t.state = next;
            // the status line draws once the TUI is on screen and reading input; SessionStart
            // can fire before that
            let ready = matches!(hook, Hook::StatusLine(_));
            (t.id, changed, if ready { t.prompt.take() } else { None })
        };
        if let Some(state) = changed {
            let _ = self.tx.unbounded_send(Event::AgentState { id, state });
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
    fn hooks_for_a_tracked_session_report_its_state_and_type_its_prompt() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let terms = Terminals::new(PtyManager::default(), tx);
        let id = SessionId(5);
        lock(&terms.tracked).insert(
            token(id),
            Tracked {
                id,
                state: AgentState::Idle,
                prompt: Some("hi".into()),
            },
        );
        terms.on_hook(Hook::Agent(json!({
            "session": token(id),
            "payload": { "hook_event_name": "UserPromptSubmit" }
        })));
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::AgentState {
                id,
                state: AgentState::Working
            })
        );
        // the prompt waits for the status line, the sign the TUI reads input
        assert!(lock(&terms.tracked)[&token(id)].prompt.is_some());
        terms.on_hook(Hook::StatusLine(json!({ "session": token(id) })));
        assert!(lock(&terms.tracked)[&token(id)].prompt.is_none());
        // a hook for someone else's session changes nothing
        terms.on_hook(Hook::StatusLine(json!({ "session": "other" })));
        assert!(rx.try_recv().is_err());
    }
}
