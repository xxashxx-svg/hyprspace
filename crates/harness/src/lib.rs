//! Drives agent CLIs over their machine protocols. Inference always runs through the user's own
//! binary (CLAUDE.md rule 1): no SDK, no API key, no token.
//!
//! Today this is the spike's one-run Claude adapter. Phase 3 grows it into the `Harness` trait
//! with Claude and Codex adapters and fake-CLI tests (docs/REWRITE.md).

pub mod claude;

/// Env a parent Claude Code session leaves behind. A child claude that inherits these thinks it
/// is nested inside that session: it turns transcript saving off (so nothing can be resumed) and
/// takes that session's id and messaging token. Only these: CLAUDE_CODE_* settings a user sets on
/// purpose (Bedrock, Git Bash path, output limits) are left alone.
pub const SESSION_ENV: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SSE_PORT",
];
