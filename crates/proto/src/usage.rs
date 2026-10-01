// Usage records. `ProviderUsage` is read from files the CLIs write locally; `LiveUsage` is the
// account's real limits from the provider's usage endpoint (CLAUDE.md rule 1 covers how).

use serde::{Deserialize, Serialize};

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
pub struct UsageDay {
    pub date: String,
    pub value: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub total_tokens: u64,
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
    pub daily: Vec<UsageDay>,
    /// "tokens" | "msgs" | "sessions"
    pub daily_unit: Option<String>,
    pub models: Vec<ModelUsage>,
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
