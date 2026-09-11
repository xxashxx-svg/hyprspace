#![allow(unused_imports)]
use super::*;
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// One conversation a CLI saved on disk, as the composer lists it.
#[derive(Serialize)]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub modified: u64,
}

const MAX: usize = 12;
// transcripts run to megabytes; the title is always near the top
const SCAN_LINES: usize = 400;

/// The provider's saved conversations for `cwd`, newest first. Claude reads its per-folder
/// transcripts, Codex its rollout files. Other CLIs have no resume by id, so they get nothing.
#[tauri::command]
pub async fn agent_sessions(provider: String, cwd: String) -> Vec<AgentSession> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = match provider.as_str() {
            "claude" => claude_sessions_in(&cwd),
            "codex" => codex_sessions_in(&cwd),
            _ => vec![],
        };
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out.truncate(MAX);
        out
    })
    .await
    .unwrap_or_default()
}

fn mtime(p: &Path) -> u64 {
    std::fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

// the first line of what the user typed, trimmed to a row. Injected context (`<command-name>`,
// `<recommended_plugins>`) starts with an angle bracket and is not a title.
fn clean(t: &str) -> Option<String> {
    let t = t.trim();
    if t.is_empty() || t.starts_with('<') {
        return None;
    }
    let one = t.lines().next().unwrap_or("").trim();
    let mut s: String = one.chars().take(80).collect();
    if one.chars().count() > 80 {
        s.push('…');
    }
    Some(s)
}

fn first_text(blocks: &Value) -> Option<&str> {
    blocks.as_array()?.iter().find_map(|b| b["text"].as_str())
}

fn claude_sessions_in(cwd: &str) -> Vec<AgentSession> {
    let Some(dir) = crate::claude_project_dir(cwd) else {
        return vec![];
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out = vec![];
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map_or(true, |x| x != "jsonl") {
            continue;
        }
        let Some(id) = p.file_stem().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };
        let Some(title) = claude_title(&p) else {
            continue;
        };
        out.push(AgentSession { id, title, modified: mtime(&p) });
    }
    out
}

// what the user typed first; failing that, the summary claude wrote for the chat
fn claude_title(p: &Path) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    let mut summary = None;
    for line in BufReader::new(f).lines().take(SCAN_LINES).map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("summary") => {
                if summary.is_none() {
                    summary = v["summary"].as_str().and_then(clean);
                }
            }
            Some("user") if !v["isMeta"].as_bool().unwrap_or(false) => {
                let c = &v["message"]["content"];
                let text = c.as_str().or_else(|| first_text(c));
                if let Some(t) = text.and_then(clean) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    summary
}

fn codex_sessions_in(cwd: &str) -> Vec<AgentSession> {
    let mut files = vec![];
    walk(&home_dir().join(".codex").join("sessions"), &mut files, 0);
    let mut out = vec![];
    for p in files {
        let Ok(f) = std::fs::File::open(&p) else {
            continue;
        };
        let mut lines = BufReader::new(f).lines();
        let Some(Ok(head)) = lines.next() else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<Value>(&head) else {
            continue;
        };
        if meta["type"].as_str() != Some("session_meta") {
            continue;
        }
        let pl = &meta["payload"];
        if !pl["cwd"].as_str().map_or(false, |c| norm(c) == norm(cwd)) {
            continue;
        }
        let Some(id) = pl["id"].as_str().map(String::from) else {
            continue;
        };
        let mut title = None;
        for line in lines.take(SCAN_LINES).map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let pl = &v["payload"];
            if pl["type"].as_str() == Some("item_completed") && pl["item"]["type"].as_str() == Some("UserMessage") {
                title = first_text(&pl["item"]["content"]).and_then(clean);
            } else if v["type"].as_str() == Some("response_item") && pl["role"].as_str() == Some("user") {
                title = first_text(&pl["content"]).and_then(clean);
            }
            if title.is_some() {
                break;
            }
        }
        let Some(title) = title else {
            continue;
        };
        out.push(AgentSession { id, title, modified: mtime(&p) });
    }
    out
}

// rollouts sit under sessions/YYYY/MM/DD
fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else if p.extension().map_or(false, |x| x == "jsonl") {
            out.push(p);
        }
    }
}
