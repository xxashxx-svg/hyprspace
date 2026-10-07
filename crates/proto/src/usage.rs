// Usage records. `ProviderUsage` is read from files the CLIs write locally; `LiveUsage` is the
// account's real limits from the provider's usage endpoint (CLAUDE.md rule 1 covers how).

use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::wire::SessionId;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub used_percent: f64,
    pub window_minutes: u64,
    /// unix seconds
    pub resets_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Read from and written to the prompt cache, together.
    pub cache_tokens: u64,
    /// The written part of `cache_tokens`.
    #[serde(default)]
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
}

/// Tokens one model spent on one day, cache included, as Claude's own /stats counts them. A day
/// known only from a total (Claude's stats file, before the transcripts' window) has no split.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDay {
    /// The local day, `YYYY-MM-DD`.
    pub date: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total: u64,
}

/// Sessions started on one local day.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    pub date: String,
    pub count: u64,
}

/// One provider's usage as its local files describe it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub id: String,
    pub label: String,
    pub signed_in: bool,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub tier: Option<String>,
    pub sessions: u64,
    pub messages: u64,
    pub tool_calls: u64,
    pub active_days: u64,
    /// Token counts cover recent files only; `tokens_window` says which.
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub total_tokens: u64,
    pub tokens_window: Option<String>,
    /// Rolling-window limits, which Codex records in its rollouts.
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    /// When the file the windows came from was last written. Codex only records them during a
    /// session, so without this a three-week-old number looks identical to a live one.
    pub updated_at: i64,
    pub models: Vec<ModelUsage>,
    /// Tokens per model per day, as far back as the files go, for Activity.
    #[serde(default)]
    pub daily_models: Vec<ModelDay>,
    /// Sessions per day, as far back as the files go.
    #[serde(default)]
    pub day_sessions: Vec<DayCount>,
    pub note: Option<String>,
}

/// One limit window as the meter draws it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBar {
    pub id: String,
    pub label: String,
    /// 0-100
    pub percent: f64,
    /// unix ms
    pub resets_at: Option<i64>,
    /// How long the window is, so the meter can judge pace.
    pub window_ms: Option<i64>,
    /// The provider's own call: "normal" | "warning" | "critical". Better than anything we can
    /// infer, so the meter colours by this when it's here.
    pub severity: Option<String>,
}

/// Extra usage bought on top of the plan, when the account has it enabled.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveExtra {
    pub percent: f64,
    pub used: f64,
    pub limit: f64,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveProblem {
    /// The token was rejected; the user has to sign in through the CLI again.
    Auth,
    /// No credentials on this machine.
    Missing,
    /// 429 or 5xx: backing off.
    Rate,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveUsage {
    pub ok: bool,
    /// unix ms of this reading
    pub at: i64,
    pub plan: Option<String>,
    pub bars: Vec<LiveBar>,
    /// The window currently doing the limiting, when the provider says.
    pub active: Option<String>,
    pub extra: Option<LiveExtra>,
    /// Absent when ok.
    pub problem: Option<LiveProblem>,
    /// A short line for the panel: why the numbers are old, or how to fix sign-in.
    pub note: Option<String>,
}

/// What Claude's status line reported in one terminal session: the account's windows (keyed by
/// Claude's own names, `five_hour`, `seven_day`...) and the model in use. Claude pushes it every
/// turn, so it is the fallback when the usage endpoint can't answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    /// The label people read, "Opus 5.5".
    pub model: Option<String>,
    pub model_id: Option<String>,
    pub windows: Vec<LiveBar>,
}

/// Usage requests (docs/CONTEXT.md, Usage). How often the live endpoints may really be asked is
/// the engine's rule, not the caller's: a request inside the floor answers from the last reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum UsageCommand {
    /// The account's live limits. Answered with `UsageEvent::Live`.
    Live { agent: Agent },
    /// What `provider`'s files on this machine say. Answered with `UsageEvent::Local`.
    Local { provider: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum UsageEvent {
    // boxed: these are big next to every other event
    Live {
        agent: Agent,
        usage: Box<LiveUsage>,
    },
    /// None when the provider is unknown.
    Local {
        provider: String,
        usage: Option<Box<ProviderUsage>>,
    },
    /// A terminal session's Claude drew its status line.
    StatusLine {
        id: SessionId,
        report: StatusReport,
    },
}
