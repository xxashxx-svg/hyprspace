use std::collections::HashSet;
use std::path::Path;

use hyprspace_proto::{Entry, Prompt, RunEvent, RunStatus, Tool};
use serde_json::Value;

use super::{resume, tools};

pub fn history(config: Option<&Path>, thread: &str) -> Option<Vec<Entry>> {
    let config = match config {
        Some(c) => c.to_path_buf(),
        None => resume::config_dir()?,
    };
    let path = resume::transcripts(&config, thread).into_iter().next()?;
    let text = std::fs::read_to_string(path).ok()?;
    Some(entries(&text))
}

fn said(text: &str) -> Option<String> {
    let text = text.trim();
    let skip = [
        "<command-",
        "<local-command",
        "<system-reminder>",
        "<bash-",
        "[Request interrupted",
    ];
    (!text.is_empty() && !skip.iter().any(|s| text.starts_with(s))).then(|| text.to_string())
}

fn entries(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut open = false;
    let mut last_text = false;
    let mut agents = HashSet::new();
    let run = |event| Entry::Run { event };
    let finish = |out: &mut Vec<Entry>, open: &mut bool| {
        if std::mem::take(open) {
            out.push(run(RunEvent::Finished {
                status: RunStatus::Done,
                ms: 0,
                text: String::new(),
                error: None,
            }));
        }
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let flag = |k: &str| v[k].as_bool() == Some(true);
        if flag("isSidechain") || flag("isMeta") || flag("isCompactSummary") {
            continue;
        }
        let content = &v["message"]["content"];
        let blocks = content.as_array().map(Vec::as_slice).unwrap_or_default();
        match v["type"].as_str() {
            Some("user") => {
                let typed = match content.as_str() {
                    Some(s) => said(s),
                    None => {
                        let parts: Vec<&str> = blocks
                            .iter()
                            .filter(|b| b["type"] == "text")
                            .filter_map(|b| b["text"].as_str())
                            .collect();
                        said(&parts.join("\n"))
                    }
                };
                if let Some(t) = typed {
                    finish(&mut out, &mut open);
                    out.push(Entry::Prompt {
                        prompt: Prompt::text(t),
                        at: None,
                    });
                    open = true;
                    last_text = false;
                    continue;
                }
                for b in blocks.iter().filter(|b| b["type"] == "tool_result") {
                    let id = b["tool_use_id"].as_str().unwrap_or_default().to_string();
                    let (output, images) = if agents.contains(&id) {
                        (tools::report(&b["content"]), Vec::new())
                    } else {
                        (
                            tools::result_text(&b["content"]),
                            crate::images::saved(&b["content"]),
                        )
                    };
                    let ok = b["is_error"].as_bool() != Some(true);
                    out.push(run(RunEvent::ToolDone {
                        id,
                        ok,
                        output,
                        images,
                    }));
                    last_text = false;
                }
            }
            Some("assistant") if open => {
                for b in blocks {
                    match b["type"].as_str() {
                        Some("text") => {
                            let t = b["text"].as_str().unwrap_or_default();
                            if t.trim().is_empty() {
                                continue;
                            }
                            let text = if last_text {
                                format!("\n\n{t}")
                            } else {
                                t.to_string()
                            };
                            out.push(run(RunEvent::Text { text }));
                            last_text = true;
                        }
                        Some("thinking") => {
                            let t = b["thinking"].as_str().unwrap_or_default();
                            if !t.trim().is_empty() {
                                out.push(run(RunEvent::Thinking {
                                    text: t.to_string(),
                                }));
                                last_text = false;
                            }
                        }
                        Some("tool_use") => {
                            let id = b["id"].as_str().unwrap_or_default().to_string();
                            let tool =
                                tools::decode(b["name"].as_str().unwrap_or_default(), &b["input"]);
                            if matches!(tool, Tool::Agent { .. }) {
                                agents.insert(id.clone());
                            }
                            out.push(run(RunEvent::Tool { id, tool }));
                            last_text = false;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    finish(&mut out, &mut open);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_conversation_reads_as_journal_entries() {
        let file = [
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"Caveat"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"list the files"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hm"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"a.rs"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"One file."}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Done."}]}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"content":"a subagent's prompt"}}"#,
            r#"{"type":"user","message":{"content":[{"type":"text","text":"thanks"}]}}"#,
        ]
        .join("\n");
        let e = entries(&file);
        let prompts: Vec<&str> = e
            .iter()
            .filter_map(|e| match e {
                Entry::Prompt { prompt, .. } => Some(prompt.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(prompts, ["list the files", "thanks"]);
        let texts: Vec<&str> = e
            .iter()
            .filter_map(|e| match e {
                Entry::Run {
                    event: RunEvent::Text { text },
                } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["One file.", "\n\nDone."]);
        assert!(e.iter().any(|e| matches!(
            e,
            Entry::Run {
                event: RunEvent::ToolDone { ok: true, .. }
            }
        )));
        let ends = e
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Entry::Run {
                        event: RunEvent::Finished { .. }
                    }
                )
            })
            .count();
        assert_eq!(ends, 2);
    }
}
