// What the engine knows about the agent CLIs on this machine: install and sign-in state, the
// conversations each one saved, and the skills they can run.

use serde::{Deserialize, Serialize};

/// Install and sign-in state for one provider CLI. Display only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub installed: bool,
    pub version: Option<String>,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub detail: Option<String>,
}

/// One conversation a CLI saved on disk, as the composer's resume list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    /// unix ms
    pub modified: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillScope {
    Project,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillKind {
    /// `.claude/skills/<name>/SKILL.md`
    Skill,
    /// `.claude/commands/<name>.md`, the older form.
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillItem {
    pub name: String,
    /// "/name"
    pub command: String,
    pub description: String,
    /// The markdown after the frontmatter: the instructions, for agents other than Claude.
    pub body: String,
    pub scope: SkillScope,
    pub kind: SkillKind,
}

/// An agent CLI the app can start. Claude and Codex run as structured sessions or in a
/// terminal; Gemini only in a terminal for now (docs/REWRITE.md, open questions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude",
            Agent::Codex => "Codex",
            Agent::Gemini => "Gemini",
        }
    }

    /// The CLI's command name, which is also its provider id.
    pub fn cli(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Gemini => "gemini",
        }
    }

    /// Whether a harness drives it over a machine protocol. The rest only run in a terminal.
    pub fn structured(self) -> bool {
        !matches!(self, Agent::Gemini)
    }
}

/// What the agent in a terminal session is doing, as its hooks report it. Only Claude has hooks,
/// so other terminal sessions stay `Idle`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    #[default]
    Idle,
    Working,
    /// Blocked on the user: a permission prompt or a question.
    Waiting,
    /// The last turn finished.
    Done,
}

/// A subagent an agent in a terminal session delegated to, still running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgent {
    pub id: String,
    /// The task it was given, or its agent type.
    pub label: String,
    /// When it started, in ms since the epoch.
    pub started: u64,
}

/// One model the composer offers for an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// What the CLI takes. Empty means "let the CLI pick".
    pub id: String,
    pub label: String,
    pub note: Option<String>,
    /// Effort levels this model takes, lowest first. Empty means the agent's own list.
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
}

/// The models and effort levels one agent can start with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCatalog {
    pub agent: Agent,
    pub models: Vec<ModelInfo>,
    /// The levels the CLI takes when a model does not list its own.
    pub efforts: Vec<String>,
}

impl AgentCatalog {
    /// The catalog's entry for `model`, whatever context window tag (`[1m]`) it carries.
    pub fn model(&self, model: &str) -> Option<&ModelInfo> {
        let id = model.split('[').next().unwrap_or(model);
        self.models.iter().find(|m| m.id == id)
    }

    /// The effort levels a start can pick for `model`: its own list, else the agent's.
    pub fn efforts_for(&self, model: &str) -> &[String] {
        match self.model(model) {
            Some(m) if !m.efforts.is_empty() => &m.efforts,
            _ => &self.efforts,
        }
    }
}

/// What the composer needs to know about one agent: whether it can start, and with what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub agent: Agent,
    pub status: ProviderStatus,
    pub catalog: AgentCatalog,
}
