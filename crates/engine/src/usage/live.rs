//! Live subscription usage, read from each provider's own usage endpoint with the token its CLI
//! already stores on this machine. Copied from src-tauri/src/devtools/live_usage.rs.
//!
//! This is the same path Ledge and other desktop clients use. The token is read from the CLI's
//! credentials file at request time and sent straight back to that provider, only ever to a usage
//! endpoint. Nothing is stored, nothing is forwarded anywhere else, and no inference call is made
//! (CLAUDE.md rule 1).
//!
//! Two things matter for not getting throttled:
//!   * Claude's oauth endpoints rate limit per token, and the bucket is SHARED with Claude Code
//!     itself. 180s is the safe cadence; polling faster burns the bucket for the CLI too. The
//!     floor is enforced here, so no caller can poll faster by mistake.
//!   * A 429 or 5xx means back off, not retry on the next tick. Both pollers hold a cooldown.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hyprspace_proto::usage::{LiveBar, LiveExtra, LiveProblem, LiveUsage};
use serde_json::Value;

use crate::util::{home_dir, read_json, title_case};

const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/codex/usage";

// Identifies the app while keeping the client string the endpoints expect. Codex's endpoint sits
// behind Cloudflare, which rejects an unknown agent outright.
const CLAUDE_UA: &str = "claude-code/2.0.0 (external, hyprspace)";
const CODEX_UA: &str = "codex_cli_rs/0.150.1";

/// The fastest Claude may be asked. A hard rule: the bucket is shared with Claude Code.
pub const CLAUDE_FLOOR_MS: i64 = 180_000;
pub const CODEX_FLOOR_MS: i64 = 60_000;

const MAX_BACKOFF_MS: i64 = 15 * 60_000;
/// A rejected token stays rejected until the CLI refreshes the file, which is not a per-tick event.
/// Without this a signed-out account would knock on the endpoint every three minutes forever.
const AUTH_COOLDOWN_MS: i64 = 10 * 60_000;
const HOUR_MS: i64 = 3_600_000;

/// Per-provider poll state: the last good reading, and how long to stay off the endpoint.
struct Poll {
    /// The last good reading.
    last: Option<LiveUsage>,
    /// What the last request produced, good or bad, and when it was sent.
    latest: Option<LiveUsage>,
    tried_at: i64,
    cool_until: i64,
    backoff: i64,
    plan: Option<String>,
    plan_tries: u8,
    /// Why we're cooling off, so every tick inside the window repeats the real reason rather than
    /// inventing a generic one.
    cool_reason: Option<(LiveProblem, String)>,
}

impl Poll {
    const fn new() -> Self {
        Self {
            last: None,
            latest: None,
            tried_at: i64::MIN,
            cool_until: 0,
            backoff: 0,
            plan: None,
            plan_tries: 0,
            cool_reason: None,
        }
    }

    /// An answer that needs no request: still cooling off, or asked again inside the floor.
    fn cached(&self, now: i64, floor: i64) -> Option<LiveUsage> {
        if let Some(u) = self.cooling(now) {
            return Some(u);
        }
        if now.saturating_sub(self.tried_at) < floor {
            return self.latest.clone();
        }
        None
    }

    /// Still cooling off: hand back the last good numbers rather than knocking again.
    fn cooling(&self, now: i64) -> Option<LiveUsage> {
        let left = self.cool_until - now;
        if left <= 0 {
            return None;
        }
        let (problem, why) = self
            .cool_reason
            .clone()
            .unwrap_or((LiveProblem::Rate, "Rate limited".into()));
        let note = format!("{why}. Retrying in {}", soon(left));
        // an auth problem has no numbers worth showing; a rate limit does, so keep the last good ones
        if problem == LiveProblem::Auth {
            return Some(self.bare(now, problem, note));
        }
        Some(match self.last.clone() {
            Some(mut last) => {
                last.note = Some(note);
                last
            }
            None => self.bare(now, problem, note),
        })
    }

    fn bare(&self, now: i64, problem: LiveProblem, note: String) -> LiveUsage {
        LiveUsage {
            at: now,
            problem: Some(problem),
            note: Some(note),
            plan: self.plan.clone(),
            ..Default::default()
        }
    }

    fn ok(&mut self, u: LiveUsage) -> LiveUsage {
        self.backoff = 0;
        self.cool_reason = None;
        self.last = Some(u.clone());
        self.latest = Some(u.clone());
        u
    }

