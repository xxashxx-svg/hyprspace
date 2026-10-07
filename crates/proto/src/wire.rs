// What crosses the channel between the UI and the engine. Commands go in, events come out.
// Session messages name the session they are about, so one event stream can serve every view;
// app-level requests (saved state, agents, resume list, clone) answer with an event of their own.
// Shape and reasons: docs/internals/overview.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{Agent, AgentInfo, AgentSession, AgentState, SubAgent};
use crate::folder::{FolderCommand, FolderEvent};
use crate::run::{Answer, Launch, Prompt, RunEvent};
use crate::skills::{SkillCommand, SkillEvent};
use crate::state::{AppState, Entry};
use crate::update::{UpdateCommand, UpdateEvent};
use crate::usage::{UsageCommand, UsageEvent};

/// Picked by whoever opens the session (the UI today), unique for the life of the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Start a structured session, and send `prompt` as its first run when there is one.
    /// With `journal`, the engine appends the session's prompts, answers and run events to that
    /// journal (see `LoadJournal`). Opening an id that is already open replaces that session.
    OpenStructured {
        id: SessionId,
        launch: Launch,
        prompt: Option<Prompt>,
        journal: Option<String>,
    },
    /// Start a run, or steer the live one: a prompt sent mid-run joins it.
    Send {
        id: SessionId,
        prompt: Prompt,
    },
    /// Stop the live run. The session stays open for the next prompt.
    Interrupt {
        id: SessionId,
    },
    /// Answer a `RunEvent::Approval`.
    Approve {
        id: SessionId,
        request: String,
        answer: Answer,
    },
    /// Read a journal back. Answered with `Event::Journal` for `id`.
    LoadJournal {
        id: SessionId,
        journal: String,
    },
    /// Spawn the default shell in a PTY sized `cols` x `rows`. With `run`, the engine types that
    /// agent's launch command into the shell once it is up, then `prompt` once the CLI is ready
    /// for it. The command never carries user text; the prompt goes in as keystrokes.
    OpenTerminal {
        id: SessionId,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        run: Option<Launch>,
        prompt: Option<String>,
    },
    /// Keystrokes, pastes and the emulator's answers to terminal queries.
    WriteTerminal {
        id: SessionId,
        bytes: Vec<u8>,
    },
    ResizeTerminal {
        id: SessionId,
        cols: u16,
        rows: u16,
    },
    /// End a session of either type and kill its process.
    Close {
        id: SessionId,
    },
    /// Find the file behind Claude's `[Image #n]` marker in a terminal session, for its preview.
    /// `conversation` is the Claude conversation the session is on. Answered with `ImageFound`.
    FindImage {
        id: SessionId,
        cwd: PathBuf,
        conversation: Option<String>,
        n: u32,
    },
    /// Open a file outside the app: in the user's code editor at `line` and `col` when one is
    /// installed, else with the OS default.
    OpenFile {
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
    /// Answered with `Event::State`.
    LoadState,
    SaveState {
        state: AppState,
    },
    /// Which agents are installed and what they can start with. Answered with `Event::Agents`.
    LoadAgents,
    /// The conversations `agent` saved for `cwd`. Answered with `Event::Resumable`.
    ListResumable {
        agent: Agent,
        cwd: PathBuf,
    },
    /// `git clone url` into `parent/name`, or straight into `parent` when `here`. Progress
    /// comes as `CloneProgress`, the end as `Cloned`, both tagged with `request`.
    Clone {
        request: u64,
        url: String,
        parent: PathBuf,
        name: String,
        here: bool,
    },
    /// The file tree, the git tab, the viewer and the open-in actions (`folder.rs`).
    Folder(FolderCommand),
    /// Live limits and local usage for the meter and Settings (`usage.rs`).
    Usage(UsageCommand),
    /// Settings' Skills view (`skills.rs`).
    Skills(SkillCommand),
    /// The app updating itself (`update.rs`).
    Update(UpdateCommand),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Run {
        id: SessionId,
        event: RunEvent,
    },
    /// Coalesced PTY output: at most one batch per frame under load.
    TerminalOutput {
        id: SessionId,
        bytes: Vec<u8>,
    },
    TerminalExit {
        id: SessionId,
        code: i32,
    },
    /// The conversation an agent in a terminal session is on, once it's known, so the thread
    /// can resume it after a restart. Claude's is claimed up front; Codex's is found on disk.
    TerminalConversation {
        id: SessionId,
        resume: String,
    },
    /// The agent CLI running in a terminal session changed: one was started by hand, or swapped
    /// for another, or quit back to the shell (`None`). `model` is what its command line or its
    /// status line names, when either does.
    TerminalAgent {
        id: SessionId,
        agent: Option<Agent>,
        model: Option<String>,
    },
    /// What the agent in a terminal session is doing, from its hooks.
    AgentState {
        id: SessionId,
        state: AgentState,
    },
    /// The same agent's work in more detail: one line on what it does now (the tool it runs,
    /// why it waits, what it last said) and the subagents still running, oldest first.
    AgentActivity {
        id: SessionId,
        doing: Option<String>,
        subs: Vec<SubAgent>,
    },
    /// The file behind `[Image #n]` in a terminal session, or None when there is no such image.
    ImageFound {
        id: SessionId,
        n: u32,
        path: Option<PathBuf>,
    },
    /// The engine could not do what a command asked. `message` is user-facing.
    Failed {
        id: SessionId,
        message: String,
    },
    /// A journal's entries, oldest first. Empty when there is none yet.
    Journal {
        id: SessionId,
        entries: Vec<Entry>,
    },
    /// The saved state, or the default on a first run. A file that would not parse was moved
    /// aside first, so nothing is lost by saving over it.
    State {
        state: AppState,
    },
    Agents {
        agents: Vec<AgentInfo>,
    },
    Resumable {
        agent: Agent,
        cwd: PathBuf,
        sessions: Vec<AgentSession>,
    },
    /// One of git's progress lines.
    CloneProgress {
        request: u64,
        line: String,
    },
    /// The new folder, or why the clone failed.
    Cloned {
        request: u64,
        result: Result<PathBuf, String>,
    },
    Folder(FolderEvent),
    Usage(UsageEvent),
    Skills(SkillEvent),
    Update(UpdateEvent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Agent;
    use crate::run::RunStatus;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
    }

    #[test]
    fn commands_survive_json() {
        let cmds = [
            Command::OpenStructured {
                id: SessionId(1),
                launch: Launch::new(Agent::Claude, "/tmp/x"),
                prompt: Some(Prompt::text("hi")),
                journal: Some("thread-4".into()),
            },
            Command::Send {
                id: SessionId(1),
                prompt: Prompt {
                    text: "look".into(),
                    images: vec![PathBuf::from("/tmp/a.png")],
                },
            },
            Command::Interrupt { id: SessionId(1) },
            Command::Approve {
                id: SessionId(1),
                request: "r1".into(),
                answer: Answer::AllowAlways,
            },
            Command::LoadJournal {
                id: SessionId(1),
                journal: "thread-4".into(),
            },
            Command::LoadState,
            Command::SaveState {
                state: AppState::default(),
            },
            Command::LoadAgents,
            Command::ListResumable {
                agent: Agent::Codex,
                cwd: PathBuf::from("/w"),
            },
            Command::Clone {
                request: 3,
                url: "https://github.com/a/b".into(),
                parent: PathBuf::from("/w"),
                name: "b".into(),
                here: false,
            },
            Command::OpenTerminal {
                id: SessionId(2),
                cwd: PathBuf::new(),
                cols: 80,
                rows: 24,
                run: Some(Launch::new(Agent::Codex, "/w")),
                prompt: Some("hi".into()),
            },
            Command::WriteTerminal {
                id: SessionId(2),
                bytes: b"ls\r".to_vec(),
            },
            Command::ResizeTerminal {
                id: SessionId(2),
                cols: 100,
                rows: 30,
            },
            Command::Close { id: SessionId(2) },
            Command::OpenFile {
                path: PathBuf::from("/w/a.rs"),
                line: Some(3),
                col: None,
            },
            Command::FindImage {
                id: SessionId(2),
                cwd: PathBuf::from("/w"),
                conversation: Some("c1".into()),
                n: 3,
            },
            Command::Folder(FolderCommand::Stage {
                cwd: PathBuf::from("/w"),
                path: "a.rs".into(),
                stage: true,
            }),
            Command::Folder(FolderCommand::Openers),
            Command::Usage(UsageCommand::Live {
                agent: Agent::Claude,
            }),
            Command::Usage(UsageCommand::Local {
                provider: "codex".into(),
            }),
            Command::Skills(SkillCommand::Write {
                cwd: PathBuf::from("/w"),
                scope: crate::agents::SkillScope::Project,
                kind: crate::agents::SkillKind::Skill,
                name: "fix".into(),
                content: "---
---
"
                .into(),
                replaces: Some((crate::agents::SkillScope::User, "old".into())),
            }),
            Command::Update(UpdateCommand::Install),
        ];
        for cmd in &cmds {
            assert_eq!(&round_trip(cmd), cmd);
        }
    }

    #[test]
    fn events_survive_json() {
        let events = [
            Event::Run {
                id: SessionId(1),
                event: RunEvent::Text { text: "hi".into() },
            },
            Event::Run {
                id: SessionId(1),
                event: RunEvent::Finished {
                    status: RunStatus::Done,
                    ms: 2000,
                    text: "hi".into(),
                    error: None,
                },
            },
            Event::TerminalOutput {
                id: SessionId(2),
                bytes: vec![0x1b, b'[', b'm'],
            },
            Event::TerminalExit {
                id: SessionId(2),
                code: 0,
            },
            Event::TerminalConversation {
                id: SessionId(2),
                resume: "019a-rollout".into(),
            },
            Event::TerminalAgent {
                id: SessionId(2),
                agent: Some(Agent::Codex),
                model: Some("gpt-5.5".into()),
            },
            Event::AgentState {
                id: SessionId(2),
                state: AgentState::Waiting,
            },
            Event::ImageFound {
                id: SessionId(2),
                n: 3,
                path: Some(PathBuf::from("/c/3.png")),
            },
            Event::AgentActivity {
                id: SessionId(2),
                doing: Some("Edit sync.rs".into()),
                subs: vec![SubAgent {
                    id: "t1".into(),
                    label: "Find the bug".into(),
                    started: 1,
                }],
            },
            Event::Failed {
                id: SessionId(3),
                message: "no shell".into(),
            },
            Event::Journal {
                id: SessionId(1),
                entries: vec![
                    Entry::Prompt {
                        prompt: Prompt::text("hi"),
                    },
                    Entry::Answer {
                        request: "r1".into(),
                        answer: Answer::Deny,
                    },
                ],
            },
            Event::CloneProgress {
                request: 3,
                line: "Receiving objects: 5%".into(),
            },
            Event::Cloned {
                request: 3,
                result: Err("no such repo".into()),
            },
            Event::Folder(FolderEvent::Dir {
                path: PathBuf::from("/w"),
                entries: Ok(vec![crate::folder::DirEntry {
                    name: "src".into(),
                    dir: true,
                }]),
            }),
            Event::Folder(FolderEvent::Git {
                cwd: PathBuf::from("/w"),
                status: Default::default(),
            }),
            Event::Usage(UsageEvent::StatusLine {
                id: SessionId(2),
                report: crate::usage::StatusReport {
                    model: Some("Opus 5.5".into()),
                    model_id: None,
                    windows: vec![crate::usage::LiveBar {
                        id: "five_hour".into(),
                        percent: 12.0,
                        ..Default::default()
                    }],
                },
            }),
            Event::Skills(SkillEvent::Read {
                scope: crate::agents::SkillScope::User,
                name: "fix".into(),
                content: Err("gone".into()),
            }),
            Event::Update(UpdateEvent::Checked {
                release: Some(crate::update::Release {
                    version: "0.22.0".into(),
                    notes: "- New".into(),
                }),
            }),
            Event::Update(UpdateEvent::Step {
                step: crate::update::Step::Downloading { percent: Some(40) },
            }),
        ];
        for event in &events {
            assert_eq!(&round_trip(event), event);
        }
    }

    #[test]
    fn tags_are_camel_case() {
        let json = serde_json::to_string(&Command::OpenTerminal {
            id: SessionId(7),
            cwd: PathBuf::new(),
            cols: 1,
            rows: 1,
            run: None,
            prompt: None,
        })
        .unwrap();
        assert!(json.contains(r#""type":"openTerminal""#), "{json}");
        assert!(json.contains(r#""id":7"#), "{json}");
    }
}
