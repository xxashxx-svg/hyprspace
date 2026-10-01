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
