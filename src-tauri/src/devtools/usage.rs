#![allow(unused_imports, dead_code)]
// Per-provider usage, read ENTIRELY from local files the CLIs already write (transcripts, session
// rollouts, stat caches, credential display fields). We never call a provider API or touch an auth
// token for auth — this is display-only aggregation, so it stays within subscription compliance.
use super::*; // home_dir, read_json
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use base64::Engine;

const RECENT_DAYS: u64 = 30; // token scans only look at recent files, so a settings tab stays snappy
const MAX_FILES: usize = 160; // hard cap on files scanned per provider
const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024; // skip pathologically huge transcripts

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    used_percent: f64,
    window_minutes: u64,
    resets_at: i64, // unix seconds
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageDay {
    date: String,
    value: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_tokens: u64,
    total_tokens: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    id: String,
    label: String,
    signed_in: bool,
    account: Option<String>,
    plan: Option<String>,
    tier: Option<String>,
    // activity
    sessions: u64,
    messages: u64,
    tool_calls: u64,
    active_days: u64,
    // tokens (bounded to recent files)
    input_tokens: u64,
    output_tokens: u64,
    cache_tokens: u64,
    total_tokens: u64,
    tokens_window: Option<String>,
    // rolling-window limits (Codex exposes these)
    primary: Option<UsageWindow>,
    secondary: Option<UsageWindow>,
    // small recent-activity sparkline
    daily: Vec<UsageDay>,
    daily_unit: Option<String>, // "tokens" | "msgs" | "sessions"
    // lifetime per-model split (claude keeps this in its stats cache)
    models: Vec<ModelUsage>,
    note: Option<String>,
}

#[tauri::command]
pub async fn provider_usage() -> Vec<ProviderUsage> {
    tauri::async_runtime::spawn_blocking(|| {
        vec![claude_usage(), codex_usage(), gemini_usage(), opencode_usage(), grok_usage()]
    })
    .await
    .unwrap_or_default()
}

// one provider at a time, so the panel can render cards as each scan finishes
// (claude's transcript scan dwarfs the others — no reason to make codex wait on it)
#[tauri::command]
pub async fn provider_usage_one(id: String) -> Option<ProviderUsage> {
    tauri::async_runtime::spawn_blocking(move || match id.as_str() {
        "claude" => Some(claude_usage()),
        "codex" => Some(codex_usage()),
        "gemini" => Some(gemini_usage()),
        "opencode" => Some(opencode_usage()),
        "grok" => Some(grok_usage()),
        _ => None,
    })
    .await
    .unwrap_or(None)
}

