#![allow(unused_imports)]
//! Live subscription usage, read from each provider's own usage endpoint with the token its CLI
//! already stores on this machine.
//!
//! This is the same path Ledge and other desktop clients use. The token is read from the CLI's
//! credentials file at request time and sent straight back to that provider, only ever to a usage
//! endpoint. Nothing is stored, nothing is forwarded anywhere else, and no inference call is made.
//!
//! Two things matter for not getting throttled:
//!   * Claude's oauth endpoints rate limit per token, and the bucket is SHARED with Claude Code
//!     itself. 180s is the safe cadence; polling faster burns the bucket for the CLI too.
//!   * A 429 or 5xx means back off, not retry on the next tick. Both pollers hold a cooldown.
use super::*;
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/codex/usage";

// Identifies the app while keeping the client string the endpoints expect. Codex's endpoint sits
// behind Cloudflare, which rejects an unknown agent outright.
const CLAUDE_UA: &str = "claude-code/2.0.0 (external, hyprspace)";
const CODEX_UA: &str = "codex_cli_rs/0.150.1";

const MAX_BACKOFF_MS: i64 = 15 * 60_000;
/// A rejected token stays rejected until the CLI refreshes the file, which is not a per-tick event.
/// Without this a signed-out account would knock on the endpoint every three minutes forever.
const AUTH_COOLDOWN_MS: i64 = 10 * 60_000;
const HOUR_MS: i64 = 3_600_000;

/// One limit window as the meter draws it.
#[derive(Serialize, Clone, Default)]
pub struct LiveBar {
    pub id: String,
    pub label: String,
    /// 0-100
    pub percent: f64,
    /// unix ms
    #[serde(rename = "resetsAt")]
    pub resets_at: Option<i64>,
    /// how long the window is, so the meter can judge pace
    #[serde(rename = "windowMs")]
    pub window_ms: Option<i64>,
    /// the provider's own call: "normal" | "warning" | "critical". Better than anything we can
    /// infer, so the meter colours by this when it's here.
    pub severity: Option<String>,
}

/// Extra usage bought on top of the plan, when the account has it enabled.
#[derive(Serialize, Clone, Default)]
pub struct LiveExtra {
    pub percent: f64,
    pub used: f64,
    pub limit: f64,
    pub currency: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct LiveUsage {
    pub ok: bool,
    /// unix ms of this reading
    pub at: i64,
    pub plan: Option<String>,
    pub bars: Vec<LiveBar>,
    /// the window currently doing the limiting, when the provider says
    pub active: Option<String>,
    pub extra: Option<LiveExtra>,
    /// "auth" | "missing" | "rate" | "error" — absent when ok
    pub kind: Option<String>,
    /// short line for the panel: why the numbers are old, or how to fix sign-in
    pub note: Option<String>,
}

/// Per-provider poll state: the last good reading, and how long to stay off the endpoint.
#[derive(Default)]
struct Poll {
    last: Option<LiveUsage>,
    cool_until: i64,
    backoff: i64,
    plan: Option<String>,
    plan_tries: u8,
    /// why we're cooling off, so every tick inside the window repeats the real reason rather than
    /// inventing a generic one: ("auth", "Run claude in a terminal to sign in again")
    cool_reason: Option<(String, String)>,
}

static CLAUDE: Mutex<Option<Poll>> = Mutex::new(None);
static CODEX: Mutex<Option<Poll>> = Mutex::new(None);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// "pro" -> "Pro". Local copy: the one in providers.rs is private to that module.
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// "default_claude_max_20x" -> "Max 20x". The tier is an internal id, not a label.
fn plan_label(tier: &str) -> String {
    let t = tier
        .trim_start_matches("default_")
        .trim_start_matches("claude_")
        .replace('_', " ");
    t.split_whitespace().map(cap).collect::<Vec<_>>().join(" ")
}

fn soon(ms: i64) -> String {
    let s = (ms as f64 / 1000.0).ceil() as i64;
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m", (s as f64 / 60.0).ceil() as i64)
    }
}

/// Both usage endpoints report 0-100 already. Verified against live responses: claude sends
/// `utilization: 66.0` for 66%, codex `used_percent: 5` for 5%. Do NOT add a "looks like a
/// fraction" rule here — it would read a real 0.5% as 50%.
fn pct(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0).clamp(0.0, 100.0)
}

