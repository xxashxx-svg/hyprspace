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

/// An agent CLI that runs as a structured session. The others (Gemini, OpenCode, Grok) run as
/// terminal sessions only for now (docs/REWRITE.md, open questions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude",
            Agent::Codex => "Codex",
        }
    }
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
    /// The effort levels a start can pick for `model`: its own list, else the agent's.
    pub fn efforts_for(&self, model: &str) -> &[String] {
        match self.models.iter().find(|m| m.id == model) {
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
