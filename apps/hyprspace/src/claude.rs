// One structured Claude turn: the user's own `claude` binary in headless stream-json mode, the
// same invocation zeron's harness uses. Inference stays on the user's subscription through their
// CLI; no SDK, no API key, no token.

use std::path::PathBuf;
use std::process::Stdio;

use futures::channel::mpsc::UnboundedSender;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Env a parent Claude Code session leaves behind. A child claude that inherits these thinks it
/// is nested inside that session, so every spawn strips them (from `drop_claude_session_env` in
/// src-tauri/src/lib.rs).
pub const SESSION_ENV: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SSE_PORT",
];

const ARGS: &[&str] = &[
    "--print",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    // the CLI refuses stream-json output under --print without it
    "--verbose",
    "--include-partial-messages",
    "--permission-prompt-tool",
    "stdio",
];

#[derive(Debug, PartialEq)]
pub enum Event {
    Init {
        model: String,
    },
    Delta(String),
    /// A tool asked for approval. The spike has no approval UI yet, so it is denied.
    Denied(String),
    Done {
        ok: bool,
        ms: u64,
        text: String,
    },
    Failed(String),
}

enum Frame {
    Event(Event),
    Permission { id: String, tool: String },
    Skip,
}

fn parse(line: &str) -> Frame {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Frame::Skip;
    };
    let s = |v: &Value, k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") if s(&v, "subtype") == "init" => Frame::Event(Event::Init {
            model: s(&v, "model"),
        }),
        // subagent output carries a parent tool id; only the main thread's text goes in the reply
        Some("stream_event") if v["parent_tool_use_id"].is_null() => {
            let delta = &v["event"]["delta"];
            if v["event"]["type"] == "content_block_delta" && delta["type"] == "text_delta" {
                Frame::Event(Event::Delta(s(delta, "text")))
            } else {
                Frame::Skip
            }
        }
        Some("control_request") if v["request"]["subtype"] == "can_use_tool" => Frame::Permission {
            id: s(&v, "request_id"),
            tool: s(&v["request"], "tool_name"),
        },
        Some("result") => Frame::Event(Event::Done {
            ok: !v["is_error"].as_bool().unwrap_or(false),
            ms: v["duration_ms"].as_u64().unwrap_or(0),
            text: s(&v, "result"),
        }),
        _ => Frame::Skip,
    }
}

fn user_line(text: &str) -> String {
    json!({ "type": "user", "message": { "role": "user", "content": text }, "parent_tool_use_id": null })
        .to_string()
}

fn deny_line(request_id: &str) -> String {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": { "behavior": "deny", "message": "HyprSpace can't approve tools yet." },
        },
    })
    .to_string()
}

fn command(program: &str, cwd: &PathBuf) -> Command {
    let mut cmd = Command::new(program);
    if program == "cmd" {
        // npm installs claude as a .cmd shim, which only cmd.exe resolves
        cmd.args(["/c", "claude"]);
    }
    cmd.args(ARGS).current_dir(cwd);
    for k in SESSION_ENV {
        cmd.env_remove(k);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // a GUI app spawning a console program gets a stray console window without this
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

/// Runs one turn and streams events to `tx`. Dropping the future kills the child.
pub async fn run(prompt: String, cwd: PathBuf, tx: UnboundedSender<Event>) {
    if let Err(e) = turn(&prompt, &cwd, &tx).await {
        let _ = tx.unbounded_send(Event::Failed(e.to_string()));
    }
}

async fn turn(prompt: &str, cwd: &PathBuf, tx: &UnboundedSender<Event>) -> anyhow::Result<()> {
    let mut child = match command("claude", cwd).spawn() {
        Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound => {
            command("cmd", cwd).spawn()?
        }
        other => other?,
    };
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");

    // stdin stays open: closing it would end the session after this turn
    stdin
        .write_all(format!("{}\n", user_line(prompt)).as_bytes())
        .await?;
    stdin.flush().await?;

    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        match parse(&line) {
            Frame::Event(e) => {
                let _ = tx.unbounded_send(e);
            }
            Frame::Permission { id, tool } => {
                stdin
                    .write_all(format!("{}\n", deny_line(&id)).as_bytes())
                    .await?;
                stdin.flush().await?;
                let _ = tx.unbounded_send(Event::Denied(tool));
            }
            Frame::Skip => {}
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        let mut err = String::new();
        stderr.read_to_string(&mut err).await?;
        anyhow::bail!("claude exited with {status}: {}", err.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(line: &str) -> Option<Event> {
        match parse(line) {
            Frame::Event(e) => Some(e),
            _ => None,
        }
    }

    // Shapes captured from claude 2.1.287.
    #[test]
    fn reads_init_delta_and_result() {
        assert_eq!(
            event(
                r#"{"type":"system","subtype":"init","model":"claude-opus-5-5","session_id":"s"}"#
            ),
            Some(Event::Init {
                model: "claude-opus-5-5".into()
            })
        );
        assert_eq!(
            event(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}},"parent_tool_use_id":null}"#
            ),
            Some(Event::Delta("hi".into()))
        );
        assert_eq!(
            event(
                r#"{"type":"result","subtype":"success","is_error":false,"duration_ms":3796,"result":"hi"}"#
            ),
            Some(Event::Done {
                ok: true,
                ms: 3796,
                text: "hi".into()
            })
        );
    }

    #[test]
    fn skips_subagent_text_and_unknown_frames() {
        assert_eq!(
            event(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}},"parent_tool_use_id":"toolu_1"}"#
            ),
            None
        );
        assert_eq!(event(r#"{"type":"rate_limit_event"}"#), None);
        assert_eq!(event("not json"), None);
    }

    #[test]
    fn tool_approval_is_a_permission_frame() {
        let line = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{}}}"#;
        assert!(
            matches!(parse(line), Frame::Permission { id, tool } if id == "r1" && tool == "Bash")
        );
        assert!(deny_line("r1").contains(r#""behavior":"deny""#));
    }
}