    /// Back off on 429 and 5xx, cool down on a rejected token, and keep showing the last good
    /// numbers with a note.
    fn failed(&mut self, err: HttpErr, sign_in: &str, now: i64) -> LiveUsage {
        let u = if err.status == 429 || err.status >= 500 {
            self.backoff = if err.retry_after_ms > 0 {
                err.retry_after_ms
            } else {
                (self.backoff * 2).clamp(60_000, MAX_BACKOFF_MS)
            };
            self.cool_until = now + self.backoff;
            let why = if err.status == 429 {
                "Rate limited"
            } else {
                "Server error"
            };
            self.cool_reason = Some((LiveProblem::Rate, why.into()));
            let note = format!("{why}. Retrying in {}", soon(self.backoff));
            match self.last.clone() {
                Some(mut last) => {
                    last.note = Some(note);
                    last
                }
                None => self.bare(now, LiveProblem::Rate, note),
            }
        } else {
            let auth = err.status == 401 || err.status == 403;
            if auth {
                self.cool_until = now + AUTH_COOLDOWN_MS;
                self.cool_reason = Some((LiveProblem::Auth, sign_in.to_string()));
            }
            LiveUsage {
                at: now,
                problem: Some(if auth {
                    LiveProblem::Auth
                } else {
                    LiveProblem::Error
                }),
                note: Some(if auth {
                    sign_in.to_string()
                } else {
                    err.message
                }),
                bars: self.last.clone().map(|l| l.bars).unwrap_or_default(),
                plan: self.plan.clone(),
                ..Default::default()
            }
        };
        self.latest = Some(u.clone());
        u
    }
}

static CLAUDE: Mutex<Poll> = Mutex::new(Poll::new());
static CODEX: Mutex<Poll> = Mutex::new(Poll::new());

// the state is plain data, so a poisoned lock is still usable
fn lock(p: &'static Mutex<Poll>) -> MutexGuard<'static, Poll> {
    p.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// "default_claude_max_20x" -> "Max 20x". The tier is an internal id, not a label.
fn plan_label(tier: &str) -> String {
    let t = tier
        .trim_start_matches("default_")
        .trim_start_matches("claude_")
        .replace('_', " ");
    t.split_whitespace()
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
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
/// fraction" rule here: it would read a real 0.5% as 50%.
fn pct(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0).clamp(0.0, 100.0)
}

/// unix ms from unix seconds, unix ms, or an ISO string.
fn reset_ms(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n > 100_000_000_000 { n } else { n * 1000 });
    }
    // the endpoints return RFC 3339; parse the parts we need without pulling in chrono
    v.as_str().and_then(iso_ms)
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
    let off_min = match b
        .iter()
        .rposition(|&c| c == b'+' || c == b'-')
        .filter(|&i| i > 10)
    {
        Some(i) => {
            let sign = if b[i] == b'-' { -1 } else { 1 };
            let oh = s
                .get(i + 1..i + 3)
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            let om = s
                .get(i + 4..i + 6)
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
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

/// A GET that separates "the endpoint pushed back" from "something else went wrong", because only
/// the first should start a cooldown.
struct HttpErr {
    status: u16,
    retry_after_ms: i64,
    message: String,
}

impl HttpErr {
    fn other(e: impl ToString) -> Self {
        Self {
            status: 0,
            retry_after_ms: 0,
            message: e.to_string(),
        }
    }
}

async fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<Value, HttpErr> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(HttpErr::other)?;
    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = req.send().await.map_err(HttpErr::other)?;
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
            message: format!(
                "{} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            ),
        });
    }
    res.json::<Value>().await.map_err(HttpErr::other)
}

fn missing(msg: &str) -> LiveUsage {
    LiveUsage {
        at: now_ms(),
        problem: Some(LiveProblem::Missing),
        note: Some(msg.to_string()),
        ..Default::default()
    }
}

// ---- Claude ----

/// Claude Code refreshes this file in place, so re-reading each poll is cheaper and safer than
/// running our own token refresh.
fn claude_token() -> Option<String> {
    let v = read_json(&home_dir().join(".claude").join(".credentials.json"))?;
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
                name.map(|n| format!("{n} this week"))
                    .unwrap_or_else(|| "Scoped weekly".into()),
            )
        }
    }
}