/// unix ms from unix seconds, unix ms, or an ISO string.
fn reset_ms(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n > 1e11 as i64 { n } else { n * 1000 });
    }
    if let Some(s) = v.as_str() {
        // the endpoints return RFC 3339; parse the parts we need without pulling in chrono
        return iso_ms(s);
    }
    None
}

/// Minimal RFC 3339 to unix ms. Returns None on anything unexpected rather than guessing.
fn iso_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, sec) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    // trailing offset: "Z", "+05:30" or "-08:00". Anything else is treated as UTC.
    let off_min = match b.iter().rposition(|&c| c == b'+' || c == b'-').filter(|&i| i > 10) {
        Some(i) => {
            let sign = if b[i] == b'-' { -1 } else { 1 };
            let oh = s.get(i + 1..i + 3).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            let om = s.get(i + 4..i + 6).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
            sign * (oh * 60 + om)
        }
        None => 0,
    };
    // days since the unix epoch, via the civil-from-days algorithm
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400) + h * 3600 + (mi - off_min) * 60 + sec) * 1000)
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// A GET that separates "the endpoint pushed back" from "something else went wrong", because only
/// the first should start a cooldown.
struct HttpErr {
    status: u16,
    retry_after_ms: i64,
    message: String,
}

async fn get_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<Value, HttpErr> {
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(|e| HttpErr {
        status: 0,
        retry_after_ms: 0,
        message: e.to_string(),
    })?;
    let status = res.status();
    if !status.is_success() {
        let retry_after_ms = res
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())
            .map(|s| s * 1000)
            .unwrap_or(0);
        return Err(HttpErr {
            status: status.as_u16(),
            retry_after_ms,
            message: format!("{} {}", status.as_u16(), status.canonical_reason().unwrap_or("")),
        });
    }
    res.json::<Value>().await.map_err(|e| HttpErr {
        status: 0,
        retry_after_ms: 0,
        message: e.to_string(),
    })
}

/// Shared failure handling: back off on 429/5xx, keep showing the last good numbers with a note.
fn on_error(p: &mut Poll, err: HttpErr, sign_in: &str) -> LiveUsage {
    if err.status == 429 || err.status >= 500 {
        p.backoff = if err.retry_after_ms > 0 {
            err.retry_after_ms
        } else {
            (p.backoff * 2).max(60_000).min(MAX_BACKOFF_MS)
        };
        p.cool_until = now_ms() + p.backoff;
        let why = if err.status == 429 { "Rate limited" } else { "Server error" };
        p.cool_reason = Some(("rate".into(), why.into()));
        let note = format!("{why}. Retrying in {}", soon(p.backoff));
        return match p.last.clone() {
            Some(mut last) => {
                last.note = Some(note);
                last
            }
            None => LiveUsage {
                at: now_ms(),
                kind: Some("rate".into()),
                note: Some(note),
                plan: p.plan.clone(),
                ..Default::default()
            },
        };
    }
    let auth = err.status == 401 || err.status == 403;
    if auth {
        p.cool_until = now_ms() + AUTH_COOLDOWN_MS;
        p.cool_reason = Some(("auth".into(), sign_in.to_string()));
    }
    LiveUsage {
        at: now_ms(),
        kind: Some(if auth { "auth".into() } else { "error".into() }),
        note: Some(if auth { sign_in.to_string() } else { err.message }),
        bars: p.last.clone().map(|l| l.bars).unwrap_or_default(),
        plan: p.plan.clone(),
        ..Default::default()
    }
}

/// Still cooling off: hand back the last good numbers rather than knocking again.
fn cooling(p: &Poll) -> Option<LiveUsage> {
    let left = p.cool_until - now_ms();
    if left <= 0 {
        return None;
    }
    let (kind, why) = p
        .cool_reason
        .clone()
        .unwrap_or_else(|| ("rate".into(), "Rate limited".into()));
    let note = format!("{why}. Retrying in {}", soon(left));
    // an auth problem has no numbers worth showing; a rate limit does, so keep the last good ones
    if kind == "auth" {
        return Some(LiveUsage {
            at: now_ms(),
            kind: Some(kind),
            note: Some(note),
            plan: p.plan.clone(),
            ..Default::default()
        });
    }
    Some(match p.last.clone() {
        Some(mut last) => {
            last.note = Some(note);
            last
        }
        None => LiveUsage {
            at: now_ms(),
            kind: Some(kind),
            note: Some(note),
            plan: p.plan.clone(),
            ..Default::default()
        },
    })
}