// ---- helpers ----
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// display-only claims out of a JWT — never trusted, never used for auth
fn decode_jwt(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn mtime_secs(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// collect *.jsonl under `root` (recursively), newest first, filtered to the recent window + capped
fn recent_jsonl(root: &Path) -> Vec<PathBuf> {
    let cutoff = now_secs().saturating_sub(RECENT_DAYS * 86_400);
    let mut files: Vec<(u64, PathBuf)> = vec![];
    collect_jsonl(root, &mut files, 0);
    files.retain(|(m, _)| *m >= cutoff);
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.truncate(MAX_FILES);
    files.into_iter().map(|(_, p)| p).collect()
}

fn collect_jsonl(dir: &Path, out: &mut Vec<(u64, PathBuf)>, depth: usize) {
    if depth > 6 || out.len() > 8000 {
        return; // guard against a pathological tree
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_jsonl(&p, out, depth + 1);
        } else if p.extension().map(|x| x == "jsonl").unwrap_or(false) {
            out.push((mtime_secs(&p), p));
        }
    }
}

fn window_from(w: &Value) -> Option<UsageWindow> {
    if !w.is_object() {
        return None;
    }
    Some(UsageWindow {
        used_percent: w["used_percent"].as_f64().unwrap_or(0.0),
        window_minutes: w["window_minutes"].as_u64().unwrap_or(0),
        resets_at: w["resets_at"].as_i64().unwrap_or(0),
    })
}

// ---- Claude ----
fn claude_usage() -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "claude".into(),
        label: "Claude Code".into(),
        ..Default::default()
    };
    let home = home_dir();
    let cdir = home.join(".claude");

    if let Some(v) = read_json(home.join(".claude.json")) {
        u.account = v["oauthAccount"]["emailAddress"].as_str().map(String::from);
    }
    if let Some(v) = read_json(cdir.join(".credentials.json")) {
        let o = &v["claudeAiOauth"];
        if let Some(sub) = o["subscriptionType"].as_str() {
            u.plan = Some(format!("Claude {}", title_case(sub)));
            u.signed_in = true;
        }
        if let Some(t) = o["rateLimitTier"].as_str() {
            u.tier = Some(title_case(&t.replace('_', " ")));
        }
    }
    u.signed_in = u.signed_in || u.account.is_some();

    // Claude keeps its own daily activity roll-up — cheap + accurate for messages/sessions/tools
    if let Some(v) = read_json(cdir.join("stats-cache.json")) {
        if let Some(days) = v["dailyActivity"].as_array() {
            u.active_days = days.len() as u64;
            for d in days {
                u.messages += d["messageCount"].as_u64().unwrap_or(0);
                u.sessions += d["sessionCount"].as_u64().unwrap_or(0);
                u.tool_calls += d["toolCallCount"].as_u64().unwrap_or(0);
            }
            let n = days.len();
            for d in days.iter().skip(n.saturating_sub(30)) {
                u.daily.push(UsageDay {
                    date: d["date"].as_str().unwrap_or("").to_string(),
                    value: d["messageCount"].as_u64().unwrap_or(0),
                });
            }
            if !u.daily.is_empty() {
                u.daily_unit = Some("msgs".into());
            }
        }
        // the cache also carries authoritative lifetime totals — prefer them over the daily sum
        if let Some(n) = v["totalSessions"].as_u64() {
            if n > 0 {
                u.sessions = n;
            }
        }
        if let Some(n) = v["totalMessages"].as_u64() {
            if n > 0 {
                u.messages = n;
            }
        }
        // tokens-per-day makes a better activity sparkline than message counts
        if let Some(days) = v["dailyModelTokens"].as_array() {
            let mut daily: Vec<UsageDay> = days
                .iter()
                .map(|d| UsageDay {
                    date: d["date"].as_str().unwrap_or("").to_string(),
                    value: d["tokensByModel"]
                        .as_object()
                        .map(|m| m.values().filter_map(|x| x.as_u64()).sum())
                        .unwrap_or(0),
                })
                .collect();
            if !daily.is_empty() {
                let n = daily.len();
                u.daily = daily.split_off(n.saturating_sub(30));
                u.daily_unit = Some("tokens".into());
            }
        }
        // lifetime split by model
        if let Some(mu) = v["modelUsage"].as_object() {
            for (name, m) in mu {
                let i = m["inputTokens"].as_u64().unwrap_or(0);
                let o = m["outputTokens"].as_u64().unwrap_or(0);
                let c = m["cacheReadInputTokens"].as_u64().unwrap_or(0)
                    + m["cacheCreationInputTokens"].as_u64().unwrap_or(0);
                if i + o + c == 0 {
                    continue;
                }
                u.models.push(ModelUsage {
                    model: name.clone(),
                    input_tokens: i,
                    output_tokens: o,
                    cache_tokens: c,
                    total_tokens: i + o + c,
                });
            }
            u.models.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
            u.models.truncate(8);
        }
    }

    // tokens aren't rolled up anywhere, so sum recent transcripts (bounded)
    let (i, o, c) = sum_claude_tokens(&cdir.join("projects"));
    u.input_tokens = i;
    u.output_tokens = o;
    u.cache_tokens = c;
    u.total_tokens = i + o + c;
    if u.total_tokens > 0 {
        u.tokens_window = Some(format!("last {RECENT_DAYS} days"));
    }
    u.note =
        Some("Weekly & session limits live on claude.ai → Settings → Usage (or /usage in a pane).".into());
    u
}

fn sum_claude_tokens(projects: &Path) -> (u64, u64, u64) {
    let (mut i, mut o, mut c) = (0u64, 0u64, 0u64);
    for f in recent_jsonl(projects) {
        if std::fs::metadata(&f).map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(false) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for line in text.lines() {
            if !line.contains("\"output_tokens\"") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let usage = if v["message"]["usage"].is_object() {
                &v["message"]["usage"]
            } else {
                &v["usage"]
            };
            if usage.is_object() {
                i += usage["input_tokens"].as_u64().unwrap_or(0);
                o += usage["output_tokens"].as_u64().unwrap_or(0);
                c += usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)
                    + usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
            }
        }
    }
    (i, o, c)
}