fn claude_shape(d: &Value, plan: Option<String>, now: i64) -> LiveUsage {
    // `limits` carries every window with its own severity and matches the five_hour / seven_day
    // numbers exactly, so it is the whole source. The older top-level fields are the fallback for
    // an account whose response doesn't include the array.
    let limits = d["limits"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut bars: Vec<LiveBar> = limits
        .iter()
        .map(|l| {
            let (id, label) = claude_limit_id(l);
            let window_ms = Some(if id == "session" {
                5 * HOUR_MS
            } else {
                168 * HOUR_MS
            });
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
    let extra = x["is_enabled"]
        .as_bool()
        .unwrap_or(false)
        .then(|| LiveExtra {
            percent: pct(&x["utilization"]),
            used: x["used_credits"].as_f64().unwrap_or(0.0),
            limit: x["monthly_limit"].as_f64().unwrap_or(0.0) / 100.0,
            currency: x["currency"].as_str().map(String::from),
        });

    LiveUsage {
        ok: true,
        at: now,
        plan,
        bars,
        active,
        extra,
        problem: None,
        note: None,
    }
}

/// The account's Claude limits, read from Anthropic's own usage endpoint with the token the CLI
/// already holds. Asked again within 180s, it answers from the last reading.
pub async fn claude() -> LiveUsage {
    let now = now_ms();
    if let Some(u) = lock(&CLAUDE).cached(now, CLAUDE_FLOOR_MS) {
        return u;
    }
    let Some(tok) = claude_token() else {
        return missing("Run claude in a terminal to sign in");
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
        let mut p = lock(&CLAUDE);
        p.tried_at = now;
        p.plan.is_none() && p.plan_tries < 3
    };
    if need_plan {
        let got = get_json(CLAUDE_PROFILE_URL, &headers).await.ok();
        let mut p = lock(&CLAUDE);
        p.plan_tries += 1;
        if let Some(v) = got {
            p.plan = v["organization"]["rate_limit_tier"]
                .as_str()
                .map(plan_label);
        }
    }

    let res = get_json(CLAUDE_USAGE_URL, &headers).await;
    let mut p = lock(&CLAUDE);
    match res {
        Ok(v) => {
            let u = claude_shape(&v, p.plan.clone(), now_ms());
            p.ok(u)
        }
        Err(e) => p.failed(e, "Run claude in a terminal to sign in again", now_ms()),
    }
}

// ---- Codex ----

fn codex_auth() -> Option<(String, String)> {
    let v = read_json(&home_dir().join(".codex").join("auth.json"))?;
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
    match (h as f64 / 24.0).round() as i64 {
        1 => "Daily".into(),
        7 => "This week".into(),
        28..=31 => "This month".into(),
        d => format!("{d} days"),
    }
}

fn codex_shape(d: &Value, now: i64) -> LiveUsage {
    let rl = &d["rate_limit"];
    let mut bars = vec![];
    for (id, key) in [
        ("primary", "primary_window"),
        ("secondary", "secondary_window"),
    ] {
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
            window_ms: (secs > 0).then_some(secs * 1000),
            severity: None,
        });
    }
    LiveUsage {
        ok: true,
        at: now,
        plan: d["plan_type"]
            .as_str()
            .map(|p| format!("ChatGPT {}", title_case(p))),
        active: bars.first().map(|b| b.id.clone()),
        bars,
        extra: None,
        problem: None,
        note: None,
    }
}

/// The account's Codex limits, read from the same endpoint the Codex CLI uses.
pub async fn codex() -> LiveUsage {
    let now = now_ms();
    if let Some(u) = lock(&CODEX).cached(now, CODEX_FLOOR_MS) {
        return u;
    }
    let Some((tok, acct)) = codex_auth() else {
        return missing("Codex is not signed in on this machine");
    };
    let auth = format!("Bearer {tok}");
    let headers = [
        ("authorization", auth.as_str()),
        ("chatgpt-account-id", acct.as_str()),
        ("originator", "codex_cli_rs"),
        ("accept", "application/json"),
        ("user-agent", CODEX_UA),
    ];

    lock(&CODEX).tried_at = now;
    let res = get_json(CODEX_USAGE_URL, &headers).await;
    let mut p = lock(&CODEX);
    match res {
        Ok(v) => {
            let u = codex_shape(&v, now_ms());
            p.plan = u.plan.clone();
            p.ok(u)
        }
        Err(e) => p.failed(e, "Run codex to sign in again", now_ms()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn err(status: u16, retry_after_ms: i64) -> HttpErr {
        HttpErr {
            status,
            retry_after_ms,
            message: format!("{status}"),
        }
    }

    fn good(p: &mut Poll, now: i64) {
        p.tried_at = now;
        p.ok(LiveUsage {
            ok: true,
            at: now,
            bars: vec![LiveBar {
                id: "session".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
    }

    #[test]
    fn claude_is_never_asked_twice_inside_180s() {
        assert_eq!(CLAUDE_FLOOR_MS, 180_000);
        let mut p = Poll::new();
        assert!(p.cached(0, CLAUDE_FLOOR_MS).is_none());
        good(&mut p, 1_000);
        assert!(p.cached(1_000 + 179_999, CLAUDE_FLOOR_MS).unwrap().ok);
        assert!(p.cached(1_000 + 180_000, CLAUDE_FLOOR_MS).is_none());
    }

    #[test]
    fn a_429_backs_off_and_doubles() {
        let mut p = Poll::new();
        good(&mut p, 0);
        let u = p.failed(err(429, 0), "", 0);
        assert_eq!(u.note.as_deref(), Some("Rate limited. Retrying in 1m"));
        assert_eq!(u.bars.len(), 1, "the last good numbers stay up");
        assert!(p.cached(59_999, 0).is_some());
        assert!(p.cached(60_000, 0).is_none());
        p.failed(err(503, 0), "", 60_000);
        assert_eq!(p.backoff, 120_000);
        let cooling = p.cooling(60_000 + 90_000).unwrap();
        assert_eq!(
            cooling.note.as_deref(),
            Some("Server error. Retrying in 30s")
        );
    }

    #[test]
    fn retry_after_wins_and_backoff_caps() {
        let mut p = Poll::new();
        p.failed(err(429, 5_000), "", 0);
        assert_eq!(p.backoff, 5_000);
        for _ in 0..10 {
            p.failed(err(500, 0), "", 0);
        }
        assert_eq!(p.backoff, MAX_BACKOFF_MS);
    }

    #[test]
    fn a_rejected_token_cools_down_without_numbers() {
        let mut p = Poll::new();
        good(&mut p, 0);
        let u = p.failed(err(401, 0), "Sign in again", 0);
        assert_eq!(u.problem, Some(LiveProblem::Auth));
        let cooling = p.cooling(AUTH_COOLDOWN_MS - 1).unwrap();
        assert!(cooling.bars.is_empty());
        assert_eq!(
            cooling.note.as_deref(),
            Some("Sign in again. Retrying in 1s")
        );
        assert!(p.cooling(AUTH_COOLDOWN_MS).is_none());
        // a plain network error starts no cooldown
        let mut q = Poll::new();
        let u = q.failed(err(0, 0), "", 0);
        assert_eq!(u.problem, Some(LiveProblem::Error));
        assert!(q.cooling(1).is_none());
    }

    #[test]
    fn parses_times() {
        assert_eq!(iso_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_ms("2026-10-02T12:00:00Z"), Some(1_790_942_400_000));
        assert_eq!(
            iso_ms("2026-10-02T12:00:00+05:30"),
            Some(1_790_942_400_000 - 330 * 60_000)
        );
        assert_eq!(iso_ms("soon"), None);
        assert_eq!(reset_ms(&json!(1_700_000_000)), Some(1_700_000_000_000));
        assert_eq!(
            reset_ms(&json!(1_700_000_000_000i64)),
            Some(1_700_000_000_000)
        );
    }

    #[test]
    fn shapes_claude_limits_and_the_fallback() {
        let d = json!({
            "limits": [
                { "kind": "session", "percent": 66.0, "severity": "warning", "is_active": true },
                { "kind": "weekly_all", "percent": 0.5 },
                { "kind": "weekly_scoped", "scope": { "model": { "display_name": "Opus" } }, "percent": 120 },
            ],
            "extra_usage": { "is_enabled": true, "utilization": 10, "used_credits": 2.5, "monthly_limit": 5000 },
        });
        let u = claude_shape(&d, Some("Max 20x".into()), 7);
        let bars: Vec<_> = u
            .bars
            .iter()
            .map(|b| (b.id.as_str(), b.label.as_str(), b.percent))
            .collect();
        assert_eq!(
            bars,
            [
                ("session", "Session", 66.0),
                ("weekly", "All models", 0.5),
                ("scoped:Opus", "Opus this week", 100.0),
            ]
        );
        assert_eq!(u.active.as_deref(), Some("session"));
        assert_eq!(u.extra.unwrap().limit, 50.0);

        let old = json!({ "five_hour": { "utilization": 12.0 }, "seven_day": null });
        let u = claude_shape(&old, None, 0);
        assert_eq!(u.bars.len(), 1);
        assert_eq!(u.bars[0].window_ms, Some(5 * HOUR_MS));
    }

    #[test]
    fn shapes_codex_windows_by_length() {
        let d = json!({
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": { "used_percent": 5, "limit_window_seconds": 18000, "reset_at": 1 },
                "secondary_window": { "used_percent": 1, "limit_window_seconds": 604800 },
            },
        });
        let u = codex_shape(&d, 0);
        assert_eq!(u.plan.as_deref(), Some("ChatGPT Plus"));
        assert_eq!(u.bars[0].label, "5 hours");
        assert_eq!(u.bars[1].label, "This week");
        assert_eq!(codex_window_label(30 * 86_400), "This month");
        assert_eq!(codex_window_label(0), "Limit");
        assert_eq!(plan_label("default_claude_max_20x"), "Max 20x");
    }
}
