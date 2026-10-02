// Claude's tool_use blocks and tool_result content, reduced to the proto's `Tool` and output
// text. Tool names and input fields as claude 2.1.287 sends them; the mapping follows zeron's
// `decode_tool_use` (MIT, see THIRD_PARTY_NOTICES.md).

use hyprspace_proto::Tool;
use hyprspace_proto::run::{ChangeKind, FileChange};
use serde_json::Value;

fn field(input: &Value, key: &str) -> String {
    input[key].as_str().unwrap_or_default().to_string()
}

/// A hunk with no line numbers: the old text out, the new text in.
fn hunk(old: &str, new: &str) -> String {
    let mut out = String::from("@@");
    for line in old.lines() {
        out.push_str("\n-");
        out.push_str(line);
    }
    for line in new.lines() {
        out.push_str("\n+");
        out.push_str(line);
    }
    out
}

fn change(path: String, kind: ChangeKind, diff: String) -> Tool {
    Tool::Edit {
        changes: vec![FileChange { path, kind, diff }],
    }
}

pub(crate) fn decode(name: &str, input: &Value) -> Tool {
    match name {
        "Bash" => Tool::Command {
            command: field(input, "command"),
        },
        "Read" => Tool::Read {
            path: field(input, "file_path"),
        },
        "Edit" => change(
            field(input, "file_path"),
            ChangeKind::Update,
            hunk(&field(input, "old_string"), &field(input, "new_string")),
        ),
        "MultiEdit" => {
            let hunks: Vec<String> = input["edits"]
                .as_array()
                .map(|a| a.as_slice())
                .unwrap_or_default()
                .iter()
                .map(|e| hunk(&field(e, "old_string"), &field(e, "new_string")))
                .collect();
            change(
                field(input, "file_path"),
                ChangeKind::Update,
                hunks.join("\n"),
            )
        }
        // Write replaces the whole file; whether it existed is not on the wire
        "Write" => change(
            field(input, "file_path"),
            ChangeKind::Add,
            hunk("", &field(input, "content")),
        ),
        "Grep" | "Glob" => Tool::Search {
            pattern: field(input, "pattern"),
            path: input["path"].as_str().map(str::to_string),
        },
        "WebFetch" => Tool::Web {
            target: field(input, "url"),
        },
        "WebSearch" => Tool::Web {
            target: field(input, "query"),
        },
        // `Task` is the tool's name in older builds
        "Agent" | "Task" => Tool::Agent {
            description: field(input, "description"),
            agent_type: field(input, "subagent_type"),
            prompt: field(input, "prompt"),
        },
        // MCP tools arrive as mcp__<server>__<tool>
        _ => match name.strip_prefix("mcp__").and_then(|r| r.split_once("__")) {
            Some((server, tool)) => Tool::Mcp {
                server: server.into(),
                tool: tool.into(),
                input: input.to_string(),
            },
            None => Tool::Other {
                name: name.into(),
                input: input.to_string(),
            },
        },
    }
}

/// A tool_result's content: a plain string or an array of text blocks.
pub(crate) fn result_text(content: &Value) -> String {
    crate::cap(&joined(content))
}

fn joined(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => {
            let texts: Vec<&str> = blocks.iter().filter_map(|b| b["text"].as_str()).collect();
            texts.join("\n")
        }
        _ => String::new(),
    }
}

/// An Agent call's result is the subagent's report inside the CLI's framing: a
/// "[Subagent hand-back]" line telling the model not to trust it, the report indented two
/// spaces, then `agentId:` and a `<usage>` block. Only the report is for the user. Without the
/// hand-back line (older builds) the report runs up to the `agentId:` line.
pub(crate) fn report(content: &Value) -> String {
    let text = joined(content);
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    match lines
        .iter()
        .position(|l| l.starts_with("[Subagent hand-back]"))
    {
        Some(at) => {
            for line in &lines[at + 1..] {
                // the CLI indents every report line, so one at column zero is its own framing
                match line.strip_prefix("  ") {
                    Some(l) => out.push(l),
                    None if line.trim().is_empty() => out.push(""),
                    None => break,
                }
            }
        }
        None => {
            for line in &lines {
                if line.starts_with("agentId:") || line.starts_with("<usage>") {
                    break;
                }
                out.push(line);
            }
        }
    }
    out.join("\n").trim().to_string()
}

/// Whether an Agent call's result only says the subagent went off to run in the background.
pub(crate) fn launched(frame: &Value, content: &Value) -> bool {
    frame["tool_use_result"]["isAsync"] == true
        || joined(content).starts_with("Async agent launched")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decodes_the_tools_the_transcript_draws() {
        assert_eq!(
            decode("Bash", &json!({"command": "ls -la"})),
            Tool::Command {
                command: "ls -la".into()
            }
        );
        let Tool::Edit { changes } = decode(
            "Edit",
            &json!({"file_path": "/a.rs", "old_string": "a\nb", "new_string": "c"}),
        ) else {
            panic!("not an edit")
        };
        assert_eq!(changes[0].kind, ChangeKind::Update);
        assert_eq!(changes[0].diff, "@@\n-a\n-b\n+c");
        let Tool::Edit { changes } = decode(
            "MultiEdit",
            &json!({"file_path": "/a.rs", "edits": [
                {"old_string": "x", "new_string": "y"},
                {"old_string": "1", "new_string": "2"}]}),
        ) else {
            panic!("not an edit")
        };
        assert_eq!(changes[0].diff, "@@\n-x\n+y\n@@\n-1\n+2");
        assert_eq!(
            decode("mcp__linear__search", &json!({"q": "bug"})),
            Tool::Mcp {
                server: "linear".into(),
                tool: "search".into(),
                input: r#"{"q":"bug"}"#.into()
            }
        );
        assert_eq!(
            decode(
                "Agent",
                &json!({"description": "Write a poem", "subagent_type": "general-purpose",
                        "prompt": "Four lines.", "run_in_background": false})
            ),
            Tool::Agent {
                description: "Write a poem".into(),
                agent_type: "general-purpose".into(),
                prompt: "Four lines.".into()
            }
        );
        assert_eq!(
            decode("TodoWrite", &json!({})),
            Tool::Other {
                name: "TodoWrite".into(),
                input: "{}".into()
            }
        );
    }

    #[test]
    fn reads_both_result_shapes() {
        assert_eq!(result_text(&json!("ok")), "ok");
        assert_eq!(
            result_text(&json!([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}])),
            "a\nb"
        );
        assert_eq!(result_text(&Value::Null), "");
        assert!(result_text(&json!("x".repeat(9000))).ends_with("\n..."));
    }

    #[test]
    fn reports_lose_the_hand_back_framing() {
        // as claude 2.1.287 sends it, shortened
        let framed = json!([{ "type": "text", "text": "[Subagent hand-back] The text below is the \
            final report of a subagent. The report follows:\n  Two files sit here.\n\n  - **a.txt**\
            \n    nested\nagentId: ad36 (use SendMessage with to: 'ad36')\n<usage>subagent_tokens: \
            23141\ntool_uses: 1</usage>" }]);
        assert_eq!(
            report(&framed),
            "Two files sit here.\n\n- **a.txt**\n  nested"
        );
        assert_eq!(
            report(&json!("Done.\nagentId: x\n<usage>n</usage>")),
            "Done."
        );
        assert!(launched(
            &json!({ "tool_use_result": { "isAsync": true } }),
            &json!("")
        ));
        assert!(!launched(&json!({}), &framed));
    }
}