fn missing(msg: &str) -> LiveUsage {
    LiveUsage {
        at: now_ms(),
        kind: Some("missing".into()),
        note: Some(msg.to_string()),
        ..Default::default()
    }
}

// ---- Claude ----

/// Claude Code refreshes this file in place, so re-reading each poll is cheaper and safer than
/// running our own token refresh.
fn claude_token() -> Option<String> {
    let v = read_json(home_dir().join(".claude").join(".credentials.json"))?;
    v["claudeAiOauth"]["accessToken"].as_str().map(String::from)
}

/// The id and label for one entry of claude's `limits` array.
fn claude_limit_id(l: &Value) -> (String, String) {
    match l["kind"].as_str() {
        Some("session") => ("session".into(), "Session".into()),
        Some("weekly_all") => ("weekly".into(), "All models".into()),
        _ => {
            let name = l["scope"]["model"]["display_name"].as_str();
            (
                format!("scoped:{}", name.unwrap_or("weekly")),
                name.map(|n| format!("{n} this week")).unwrap_or_else(|| "Scoped weekly".into()),
            )
        }
    }
}

fn claude_shape(d: &Value, plan: Option<String>) -> LiveUsage {
    // `limits` carries every window with its own severity and matches the five_hour / seven_day
    // numbers exactly, so it is the whole source. The older top-level fields are the fallback for
    // an account whose response doesn't include the array.
    let empty = vec![];
    let limits = d["limits"].as_array().unwrap_or(&empty);
    let mut bars: Vec<LiveBar> = limits
        .iter()
        .map(|l| {
            let (id, label) = claude_limit_id(l);
            let window_ms = Some(if id == "session" { 5 * HOUR_MS } else { 168 * HOUR_MS });
            LiveBar {
                id,
                label,
                percent: pct(&l["percent"]),
                resets_at: reset_ms(&l["resets_at"]),
                window_ms,
                severity: l["severity"].as_str().map(String::from),
            }
        })
        .collect();
    if bars.is_empty() {
        for (key, id, label, win) in [
            ("five_hour", "session", "Session", 5 * HOUR_MS),
            ("seven_day", "weekly", "All models", 168 * HOUR_MS),
        ] {
            if let Some(w) = d.get(key).filter(|v| !v.is_null()) {
                bars.push(LiveBar {
                    id: id.into(),
                    label: label.into(),
                    percent: pct(&w["utilization"]),
                    resets_at: reset_ms(&w["resets_at"]),
                    window_ms: Some(win),
                    severity: None,
                });
            }
        }
    }

    let active = limits
        .iter()
        .find(|l| l["is_active"].as_bool().unwrap_or(false))
        .map(|l| claude_limit_id(l).0);

    let x = &d["extra_usage"];
    let extra = if x["is_enabled"].as_bool().unwrap_or(false) {
        Some(LiveExtra {
            percent: pct(&x["utilization"]),
            used: x["used_credits"].as_f64().unwrap_or(0.0),
            limit: x["monthly_limit"].as_f64().unwrap_or(0.0) / 100.0,
            currency: x["currency"].as_str().map(String::from),
        })
    } else {
        None
    };

    LiveUsage { ok: true, at: now_ms(), plan, bars, active, extra, kind: None, note: None }
}

