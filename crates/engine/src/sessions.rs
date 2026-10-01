// The conversations each CLI saved on disk, for the composer's resume list, and how a restored
// claude session should come back. Copied from src-tauri/src/devtools/sessions.rs and lib.rs.
//
// `claude --resume <id>` is folder-scoped: a conversation only resumes in the folder it was
// created in, so everything here is keyed by that folder.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use hyprspace_proto::agents::AgentSession;
use serde_json::Value;

use crate::util::{home_dir, mtime_ms};

const MAX: usize = 12;
// transcripts run to megabytes; the title is always near the top
const SCAN_LINES: usize = 400;

/// The provider's saved conversations for `cwd`, newest first. Claude reads its per-folder
/// transcripts, Codex its rollout files. Other CLIs have no resume by id, so they get nothing.
pub fn list(provider: &str, cwd: &Path) -> Vec<AgentSession> {
    list_in(&home_dir(), provider, cwd)
}

fn list_in(home: &Path, provider: &str, cwd: &Path) -> Vec<AgentSession> {
    let mut out = match provider {
        "claude" => claude_sessions(&claude_project_dir(home, cwd)),
        "codex" => codex_sessions(home, cwd),
        _ => vec![],
    };
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    out.truncate(MAX);
    out
}

/// `~/.claude/projects/<encoded cwd>`, where claude keeps a folder's transcripts.
///
/// claude encodes a cwd by replacing EVERY non-alphanumeric char with '-', not just separators.
/// Verified against ~/.claude/projects: all 131 dirs are [A-Za-z0-9-] only, and
/// C:\Users\x\.hyprspace\... lands at C--Users-x--hyprspace-... (the dot becomes a dash too).
/// Getting this wrong silently breaks resume for any path with a dot or space, i.e. every worktree.
pub fn claude_project_dir(home: &Path, cwd: &Path) -> PathBuf {
    let enc: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    home.join(".claude").join("projects").join(enc)
}

/// How a restored claude session should come back. claude stores each chat as
/// <session-id>.jsonl, and our session pins that id, so look for its own transcript first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resume {
    /// `claude --resume <id>`: its own chat exists, the normal path.
    Resume,
    /// `claude --continue`: no transcript for this id, but the folder has other history.
    Continue,
    Fresh,
}

pub fn resume_mode(cwd: &Path, session_id: &str) -> Resume {
    resume_in(&home_dir(), cwd, session_id)
}

fn resume_in(home: &Path, cwd: &Path, session_id: &str) -> Resume {
    let dir = claude_project_dir(home, cwd);
    if dir.join(format!("{session_id}.jsonl")).exists() {
        Resume::Resume
    } else if has_transcript(&dir) {
        Resume::Continue
    } else {
        Resume::Fresh
    }
}

fn has_transcript(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().any(|e| is_jsonl(&e.path())))
        .unwrap_or(false)
}

fn is_jsonl(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "jsonl")
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

fn claude_sessions(dir: &Path) -> Vec<AgentSession> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    rd.flatten()
        .filter_map(|e| {
            let p = e.path();
            if !is_jsonl(&p) {
                return None;
            }
            Some(AgentSession {
                id: p.file_stem()?.to_str()?.to_string(),
                title: claude_title(&p)?,
                modified: mtime_ms(&p),
            })
        })
        .collect()
}

