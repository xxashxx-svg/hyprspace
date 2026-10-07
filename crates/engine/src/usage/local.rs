// Per-provider usage, read ENTIRELY from local files the CLIs already write (transcripts, session
// rollouts, stat caches, credential display fields). No provider API is called and no token is
// used: this is display-only aggregation. Copied from src-tauri/src/devtools/usage.rs.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hyprspace_proto::usage::{DayCount, ModelDay, ModelUsage, ProviderUsage, UsageWindow};
use serde_json::Value;

use crate::util::{decode_jwt, home_dir, read_json, title_case};

const RECENT_DAYS: u64 = 30; // token scans only look at recent files, so a settings tab stays snappy
const MAX_FILES: usize = 600; // hard cap on files scanned per provider
/// Codex rollouts are small and read from the tail, so a year of them is cheap.
const CODEX_DAYS: u64 = 365;
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
    jsonl_within(root, now, RECENT_DAYS)
}

fn jsonl_within(root: &Path, now: u64, days: u64) -> Vec<PathBuf> {
    let cutoff = now.saturating_sub(days * 86_400);
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

    let stats = read_json(&cdir.join("stats-cache.json"));
    if let Some(v) = &stats {
        apply_stats_cache(&mut u, v);
    }

    // tokens aren't rolled up anywhere fresh, so read the recent transcripts (bounded)
    let projects = cdir.join("projects");
    let start = window_start(now);
    let scan = scan_claude(&projects, recent_jsonl(&projects, now), &start);
    u.input_tokens = scan.total.input;
    u.output_tokens = scan.total.output;
    u.cache_tokens = scan.total.cache_read.saturating_add(scan.total.cache_write);
    u.total_tokens = scan.total.total;
    if u.total_tokens > 0 {
        u.tokens_window = Some(format!("last {RECENT_DAYS} days"));
    }
    let (mut days, mut sessions) = (scan.days, scan.sessions);
    if let Some(v) = &stats {
        stats_days(v, &mut days, &mut sessions);
    }
    by_model_and_day(&mut u, days, sessions);
    u
}

/// One model's tokens on one day, or over a stretch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Split {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    total: u64,
}

impl Split {
    fn add(&mut self, o: Split) {
        self.input = self.input.saturating_add(o.input);
        self.output = self.output.saturating_add(o.output);
        self.cache_read = self.cache_read.saturating_add(o.cache_read);
        self.cache_write = self.cache_write.saturating_add(o.cache_write);
        self.total = self.total.saturating_add(o.total);
    }
}

/// Tokens per (day, model).
type Days = BTreeMap<(String, String), Split>;