/// The account's Claude limits, read from Anthropic's own usage endpoint with the token the CLI
/// already holds. Poll no faster than every 180s: the bucket is shared with Claude Code.
#[tauri::command]
pub async fn claude_live_usage() -> LiveUsage {
    {
        let mut g = CLAUDE.lock().unwrap();
        let p = g.get_or_insert_with(Poll::default);
        if let Some(u) = cooling(p) {
            return u;
        }
    }
    let Some(tok) = claude_token() else {
        return missing("Run claude in a terminal to sign in");
    };
    let Ok(client) = http() else {
        return missing("Could not start the http client");
    };
    let auth = format!("Bearer {tok}");
    let headers = [
        ("authorization", auth.as_str()),
        ("anthropic-beta", "oauth-2025-04-20"),
        ("accept", "application/json"),
        ("user-agent", CLAUDE_UA),
    ];

    // the plan name is cosmetic, so give up after a few misses rather than doubling the request rate
    let need_plan = {
        let mut g = CLAUDE.lock().unwrap();
        let p = g.get_or_insert_with(Poll::default);
        p.plan.is_none() && p.plan_tries < 3
    };
    if need_plan {
        let got = get_json(&client, CLAUDE_PROFILE_URL, &headers).await.ok();
        let mut g = CLAUDE.lock().unwrap();
        let p = g.get_or_insert_with(Poll::default);
        p.plan_tries += 1;
        if let Some(v) = got {
            p.plan = v["organization"]["rate_limit_tier"].as_str().map(plan_label);
        }
    }

    let res = get_json(&client, CLAUDE_USAGE_URL, &headers).await;
    let mut g = CLAUDE.lock().unwrap();
    let p = g.get_or_insert_with(Poll::default);
    match res {
        Ok(v) => {
            let u = claude_shape(&v, p.plan.clone());
            p.backoff = 0;
            p.cool_reason = None;
            p.last = Some(u.clone());
            u
        }
        Err(e) => on_error(p, e, "Run claude in a terminal to sign in again"),
    }
}

// ---- Codex ----

fn codex_auth() -> Option<(String, String)> {
    let v = read_json(home_dir().join(".codex").join("auth.json"))?;
    let t = v["tokens"]["access_token"].as_str()?.to_string();
    let acct = v["tokens"]["account_id"].as_str().unwrap_or("").to_string();
    Some((t, acct))
}

/// Plans differ: some return one 30-day window, others a 5h primary plus a weekly secondary. Name
/// the window from its own length rather than assuming which plan this is.
fn codex_window_label(seconds: i64) -> String {
    if seconds <= 0 {
        return "Limit".into();
    }
    let h = (seconds as f64 / 3600.0).round() as i64;
    if h <= 1 {
        return "Hourly".into();
    }
    if h < 24 {
        return format!("{h} hours");
    }
    let d = (h as f64 / 24.0).round() as i64;
    match d {
        1 => "Daily".into(),
        7 => "This week".into(),
        28..=31 => "This month".into(),
        _ => format!("{d} days"),
    }
}

fn codex_shape(d: &Value) -> LiveUsage {
    let rl = &d["rate_limit"];
    let mut bars = vec![];
    for (id, key) in [("primary", "primary_window"), ("secondary", "secondary_window")] {
        let w = &rl[key];
        if w.is_null() {
            continue;
        }
        let secs = w["limit_window_seconds"].as_i64().unwrap_or(0);
        bars.push(LiveBar {
            id: id.into(),
            label: codex_window_label(secs),
            percent: pct(&w["used_percent"]),
            resets_at: reset_ms(&w["reset_at"]),
            window_ms: if secs > 0 { Some(secs * 1000) } else { None },
            severity: None,
        });
    }
    let active = bars.first().map(|b| b.id.clone());
    LiveUsage {
        ok: true,
        at: now_ms(),
        plan: d["plan_type"].as_str().map(|p| format!("ChatGPT {}", cap(p))),
        bars,
        active,
        extra: None,
        kind: None,
        note: None,
    }
}

/// The account's Codex limits, read from the same endpoint the Codex CLI uses.
#[tauri::command]
pub async fn codex_live_usage() -> LiveUsage {
    {
        let mut g = CODEX.lock().unwrap();
        let p = g.get_or_insert_with(Poll::default);
        if let Some(u) = cooling(p) {
            return u;
        }
    }
    let Some((tok, acct)) = codex_auth() else {
        return missing("Codex is not signed in on this machine");
    };
    let Ok(client) = http() else {
        return missing("Could not start the http client");
    };
    let auth = format!("Bearer {tok}");
    let headers = [
        ("authorization", auth.as_str()),
        ("chatgpt-account-id", acct.as_str()),
        ("originator", "codex_cli_rs"),
        ("accept", "application/json"),
        ("user-agent", CODEX_UA),
    ];

    let res = get_json(&client, CODEX_USAGE_URL, &headers).await;
    let mut g = CODEX.lock().unwrap();
    let p = g.get_or_insert_with(Poll::default);
    match res {
        Ok(v) => {
            let u = codex_shape(&v);
            p.backoff = 0;
            p.cool_reason = None;
            p.plan = u.plan.clone();
            p.last = Some(u.clone());
            u
        }
        Err(e) => on_error(p, e, "Run codex to sign in again"),
    }
}