// what the user typed first; failing that, the summary claude wrote for the chat
fn claude_title(p: &Path) -> Option<String> {
    let f = std::fs::File::open(p).ok()?;
    let mut summary = None;
    for line in BufReader::new(f)
        .lines()
        .take(SCAN_LINES)
        .map_while(Result::ok)
    {
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
                if let Some(t) = c.as_str().or_else(|| first_text(c)).and_then(clean) {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    summary
}

fn codex_sessions(home: &Path, cwd: &Path) -> Vec<AgentSession> {
    let mut files = vec![];
    walk(&home.join(".codex").join("sessions"), &mut files, 0);
    let want = norm(&cwd.to_string_lossy());
    files
        .into_iter()
        .filter_map(|p| {
            let (id, title) = codex_rollout(&p, &want)?;
            Some(AgentSession {
                id,
                title,
                modified: mtime_ms(&p),
            })
        })
        .collect()
}

// (id, title) of a rollout started in the folder `want`
fn codex_rollout(p: &Path, want: &str) -> Option<(String, String)> {
    let f = std::fs::File::open(p).ok()?;
    let mut lines = BufReader::new(f).lines();
    let meta: Value = serde_json::from_str(&lines.next()?.ok()?).ok()?;
    if meta["type"].as_str() != Some("session_meta") {
        return None;
    }
    let pl = &meta["payload"];
    if pl["cwd"].as_str().map(norm).as_deref() != Some(want) {
        return None;
    }
    let id = pl["id"].as_str()?.to_string();
    for line in lines.take(SCAN_LINES).map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let pl = &v["payload"];
        let title = if pl["type"].as_str() == Some("item_completed")
            && pl["item"]["type"].as_str() == Some("UserMessage")
        {
            first_text(&pl["item"]["content"]).and_then(clean)
        } else if v["type"].as_str() == Some("response_item") && pl["role"].as_str() == Some("user")
        {
            first_text(&pl["content"]).and_then(clean)
        } else {
            None
        };
        if let Some(t) = title {
            return Some((id, t));
        }
    }
    None
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
        } else if is_jsonl(&p) {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn lines(p: &Path, rows: &[Value]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let body: Vec<String> = rows.iter().map(Value::to_string).collect();
        std::fs::write(p, body.join("\n")).unwrap();
    }

    #[test]
    fn project_dir_encodes_every_non_alphanumeric() {
        let dir = claude_project_dir(Path::new("/h"), Path::new(r"C:\Users\x\.hyprspace\wt one"));
        assert!(dir.ends_with("C--Users-x--hyprspace-wt-one"), "{dir:?}");
    }

    #[test]
    fn titles_skip_injected_context_and_trim() {
        assert_eq!(clean("  <command-name>x"), None);
        assert_eq!(clean("fix it\nmore").as_deref(), Some("fix it"));
        let long = "a".repeat(90);
        assert_eq!(clean(&long).unwrap().chars().count(), 81);
    }

    #[test]
    fn lists_claude_transcripts_with_their_first_prompt() {
        let home = tempfile::tempdir().unwrap();
        let cwd = Path::new("/work/app");
        let dir = claude_project_dir(home.path(), cwd);
        lines(
            &dir.join("s1.jsonl"),
            &[
                json!({ "type": "user", "isMeta": true, "message": { "content": "meta" } }),
                json!({ "type": "user", "message": { "content": [{ "type": "text", "text": "Add login" }] } }),
            ],
        );
        lines(
            &dir.join("s2.jsonl"),
            &[json!({ "type": "summary", "summary": "Old chat" })],
        );
        lines(&dir.join("empty.jsonl"), &[]);
        let mut got = list_in(home.path(), "claude", cwd);
        got.sort_by(|a, b| a.id.cmp(&b.id));
        let rows: Vec<_> = got
            .iter()
            .map(|s| (s.id.as_str(), s.title.as_str()))
            .collect();
        assert_eq!(rows, [("s1", "Add login"), ("s2", "Old chat")]);
        assert!(list_in(home.path(), "gemini", cwd).is_empty());

        assert_eq!(resume_in(home.path(), cwd, "s1"), Resume::Resume);
        assert_eq!(resume_in(home.path(), cwd, "gone"), Resume::Continue);
        assert_eq!(
            resume_in(home.path(), Path::new("/new"), "s1"),
            Resume::Fresh
        );
    }

    #[test]
    fn lists_codex_rollouts_for_this_folder_only() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join(".codex/sessions/2026/10/02");
        let rollout = |name: &str, cwd: &str, text: &str| {
            lines(
                &day.join(format!("{name}.jsonl")),
                &[
                    json!({ "type": "session_meta", "payload": { "id": name, "cwd": cwd } }),
                    json!({ "type": "response_item", "payload": { "role": "user", "content": [{ "text": text }] } }),
                ],
            )
        };
        rollout("r1", r"C:\Work\App\", "Ship it");
        rollout("r2", "/elsewhere", "Nope");
        let got = list_in(home.path(), "codex", Path::new("c:/work/app"));
        assert_eq!(got.len(), 1);
        assert_eq!(
            (got[0].id.as_str(), got[0].title.as_str()),
            ("r1", "Ship it")
        );
    }
}
