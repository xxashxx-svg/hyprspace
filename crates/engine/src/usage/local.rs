// Per-provider usage, read ENTIRELY from local files the CLIs already write (transcripts, session
// rollouts, stat caches, credential display fields). No provider API is called and no token is
// used: this is display-only aggregation. Copied from src-tauri/src/devtools/usage.rs.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyprspace_proto::usage::{ModelDay, ModelUsage, ProviderUsage, UsageDay, UsageWindow};
use serde_json::Value;

use crate::util::{decode_jwt, home_dir, read_json, title_case};

const RECENT_DAYS: u64 = 30; // token scans only look at recent files, so a settings tab stays snappy
const MAX_FILES: usize = 160; // hard cap on files scanned per provider
const MAX_FILE_BYTES: u64 = 80 * 1024 * 1024; // skip pathologically huge transcripts

/// One provider at a time, so a panel can render each card as its scan finishes (claude's
/// transcript scan dwarfs the others). Blocks on disk reads.
pub fn provider_usage(id: &str) -> Option<ProviderUsage> {
    let home = home_dir();
    let now = now_secs();
    Some(match id {
        "claude" => claude_usage(&home, now),
        "codex" => codex_usage(&home, now),
        "opencode" => opencode_usage(&home),
        "grok" => grok_usage(&home, now, std::env::var("XAI_API_KEY").is_ok()),
        _ => return None,
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mtime_secs(p: &Path) -> u64 {
    crate::util::mtime_ms(p) / 1000
}

// *.jsonl under `root` (recursively), newest first, filtered to the recent window and capped
fn recent_jsonl(root: &Path, now: u64) -> Vec<PathBuf> {
    let cutoff = now.saturating_sub(RECENT_DAYS * 86_400);
    let mut files: Vec<(u64, PathBuf)> = vec![];
    collect_jsonl(root, &mut files, 0);
    files.retain(|(m, _)| *m >= cutoff);
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
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
        } else if p.extension().is_some_and(|x| x == "jsonl") {
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

fn u64_at(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or(0)
}

// ---- Claude ----

fn claude_usage(home: &Path, now: u64) -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "claude".into(),
        label: "Claude Code".into(),
        ..Default::default()
    };
    let cdir = home.join(".claude");

    if let Some(v) = read_json(&home.join(".claude.json")) {
        u.account = v["oauthAccount"]["emailAddress"].as_str().map(String::from);
    }
    if let Some(v) = read_json(&cdir.join(".credentials.json")) {
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

    if let Some(v) = read_json(&cdir.join("stats-cache.json")) {
        apply_stats_cache(&mut u, &v);
    }

    // tokens aren't rolled up anywhere, so sum recent transcripts (bounded)
    let (i, o, c, days) = sum_claude_tokens(&cdir.join("projects"), now);
    u.input_tokens = i;
    u.output_tokens = o;
    u.cache_tokens = c;
    u.total_tokens = i.saturating_add(o).saturating_add(c);
    if u.total_tokens > 0 {
        u.tokens_window = Some(format!("last {RECENT_DAYS} days"));
    }
    by_model_and_day(&mut u, days);
    u
}

/// Fills the per-model, per-day figures from `days`, keyed by (day, model) with tokens in and
/// out. A provider with no model split or daily figures of its own takes them from here.
fn by_model_and_day(u: &mut ProviderUsage, mut days: BTreeMap<(String, String), (u64, u64)>) {
    // a session that spent nothing has nothing to show
    days.retain(|_, (i, o)| *i + *o > 0);
    let mut models: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    let mut daily: BTreeMap<&str, u64> = BTreeMap::new();
    for ((date, model), (i, o)) in &days {
        let m = models.entry(model).or_default();
        m.0 = m.0.saturating_add(*i);
        m.1 = m.1.saturating_add(*o);
        let d = daily.entry(date).or_default();
        *d = d.saturating_add(i.saturating_add(*o));
    }
    if u.models.is_empty() {
        u.models = models
            .iter()
            .map(|(model, (i, o))| ModelUsage {
                model: model.to_string(),
                input_tokens: *i,
                output_tokens: *o,
                cache_tokens: 0,
                total_tokens: i.saturating_add(*o),
            })
            .collect();
        u.models.sort_by_key(|m| std::cmp::Reverse(m.total_tokens));
        u.models.truncate(8);
    }
    if u.daily.len() < 2 && daily.len() > 1 {
        u.daily = daily
            .iter()
            .map(|(date, value)| UsageDay {
                date: date.to_string(),
                value: *value,
            })
            .collect();
        u.daily_unit = Some("tokens".into());
    }
    u.daily_models = days
        .into_iter()
        .map(|((date, model), (i, o))| ModelDay {
            date,
            model,
            tokens: i.saturating_add(o),
        })
        .collect();
}

/// The local day an RFC 3339 time falls on, `YYYY-MM-DD`.
fn local_day(stamp: &str) -> Option<String> {
    let t = chrono::DateTime::parse_from_rfc3339(stamp).ok()?;
    Some(
        t.with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string(),
    )
}

// Claude keeps its own daily activity roll-up: cheap and accurate for messages, sessions, tools
fn apply_stats_cache(u: &mut ProviderUsage, v: &Value) {
    if let Some(days) = v["dailyActivity"].as_array() {
        u.active_days = days.len() as u64;
        for d in days {
            u.messages = u.messages.saturating_add(u64_at(d, "messageCount"));
            u.sessions = u.sessions.saturating_add(u64_at(d, "sessionCount"));
            u.tool_calls = u.tool_calls.saturating_add(u64_at(d, "toolCallCount"));
        }
        u.daily = days
            .iter()
            .skip(days.len().saturating_sub(30))
            .map(|d| UsageDay {
                date: d["date"].as_str().unwrap_or("").to_string(),
                value: u64_at(d, "messageCount"),
            })
            .collect();
        if !u.daily.is_empty() {
            u.daily_unit = Some("msgs".into());
        }
    }
    // the cache also carries authoritative lifetime totals; prefer them over the daily sum
    if let Some(n) = v["totalSessions"].as_u64().filter(|n| *n > 0) {
        u.sessions = n;
    }
    if let Some(n) = v["totalMessages"].as_u64().filter(|n| *n > 0) {
        u.messages = n;
    }
    // tokens per day make a better activity sparkline than message counts
    if let Some(days) = v["dailyModelTokens"].as_array() {
        let mut daily: Vec<UsageDay> = days
            .iter()
            .map(|d| UsageDay {
                date: d["date"].as_str().unwrap_or("").to_string(),
                value: d["tokensByModel"]
                    .as_object()
                    .map(|m| {
                        m.values()
                            .filter_map(Value::as_u64)
                            .fold(0u64, u64::saturating_add)
                    })
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
            let i = u64_at(m, "inputTokens");
            let o = u64_at(m, "outputTokens");
            let c = u64_at(m, "cacheReadInputTokens")
                .saturating_add(u64_at(m, "cacheCreationInputTokens"));
            let total = i.saturating_add(o).saturating_add(c);
            if total == 0 {
                continue;
            }
            u.models.push(ModelUsage {
                model: name.clone(),
                input_tokens: i,
                output_tokens: o,
                cache_tokens: c,
                total_tokens: total,
            });
        }
        u.models.sort_by_key(|m| std::cmp::Reverse(m.total_tokens));
        u.models.truncate(8);
    }
}

/// Tokens in, out and from cache over the recent transcripts, and in and out per (day, model).
type Days = BTreeMap<(String, String), (u64, u64)>;

fn sum_claude_tokens(projects: &Path, now: u64) -> (u64, u64, u64, Days) {
    let (mut i, mut o, mut c) = (0u64, 0u64, 0u64);
    let mut days = Days::new();
    for f in recent_jsonl(projects, now) {
        if std::fs::metadata(&f).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        // stream line by line: reading the whole file allocated up to MAX_FILE_BYTES per transcript
        let Ok(file) = std::fs::File::open(&f) else {
            continue;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if !line.contains("\"output_tokens\"") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let usage = if v["message"]["usage"].is_object() {
                &v["message"]["usage"]
            } else {
                &v["usage"]
            };
            if usage.is_object() {
                let (li, lo) = (
                    u64_at(usage, "input_tokens"),
                    u64_at(usage, "output_tokens"),
                );
                i = i.saturating_add(li);
                o = o.saturating_add(lo);
                c = c
                    .saturating_add(u64_at(usage, "cache_creation_input_tokens"))
                    .saturating_add(u64_at(usage, "cache_read_input_tokens"));
                // a reply Claude made up itself carries "<synthetic>" for a model
                let model = v["message"]["model"]
                    .as_str()
                    .filter(|m| !m.is_empty() && !m.starts_with('<'));
                let day = v["timestamp"].as_str().and_then(local_day);
                if let (Some(model), Some(day)) = (model, day) {
                    let e = days.entry((day, model.to_string())).or_default();
                    e.0 = e.0.saturating_add(li);
                    e.1 = e.1.saturating_add(lo);
                }
            }
        }
    }
    (i, o, c, days)
}

// ---- Codex ----

fn codex_usage(home: &Path, now: u64) -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "codex".into(),
        label: "Codex".into(),
        ..Default::default()
    };
    let cdir = home.join(".codex");

    if let Some(v) = read_json(&cdir.join("auth.json")) {
        u.signed_in = true;
        if let Some(p) = v["tokens"]["id_token"].as_str().and_then(decode_jwt) {
            u.account = p["email"].as_str().map(String::from);
            if let Some(plan) = p["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str() {
                u.plan = Some(format!("ChatGPT {}", title_case(plan)));
            }
        }
        if u.plan.is_none() && v["OPENAI_API_KEY"].as_str().is_some() {
            u.plan = Some("API key".into());
        }
    }

    // session rollouts carry token_count events with the live rate-limit windows
    let files = recent_jsonl(&cdir.join("sessions"), now);
    // only rollout-*.jsonl are real sessions; other jsonls in the tree would over-count
    u.sessions = files
        .iter()
        .filter(|p| file_name(p).is_some_and(|n| n.starts_with("rollout-")))
        .count() as u64;
    let (mut i, mut o, mut c) = (0u64, 0u64, 0u64);
    let mut by_day: BTreeMap<String, u64> = BTreeMap::new();
    let mut days = Days::new();
    for f in &files {
        let Some(tc) = last_token_count(f) else {
            continue;
        };
        let ttu = &tc["info"]["total_token_usage"];
        let (fi, fo) = (u64_at(ttu, "input_tokens"), u64_at(ttu, "output_tokens"));
        i = i.saturating_add(fi);
        o = o.saturating_add(fo);
        c = c.saturating_add(u64_at(ttu, "cached_input_tokens"));
        // rollout filenames embed the session date: rollout-YYYY-MM-DD...
        if let Some(d) = file_name(f)
            .and_then(|n| n.strip_prefix("rollout-"))
            .and_then(|n| n.get(..10))
        {
            let e = by_day.entry(d.to_string()).or_insert(0);
            *e = e.saturating_add(u64_at(ttu, "total_tokens"));
            // the whole session goes to the model it ran on
            let model = rollout_model(f).unwrap_or_else(|| "Codex".into());
            let e = days.entry((d.to_string(), model)).or_default();
            e.0 = e.0.saturating_add(fi);
            e.1 = e.1.saturating_add(fo);
        }
        // Windows come from the newest rollout that actually carries them, not simply the newest
        // rollout: codex writes `rate_limits` on every session but leaves primary/secondary null
        // unless the server sent limits that turn, so the latest file is often empty.
        if u.primary.is_none() && u.secondary.is_none() {
            let rl = &tc["rate_limits"];
            let (p, s) = (window_from(&rl["primary"]), window_from(&rl["secondary"]));
            if p.is_some() || s.is_some() {
                u.primary = p;
                u.secondary = s;
                // age of the reading, not of the newest session
                u.updated_at = mtime_secs(f) as i64;
                if u.plan.is_none()
                    && let Some(pt) = rl["plan_type"].as_str()
                {
                    u.plan = Some(format!("ChatGPT {}", title_case(pt)));
                }
            }
        }
    }
    u.input_tokens = i;
    u.output_tokens = o;
    u.cache_tokens = c;
    u.total_tokens = i.saturating_add(o).saturating_add(c);
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
    by_model_and_day(&mut u, days);
    u
}

/// The model a Codex session ran on, from the first turn's context near the top of its rollout.
fn rollout_model(file: &Path) -> Option<String> {
    let f = std::fs::File::open(file).ok()?;
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .take(400)
        .filter(|l| l.contains("\"turn_context\""))
        .filter_map(|l| serde_json::from_str::<Value>(&l).ok())
        .find_map(|v| v["payload"]["model"].as_str().map(String::from))
}

fn file_name(p: &Path) -> Option<&str> {
    p.file_name().and_then(|n| n.to_str())
}

// The last token_count event in a rollout (it has the current rate limits). Only the tail
// matters, so seek and read the last chunk instead of the whole multi-MB file.
fn last_token_count(file: &Path) -> Option<Value> {
    const TAIL_BYTES: u64 = 256 * 1024;
    let len = std::fs::metadata(file).ok()?.len();
    if len > MAX_FILE_BYTES {
        return None;
    }
    let mut f = std::fs::File::open(file).ok()?;
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut raw = Vec::with_capacity((len - start) as usize);
    f.read_to_end(&mut raw).ok()?;
    // a seek can land mid-utf8 or mid-line; convert lossily and drop the partial first line
    let text = String::from_utf8_lossy(&raw);
    let body = if start > 0 {
        text.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        &text
    };
    body.lines()
        .rev()
        .filter(|l| l.contains("\"token_count\"") && l.contains("\"rate_limits\""))
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["payload"]["type"] == "token_count")
        .map(|v| v["payload"].clone())
}

// ---- OpenCode ----

fn opencode_usage(home: &Path) -> ProviderUsage {
    let mut u = ProviderUsage {
        id: "opencode".into(),
        label: "OpenCode".into(),
        ..Default::default()
    };
    let auth = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("auth.json");
    if let Some(obj) = read_json(&auth).and_then(|v| v.as_object().cloned()) {
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
    if u.note.is_none() {
        u.note = Some("Bring your own model. Usage lives in OpenCode's own local database.".into());
    }
    u
}

// ---- Grok ----

fn grok_usage(home: &Path, now: u64, xai_key: bool) -> ProviderUsage {
    let g = home.join(".grok");
    // a grok session is one folder holding a chat_history.jsonl (it also writes events and prompt
    // jsonls alongside, which would triple-count). Filter to chat_history.jsonl BEFORE the file
    // cap, or the sibling jsonls could crowd the real sessions out of the top-160 window.
    let cutoff = now.saturating_sub(RECENT_DAYS * 86_400);
    let mut sess: Vec<(u64, PathBuf)> = vec![];
    collect_jsonl(&g.join("sessions"), &mut sess, 0);
    sess.retain(|(m, p)| *m >= cutoff && file_name(p) == Some("chat_history.jsonl"));
    sess.truncate(MAX_FILES);
    ProviderUsage {
        id: "grok".into(),
        label: "Grok".into(),
        signed_in: g.exists() || xai_key,
        sessions: sess.len() as u64,
        note: Some("Grok Build CLI. Token usage and rate limits live in your xAI console.".into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn write(p: &Path, body: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn token_count(primary: Value, total: u64) -> String {
        json!({ "type": "event_msg", "payload": {
            "type": "token_count",
            "info": { "total_token_usage": { "input_tokens": 10, "output_tokens": 5, "cached_input_tokens": 1, "total_tokens": total } },
            "rate_limits": { "primary": primary, "secondary": null, "plan_type": "pro" },
        }})
        .to_string()
    }

    #[test]
    fn last_token_count_reads_the_tail() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("rollout.jsonl");
        let body = [
            token_count(json!({ "used_percent": 1.0 }), 1),
            "{\"type\":\"other\"}".into(),
            token_count(json!({ "used_percent": 2.0 }), 2),
        ]
        .join("\n");
        write(&p, &body);
        let tc = last_token_count(&p).unwrap();
        assert_eq!(tc["rate_limits"]["primary"]["used_percent"], 2.0);
    }

    #[test]
    fn codex_takes_windows_from_the_newest_rollout_that_has_them() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join(".codex/sessions/2026/10/02");
        write(
            &day.join("rollout-2026-10-01-a.jsonl"),
            &token_count(
                json!({ "used_percent": 42.0, "window_minutes": 300, "resets_at": 9 }),
                100,
            ),
        );
        // its windows are null, so the reading has to come from the other rollout
        write(
            &day.join("rollout-2026-10-02-b.jsonl"),
            &token_count(Value::Null, 50),
        );
        let u = codex_usage(home.path(), now_secs());
        assert_eq!(u.sessions, 2);
        assert_eq!(u.primary.unwrap().used_percent, 42.0);
        assert_eq!(u.plan.as_deref(), Some("ChatGPT Pro"));
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.cache_tokens),
            (20, 10, 2)
        );
        assert_eq!(u.daily.len(), 2);
        // no turn context in these rollouts, so the sessions go to Codex at large
        assert_eq!(u.daily_models.len(), 2);
        assert!(u.daily_models.iter().all(|d| d.model == "Codex"));
    }

    #[test]
    fn codex_names_the_model_a_session_ran_on() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join(".codex/sessions/2026/10/02");
        let ctx = json!({ "type": "turn_context", "payload": { "model": "gpt-6-luna" } });
        write(
            &day.join("rollout-2026-10-02-a.jsonl"),
            &format!("{ctx}\n{}", token_count(Value::Null, 15)),
        );
        let u = codex_usage(home.path(), now_secs());
        assert_eq!(
            u.daily_models,
            [ModelDay {
                date: "2026-10-02".into(),
                model: "gpt-6-luna".into(),
                tokens: 15,
            }]
        );
        assert_eq!(u.models[0].model, "gpt-6-luna");
    }

    #[test]
    fn claude_splits_transcripts_by_day_and_model() {
        let home = tempfile::tempdir().unwrap();
        let line = |stamp: &str, model: &str, i: u64, o: u64| {
            json!({ "timestamp": stamp, "message": { "model": model, "usage": { "input_tokens": i, "output_tokens": o } } })
                .to_string()
        };
        // midday UTC lands on the same local day in every time zone a person lives in
        let body = [
            line("2026-10-01T12:00:00Z", "claude-opus-5-5", 10, 5),
            line("2026-10-01T12:30:00Z", "claude-opus-5-5", 1, 1),
            line("2026-10-02T12:00:00Z", "claude-haiku-4-5", 2, 2),
            line("2026-10-02T12:00:00Z", "<synthetic>", 9, 9),
        ]
        .join("\n");
        write(&home.path().join(".claude/projects/p/s.jsonl"), &body);
        let u = claude_usage(home.path(), now_secs());
        let mut got: Vec<(String, String, u64)> = u
            .daily_models
            .iter()
            .map(|d| (d.date.clone(), d.model.clone(), d.tokens))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("2026-10-01".into(), "claude-opus-5-5".into(), 17),
                ("2026-10-02".into(), "claude-haiku-4-5".into(), 4),
            ]
        );
        // no stats file: the model split and the days come from the transcripts
        assert_eq!(u.models[0].model, "claude-opus-5-5");
        assert_eq!(u.daily.len(), 2);
    }

    #[test]
    fn claude_reads_the_stats_cache_and_transcripts() {
        let home = tempfile::tempdir().unwrap();
        let cdir = home.path().join(".claude");
        write(
            &cdir.join("stats-cache.json"),
            &json!({
                "dailyActivity": [
                    { "date": "2026-10-01", "messageCount": 3, "sessionCount": 1, "toolCallCount": 2 },
                    { "date": "2026-10-02", "messageCount": 4, "sessionCount": 2, "toolCallCount": 0 },
                ],
                "totalMessages": 99,
                "modelUsage": { "opus": { "inputTokens": 5, "outputTokens": 5 }, "idle": {} },
            })
            .to_string(),
        );
        write(
            &cdir.join("projects/p/s.jsonl"),
            &json!({ "message": { "usage": { "input_tokens": 7, "output_tokens": 3, "cache_read_input_tokens": 1 } } })
                .to_string(),
        );
        let u = claude_usage(home.path(), now_secs());
        assert_eq!(
            (u.messages, u.sessions, u.tool_calls, u.active_days),
            (99, 3, 2, 2)
        );
        assert_eq!(u.daily_unit.as_deref(), Some("msgs"));
        assert_eq!(u.models.len(), 1);
        assert_eq!(u.total_tokens, 11);
        assert!(!u.signed_in);
    }

    #[test]
    fn grok_counts_only_chat_histories() {
        let home = tempfile::tempdir().unwrap();
        let s = home.path().join(".grok/sessions/one");
        write(&s.join("chat_history.jsonl"), "{}");
        write(&s.join("events.jsonl"), "{}");
        let u = grok_usage(home.path(), now_secs(), false);
        assert_eq!(u.sessions, 1);
        assert!(u.signed_in);
        assert!(provider_usage("nope").is_none());
    }
}