/// The first local day the transcripts' window covers.
fn window_start(now: u64) -> String {
    let at = chrono::DateTime::from_timestamp((now - RECENT_DAYS * 86_400) as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    at.format("%Y-%m-%d").to_string()
}

/// Lays Claude's stats file over the transcripts' days. Every day the file covers takes its
/// totals and sessions from it, so those days read exactly as `/stats` shows them (the file
/// keeps days whose transcripts Claude has since deleted); the transcripts still give those
/// days their split between input, output and the cache, and alone cover the days since.
fn stats_days(v: &Value, days: &mut Days, sessions: &mut BTreeMap<String, u64>) {
    let list = v["dailyModelTokens"].as_array();
    let through = v["lastComputedDate"]
        .as_str()
        .or_else(|| list?.iter().filter_map(|d| d["date"].as_str()).max())
        .unwrap_or("")
        .to_string();
    for ((date, _), s) in days.iter_mut() {
        if *date <= through {
            s.total = 0;
        }
    }
    sessions.retain(|date, _| *date > through);
    for d in list.into_iter().flatten() {
        let Some(date) = d["date"].as_str() else {
            continue;
        };
        for (model, n) in d["tokensByModel"].as_object().into_iter().flatten() {
            let e = days.entry((date.to_string(), model.clone())).or_default();
            e.total = e.total.saturating_add(n.as_u64().unwrap_or(0));
        }
    }
    for d in v["dailyActivity"].as_array().into_iter().flatten() {
        if let Some(date) = d["date"].as_str() {
            *sessions.entry(date.to_string()).or_default() += u64_at(d, "sessionCount");
        }
    }
}

/// Fills the per-model, per-day figures and the sessions per day. A provider with no model
/// split of its own takes it from here.
fn by_model_and_day(u: &mut ProviderUsage, mut days: Days, sessions: BTreeMap<String, u64>) {
    // a session that spent nothing has nothing to show
    days.retain(|_, s| s.total > 0);
    if u.models.is_empty() {
        let mut models: BTreeMap<&str, Split> = BTreeMap::new();
        for ((_, model), s) in &days {
            models.entry(model).or_default().add(*s);
        }
        u.models = models
            .iter()
            .map(|(model, s)| ModelUsage {
                model: model.to_string(),
                input_tokens: s.input,
                output_tokens: s.output,
                cache_tokens: s.cache_read.saturating_add(s.cache_write),
                cache_write_tokens: s.cache_write,
                total_tokens: s.total,
            })
            .collect();
        u.models.sort_by_key(|m| std::cmp::Reverse(m.total_tokens));
        u.models.truncate(8);
    }
    u.daily_models = days
        .into_iter()
        .map(|((date, model), s)| ModelDay {
            date,
            model,
            input: s.input,
            output: s.output,
            cache_read: s.cache_read,
            cache_write: s.cache_write,
            total: s.total,
        })
        .collect();
    u.day_sessions = sessions
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(date, count)| DayCount { date, count })
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
    }
    // the cache also carries authoritative lifetime totals; prefer them over the daily sum
    if let Some(n) = v["totalSessions"].as_u64().filter(|n| *n > 0) {
        u.sessions = n;
    }
    if let Some(n) = v["totalMessages"].as_u64().filter(|n| *n > 0) {
        u.messages = n;
    }
    // lifetime split by model
    if let Some(mu) = v["modelUsage"].as_object() {
        for (name, m) in mu {
            let i = u64_at(m, "inputTokens");
            let o = u64_at(m, "outputTokens");
            let w = u64_at(m, "cacheCreationInputTokens");
            let c = u64_at(m, "cacheReadInputTokens").saturating_add(w);
            let total = i.saturating_add(o).saturating_add(c);
            if total == 0 {
                continue;
            }
            u.models.push(ModelUsage {
                model: name.clone(),
                input_tokens: i,
                output_tokens: o,
                cache_tokens: c,
                cache_write_tokens: w,
                total_tokens: total,
            });
        }
        u.models.sort_by_key(|m| std::cmp::Reverse(m.total_tokens));
        u.models.truncate(8);
    }
}

/// What the recent transcripts say: tokens in all, per (day, model), and sessions per day.
#[derive(Default)]
struct Scan {
    total: Split,
    days: Days,
    sessions: BTreeMap<String, u64>,
}

