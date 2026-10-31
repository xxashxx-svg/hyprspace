// Updating the agent CLIs with their own updaters: how each one is installed, whether a newer
// version is out, and running the update on a click. The engine does the work
// (engine/src/agent_update.rs); this computer's agents only.

use serde::{Deserialize, Serialize};

use crate::Agent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum AgentUpdateCommand {
    /// Answered with one `Found` per agent. `latest` also asks npm for the newest version.
    Check { latest: bool },
    /// Runs the agent's own updater. Answered with `Running`, then `Done`.
    Run { agent: Agent },
    /// Installs `version` again, after an update that went wrong. Native Claude only.
    Revert { agent: Agent, version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum AgentUpdateEvent {
    Found { agent: Agent, install: Install },
    Running { agent: Agent },
    Done { agent: Agent, outcome: Outcome },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    /// None when the agent isn't installed.
    pub version: Option<String>,
    pub how: How,
    /// The command HyprSpace runs, or the one to run yourself when it can't.
    pub command: String,
    /// The newest version npm has, when it was asked and is newer than `version`.
    pub latest: Option<String>,
    /// `latest` breaks HyprSpace's threads, so it isn't offered.
    pub held: bool,
}

impl Install {
    pub fn runs(&self) -> bool {
        matches!(
            self.how,
            How::Native | How::Standalone | How::Npm | How::Homebrew
        )
    }

    pub fn behind(&self) -> bool {
        self.latest.is_some() && !self.held
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum How {
    /// Claude's own installer, which `claude update` updates.
    Native,
    /// Codex's own installer, which `codex update` updates.
    Standalone,
    Npm,
    Homebrew,
    /// Installed some way HyprSpace can't prove, so it shows the command instead of running it.
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome {
    Updated {
        from: String,
        to: String,
    },
    /// The updater ran and the version didn't move: already the newest on its channel.
    Current {
        version: String,
    },
    Reverted {
        to: String,
    },
    /// `why` is user-facing.
    Failed {
        why: String,
    },
}
