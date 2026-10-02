// Codex thread items reduced to the proto's `Tool`, and how each one ended. Item shapes as
// `codex app-server generate-ts` describes them for codex-cli 0.159.3; the mapping follows
// zeron's codex/normalize.rs (MIT, see THIRD_PARTY_NOTICES.md).

use hyprspace_proto::Tool;
use hyprspace_proto::run::{ChangeKind, FileChange};
use serde_json::Value;

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn change(c: &Value) -> FileChange {
    // `kind` is `{ "type": "add" }` now and was a bare string in older builds
    let kind = c["kind"]["type"].as_str().or(c["kind"].as_str());
    FileChange {
        path: text(&c["path"]),
        kind: match kind {
            Some("add") => ChangeKind::Add,
            Some("delete") => ChangeKind::Delete,
            _ => ChangeKind::Update,
        },
        diff: text(&c["diff"]),
    }
}

/// The tool an item stands for, or None for items that are not tool calls (messages,
/// reasoning, the user's own prompt).
pub(crate) fn tool(item: &Value) -> Option<Tool> {
    Some(match item["type"].as_str()? {
        "commandExecution" => Tool::Command {
            command: text(&item["command"]),
        },
        "fileChange" => Tool::Edit {
            changes: item["changes"]
                .as_array()
                .map(|a| a.iter().map(change).collect())
                .unwrap_or_default(),
        },
        "mcpToolCall" => Tool::Mcp {
            server: text(&item["server"]),
            tool: text(&item["tool"]),
            input: item["arguments"].to_string(),
        },
        "dynamicToolCall" => Tool::Other {
            name: text(&item["tool"]),
            input: item["arguments"].to_string(),
        },
        "webSearch" => Tool::Web {
            target: text(&item["query"]),
        },
        "imageView" => Tool::Read {
            path: text(&item["path"]),
        },
        "collabAgentToolCall" => Tool::Other {
            name: format!("Agent: {}", text(&item["tool"])),
            input: text(&item["prompt"]),
        },
        _ => return None,
    })
}

/// Whether a finished tool item worked, and its output for display.
pub(crate) fn outcome(item: &Value) -> (bool, String) {
    let status = item["status"].as_str().unwrap_or("completed");
    let ok = !matches!(status, "failed" | "declined");
    match item["type"].as_str() {
        Some("commandExecution") => {
            let code = item["exitCode"].as_i64().unwrap_or(0);
            (
                ok && code == 0,
                crate::cap(item["aggregatedOutput"].as_str().unwrap_or_default()),
            )
        }
        Some("mcpToolCall") => {
            let output = match item["error"]["message"].as_str() {
                Some(e) => e.to_string(),
                None => item["result"]["content"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| c["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default(),
            };
            (ok && item["error"].is_null(), crate::cap(&output))
        }
        _ => (ok, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_tool_items() {
        let cmd = json!({"type": "commandExecution", "id": "c1", "command": "ls",
            "status": "completed", "exitCode": 1, "aggregatedOutput": "nope"});
        assert_eq!(
            tool(&cmd),
            Some(Tool::Command {
                command: "ls".into()
            })
        );
        assert_eq!(outcome(&cmd), (false, "nope".into()));

        let edit = json!({"type": "fileChange", "id": "f1", "status": "completed", "changes": [
            {"path": "/a.rs", "kind": {"type": "add"}, "diff": "+fn a() {}"},
            {"path": "/b.rs", "kind": "delete", "diff": ""},
            {"path": "/c.rs", "kind": {"type": "update", "move_path": null}, "diff": "@@ -1 +1 @@\n-a\n+b"}]});
        let Some(Tool::Edit { changes }) = tool(&edit) else {
            panic!("not an edit")
        };
        let kinds: Vec<ChangeKind> = changes.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [ChangeKind::Add, ChangeKind::Delete, ChangeKind::Update]
        );
        assert_eq!(changes[2].diff, "@@ -1 +1 @@\n-a\n+b");
        assert_eq!(outcome(&edit), (true, String::new()));

        let mcp = json!({"type": "mcpToolCall", "server": "s", "tool": "t", "arguments": {"q": 1},
            "status": "completed", "result": {"content": [{"type": "text", "text": "hit"}]}, "error": null});
        assert_eq!(outcome(&mcp), (true, "hit".into()));
        assert_eq!(tool(&json!({"type": "agentMessage", "text": "hi"})), None);
        assert_eq!(tool(&json!({"type": "reasoning"})), None);
    }
}