/// Reads Claude's transcripts, counting every line's usage the way Claude's own `/stats` and
/// its `stats-cache.json` do. A reply written over several lines repeats its usage on each, so
/// this runs about three times what the API billed, but older days exist only in Claude's
/// count (it deletes transcripts after 30 days), and one way of counting keeps the days
/// comparable (ADR 0018). A session is a transcript right under its project's folder (subagents
/// keep theirs deeper), on the day it began. The total counts from `from` on: a transcript
/// touched lately can hold replies from long before.
fn scan_claude(projects: &Path, files: Vec<PathBuf>, from: &str) -> Scan {
    let mut scan = Scan::default();
    for f in files {
        if std::fs::metadata(&f).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        let session = f.parent().and_then(Path::parent) == Some(projects);
        let mut began: Option<String> = None;
        // stream line by line: reading the whole file allocated up to MAX_FILE_BYTES per transcript
        let Ok(file) = std::fs::File::open(&f) else {
            continue;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let usage_line = line.contains("\"output_tokens\"");
            let want_start = began.is_none() && line.contains("\"timestamp\"");
            if !usage_line && !want_start {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let day = v["timestamp"].as_str().and_then(local_day);
            if began.is_none() {
                began = day.clone();
            }
            let usage = if v["message"]["usage"].is_object() {
                &v["message"]["usage"]
            } else {
                &v["usage"]
            };
            if !usage_line || !usage.is_object() {
                continue;
            }
            let (Some(day), Some(model)) = (day, v["message"]["model"].as_str()) else {
                continue;
            };
            let mut s = Split {
                input: u64_at(usage, "input_tokens"),
                output: u64_at(usage, "output_tokens"),
                cache_read: u64_at(usage, "cache_read_input_tokens"),
                cache_write: u64_at(usage, "cache_creation_input_tokens"),
                total: 0,
            };
            s.total = s.input + s.output + s.cache_read + s.cache_write;
            if day.as_str() >= from {
                scan.total.add(s);
            }
            // a reply Claude made up itself carries "<synthetic>" for a model
            if !model.is_empty() && !model.starts_with('<') {
                scan.days
                    .entry((day, model.to_string()))
                    .or_default()
                    .add(s);
            }
        }
        if session && let Some(day) = began {
            *scan.sessions.entry(day).or_default() += 1;
        }
    }
    scan
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

    // session rollouts carry token_count events with the live rate-limit windows. A year of them
    // for the days; the card's own figures keep to the recent window
    let files = jsonl_within(&cdir.join("sessions"), now, CODEX_DAYS);
    let start = window_start(now);
    let (mut i, mut o, mut c) = (0u64, 0u64, 0u64);
    let mut days = Days::new();
    let mut sessions: BTreeMap<String, u64> = BTreeMap::new();
    for f in &files {
        // only rollout-*.jsonl are real sessions; other jsonls in the tree would over-count.
        // Their names carry the day: rollout-YYYY-MM-DD...
        let Some(d) = file_name(f)
            .and_then(|n| n.strip_prefix("rollout-"))
            .and_then(|n| n.get(..10))
            .map(String::from)
        else {
            continue;
        };
        *sessions.entry(d.clone()).or_default() += 1;
        let recent = d >= start;
        if recent {
            u.sessions += 1;
        }
        let Some(tc) = last_token_count(f) else {
            continue;
        };
        let ttu = &tc["info"]["total_token_usage"];
        let (fi, fo, fc) = (
            u64_at(ttu, "input_tokens"),
            u64_at(ttu, "output_tokens"),
            u64_at(ttu, "cached_input_tokens"),
        );
        if recent {
            i = i.saturating_add(fi);
            o = o.saturating_add(fo);
            c = c.saturating_add(fc);
        }
        // the whole session goes to the model it ran on; its input counts the cached part
        let model = rollout_model(f).unwrap_or_else(|| "Codex".into());
        days.entry((d, model)).or_default().add(Split {
            input: fi.saturating_sub(fc),
            output: fo,
            cache_read: fc,
            cache_write: 0,
            total: fi.saturating_add(fo),
        });
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
    by_model_and_day(&mut u, days, sessions);
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

    /// The fixtures' dates are early October 2026; the windows count back from here.
    const OCT_3: u64 = 1_791_028_800;

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
        let u = codex_usage(home.path(), OCT_3);
        assert_eq!(u.sessions, 2);
        assert_eq!(u.primary.unwrap().used_percent, 42.0);
        assert_eq!(u.plan.as_deref(), Some("ChatGPT Pro"));
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.cache_tokens),
            (20, 10, 2)
        );
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
        let u = codex_usage(home.path(), OCT_3);
        // input counts its cached part, which shows apart as read from cache
        assert_eq!(
            u.daily_models,
            [ModelDay {
                date: "2026-10-02".into(),
                model: "gpt-6-luna".into(),
                input: 9,
                output: 5,
                cache_read: 1,
                cache_write: 0,
                total: 15,
            }]
        );
        assert_eq!(u.models[0].model, "gpt-6-luna");
        assert_eq!(
            u.day_sessions,
            [DayCount {
                date: "2026-10-02".into(),
                count: 1
            }]
        );
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
        let u = claude_usage(home.path(), OCT_3);
        let mut got: Vec<(String, String, u64)> = u
            .daily_models
            .iter()
            .map(|d| (d.date.clone(), d.model.clone(), d.total))
            .collect();
        got.sort();
        assert_eq!(
            got,
            [
                ("2026-10-01".into(), "claude-opus-5-5".into(), 17),
                ("2026-10-02".into(), "claude-haiku-4-5".into(), 4),
            ]
        );
        // no stats file: the model split comes from the transcripts
        assert_eq!(u.models[0].model, "claude-opus-5-5");
        // the transcript is a session, on the day it began
        assert_eq!(
            u.day_sessions,
            [DayCount {
                date: "2026-10-01".into(),
                count: 1
            }]
        );
    }

    #[test]
    fn every_line_counts_as_claude_stats_counts_it() {
        let home = tempfile::tempdir().unwrap();
        let line = |out: u64, cache: u64| {
            json!({ "timestamp": "2026-10-01T12:00:00Z", "message": { "id": "msg_1", "model": "claude-opus-5-5",
                "usage": { "input_tokens": 3, "output_tokens": out, "cache_read_input_tokens": cache } } })
            .to_string()
        };
        // a text block, then a tool call, of the same reply: the usage repeats on each line
        let body = [line(40, 1000), line(40, 1000), line(90, 1000)].join("\n");
        write(&home.path().join(".claude/projects/p/s.jsonl"), &body);
        // the subagent's own transcript is no session of its own
        write(
            &home.path().join(".claude/projects/p/s/subagents/a.jsonl"),
            &line(10, 500),
        );
        let u = claude_usage(home.path(), OCT_3);
        assert_eq!(
            (u.input_tokens, u.output_tokens, u.cache_tokens),
            (12, 180, 3500)
        );
        assert_eq!(u.day_sessions.iter().map(|d| d.count).sum::<u64>(), 1);
    }

    #[test]
    fn the_card_counts_only_the_last_30_days_of_a_long_transcript() {
        let home = tempfile::tempdir().unwrap();
        let line = |at: &str, id: &str, out: u64| {
            json!({ "timestamp": at, "message": { "id": id, "model": "claude-opus-5-5",
                "usage": { "input_tokens": 1, "output_tokens": out } } })
            .to_string()
        };
        let body = [
            line("2026-07-01T12:00:00Z", "old", 500),
            line("2026-10-02T12:00:00Z", "new", 7),
        ]
        .join(
            "
",
        );
        write(&home.path().join(".claude/projects/p/s.jsonl"), &body);
        let u = claude_usage(home.path(), OCT_3);
        assert_eq!((u.input_tokens, u.output_tokens), (1, 7));
        // the days still have both, for the heatmap
        assert_eq!(u.daily_models.len(), 2);
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
            &json!({ "timestamp": "2026-10-02T12:00:00Z", "message": { "id": "m", "model": "claude-opus-5-5",
                "usage": { "input_tokens": 7, "output_tokens": 3, "cache_read_input_tokens": 1 } } })
            .to_string(),
        );
        let u = claude_usage(home.path(), OCT_3);
        assert_eq!(
            (u.messages, u.sessions, u.tool_calls, u.active_days),
            (99, 3, 2, 2)
        );
        assert_eq!(u.models.len(), 1);
        assert_eq!(u.total_tokens, 11);
        assert!(!u.signed_in);
    }

    #[test]
    fn the_stats_file_has_the_last_word_on_the_days_it_covers() {
        let home = tempfile::tempdir().unwrap();
        let cdir = home.path().join(".claude");
        write(
            &cdir.join("stats-cache.json"),
            &json!({
                "lastComputedDate": "2026-10-01",
                "dailyActivity": [{ "date": "2026-10-01", "sessionCount": 4 }],
                "dailyModelTokens": [{ "date": "2026-10-01", "tokensByModel": { "claude-opus-5-5": 1000 } }],
            })
            .to_string(),
        );
        let line = |at: &str| {
            json!({ "timestamp": at, "message": { "model": "claude-opus-5-5",
                "usage": { "input_tokens": 2, "output_tokens": 3 } } })
            .to_string()
        };
        write(
            &cdir.join("projects/p/a.jsonl"),
            &line("2026-10-01T12:00:00Z"),
        );
        write(
            &cdir.join("projects/p/b.jsonl"),
            &line("2026-10-02T12:00:00Z"),
        );
        let u = claude_usage(home.path(), OCT_3);
        let days: Vec<(&str, u64, u64)> = u
            .daily_models
            .iter()
            .map(|d| (d.date.as_str(), d.input, d.total))
            .collect();
        // the covered day keeps the file's total and the transcript's split
        assert_eq!(days, [("2026-10-01", 2, 1000), ("2026-10-02", 2, 5)]);
        let sessions: Vec<u64> = u.day_sessions.iter().map(|d| d.count).collect();
        assert_eq!(sessions, [4, 1]);
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
