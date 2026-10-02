// Requests for Settings' Skills view: list, read, write and delete Claude skills and slash
// commands (docs/CONTEXT.md, Skill). Each names the folder whose project skills it is about.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{SkillItem, SkillKind, SkillScope};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum SkillCommand {
    /// The user's skills, plus `cwd`'s when it is not empty. Answered with `SkillEvent::List`.
    List { cwd: PathBuf },
    /// One skill's whole file. Answered with `SkillEvent::Read`.
    Read {
        cwd: PathBuf,
        scope: SkillScope,
        kind: SkillKind,
        name: String,
    },
    /// Writes the file, then deletes `replaces` (a rename, or a move to the other scope).
    /// Answered with `SkillEvent::Done`, then a fresh `List` when it worked.
    Write {
        cwd: PathBuf,
        scope: SkillScope,
        kind: SkillKind,
        name: String,
        content: String,
        replaces: Option<(SkillScope, String)>,
    },
    /// Answered like `Write`.
    Delete {
        cwd: PathBuf,
        scope: SkillScope,
        kind: SkillKind,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum SkillEvent {
    List {
        cwd: PathBuf,
        items: Vec<SkillItem>,
    },
    Read {
        scope: SkillScope,
        name: String,
        content: Result<String, String>,
    },
    /// A write or delete finished. `error` is user-facing.
    Done {
        error: Option<String>,
    },
}