// ---- Codex ----
fn codex_usage() -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "codex".into(),
        label: "Codex".into(),
        ..Default::default()
    };
    let cdir = home_dir().join(".codex");

    if let Some(v) = read_json(cdir.join("auth.json")) {
        u.signed_in = true;
        if let Some(tok) = v["tokens"]["id_token"].as_str() {
            if let Some(p) = decode_jwt(tok) {
                u.account = p["email"].as_str().map(String::from);
                if let Some(plan) = p["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str() {
                    u.plan = Some(format!("ChatGPT {}", title_case(plan)));
                }
            }
        }
        if u.plan.is_none() && v["OPENAI_API_KEY"].as_str().is_some() {
            u.plan = Some("API key".into());
        }
    }

    // session rollouts carry token_count events with the live rate-limit windows
    let files = recent_jsonl(&cdir.join("sessions"));
    u.sessions = files.len() as u64;
    let (mut i, mut o, mut c) = (0u64, 0u64, 0u64);
    let mut by_day: std::collections::BTreeMap<String, u64> = Default::default();
    for (idx, f) in files.iter().enumerate() {
        let Some(tc) = last_token_count(f) else {
            continue;
        };
        let ttu = &tc["info"]["total_token_usage"];
        i += ttu["input_tokens"].as_u64().unwrap_or(0);
        o += ttu["output_tokens"].as_u64().unwrap_or(0);
        c += ttu["cached_input_tokens"].as_u64().unwrap_or(0);
        // rollout filenames embed the session date: rollout-YYYY-MM-DD...
        if let Some(d) = f
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("rollout-"))
            .and_then(|n| n.get(..10))
        {
            *by_day.entry(d.to_string()).or_insert(0) +=
                ttu["total_tokens"].as_u64().unwrap_or(0);
        }
        if idx == 0 {
            let rl = &tc["rate_limits"];
            u.primary = window_from(&rl["primary"]);
            u.secondary = window_from(&rl["secondary"]);
            if u.plan.is_none() {
                if let Some(pt) = rl["plan_type"].as_str() {
                    u.plan = Some(format!("ChatGPT {}", title_case(pt)));
                }
            }
        }
    }
    u.input_tokens = i;
    u.output_tokens = o;
    u.cache_tokens = c;
    u.total_tokens = i + o + c;
    if u.total_tokens > 0 {
        u.tokens_window = Some("recent sessions".into());
    }
    if by_day.len() > 1 {
        u.daily = by_day
            .into_iter()
            .map(|(date, value)| UsageDay { date, value })
            .collect();
        u.daily_unit = Some("tokens".into());
    }
    u
}

// scan a rollout from the end for the last token_count event (has the current rate limits)
fn last_token_count(file: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(file).ok()?;
    for line in text.lines().rev() {
        if line.contains("\"token_count\"") && line.contains("\"rate_limits\"") {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                if v["payload"]["type"] == "token_count" {
                    return Some(v["payload"].clone());
                }
            }
        }
    }
    None
}

// ---- Gemini ----
fn gemini_usage() -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "gemini".into(),
        label: "Gemini".into(),
        ..Default::default()
    };
    let g = home_dir().join(".gemini");
    if let Some(v) = read_json(g.join("google_accounts.json")) {
        u.account = v["active"].as_str().map(String::from);
    }
    u.signed_in = g.join("oauth_creds.json").exists() || u.account.is_some();
    u.note = Some("Gemini CLI doesn't record token usage locally — only the signed-in account is shown.".into());
    u
}

// ---- OpenCode ----
fn opencode_usage() -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "opencode".into(),
        label: "OpenCode".into(),
        ..Default::default()
    };
    let auth = home_dir()
        .join(".local")
        .join("share")
        .join("opencode")
        .join("auth.json");
    if let Some(v) = read_json(auth) {
        if let Some(obj) = v.as_object() {
            u.signed_in = !obj.is_empty();
            let names: Vec<String> = obj.keys().map(|k| title_case(k)).collect();
            if !names.is_empty() {
                u.plan = Some(format!(
                    "{} provider{}",
                    names.len(),
                    if names.len() == 1 { "" } else { "s" }
                ));
                u.note = Some(format!("Model providers: {}", names.join(", ")));
            }
        }
    }
    if u.note.is_none() {
        u.note = Some("BYO-model — usage lives in OpenCode's own local database.".into());
    }
    u
}

// ---- Grok ----
fn grok_usage() -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "grok".into(),
        label: "Grok".into(),
        ..Default::default()
    };
    let g = home_dir().join(".grok");
    u.signed_in = g.exists() || std::env::var("XAI_API_KEY").is_ok();
    // a grok session = one folder holding a chat_history.jsonl (it also writes events/prompt
    // jsonls alongside, which would triple-count)
    u.sessions = recent_jsonl(&g.join("sessions"))
        .iter()
        .filter(|p| p.file_name().map(|n| n == "chat_history.jsonl").unwrap_or(false))
        .count() as u64;
    u.note = Some("Grok Build CLI — token usage & rate limits live in your xAI console.".into());
    u
}
