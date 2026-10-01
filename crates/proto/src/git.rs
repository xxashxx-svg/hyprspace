// Git records the dock and the composer show.

use serde::{Deserialize, Serialize};

/// One changed file in a working tree, with line counts where git has them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// Raw porcelain XY code: X is staged, Y unstaged (`M `, ` M`, `??`, `R `...).
    pub status: String,
    pub added: u32,
    pub removed: u32,
}

/// The current branch and how far it is from its upstream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub upstream: bool,
    pub is_repo: bool,
}

/// Suggested defaults for a new pull request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDefaults {
    pub head: String,
    pub base: String,
    pub title: String,
    pub body: String,
    /// Choices for the base picker.
    pub branches: Vec<String>,
    /// Whether the current branch has an upstream.
    pub pushed: bool,
    /// head == base, so no pull request is possible.
    pub on_default: bool,
}

/// Everything the "initialize repository" flow asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitRepo {
    pub name: String,
    pub branch: String,
    /// File contents; empty means don't add one.
    pub gitignore: String,
    pub readme: bool,
    pub commit: bool,
    pub commit_msg: String,
    pub github: bool,
    pub private: bool,
    pub description: String,
}

/// What a file action in the dock does. An empty path means every file (discard refuses that).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileOp {
    Stage,
    Unstage,
    Discard,
}
