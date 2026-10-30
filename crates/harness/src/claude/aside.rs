// A side question (`/btw`) about a Claude conversation. It runs the thread's own command line, so
// the prompt cache the thread built still holds, resumed as a fork that isn't saved, answers in
// one turn and is refused every tool. Permission only drops to Ask, which the cache doesn't see.

use hyprspace_proto::{Answer, Aside, Launch, Permission, Prompt};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{Claude, args, chrome_file, chrome_on, mcp_config, resume, wire};
use crate::spawn::{exit_message, spawn, writer};

const FRAME: &str = "This is a side question, asked while you work. Answer it briefly from what \
    you already know in this conversation. Don't use any tools.";

impl Claude {
    /// Asks `question` of the conversation `launch.resume` names, telling `said` the answer as it
    /// streams and then `Done` or `Failed`. Dropping the future stops the CLI.
    pub async fn aside(
        &self,
        launch: Launch,
        question: String,
        images: Vec<std::path::PathBuf>,
        mut said: impl FnMut(Aside),
    ) {
        match self.ask_aside(launch, &question, images, &mut said).await {
            Ok(()) => said(Aside::Done),
            Err(message) => said(Aside::Failed { message }),
        }
    }

    async fn ask_aside(
        &self,
        mut launch: Launch,
        question: &str,
        images: Vec<std::path::PathBuf>,
        said: &mut impl FnMut(Aside),
    ) -> Result<(), String> {
        let Some(thread) = launch.resume.clone() else {
            return Err(
                "Send a message first. A side question asks about the conversation so far.".into(),
            );
        };
        if launch.permission != Permission::Plan {
            launch.permission = Permission::Ask;
        }
        let config = self.config.clone().or_else(resume::config_dir);
        let cwd = config
            .as_deref()
            .and_then(|c| resume::origin(c, &thread))
            .unwrap_or_else(|| launch.cwd.clone());
        let mut args = args(&launch);
        args.extend(
            [
                "--fork-session",
                "--no-session-persistence",
                "--max-turns",
                "1",
            ]
            .map(String::from),
        );
        if let Some(path) = self.mcp.as_ref().and_then(mcp_config) {
            args.extend(["--mcp-config".into(), path.to_string_lossy().into_owned()]);
        }
        if chrome_file().is_some_and(|f| chrome_on(&f)) {
            args.push("--chrome".into());
        }
        let mut proc = spawn(&self.program, &args, &cwd)
            .map_err(|e| format!("Could not start Claude: {e}"))?;
        let ask = wire::user_line(wire::content(&Prompt {
            text: format!("{FRAME}\n\n{question}"),
            images,
        }));
        let mut lines = BufReader::new(proc.stdout).lines();
        let stdin = writer(proc.stdin);
        let _ = stdin.send(ask);
        let mut streamed = false;
        while let Ok(Some(line)) = lines.next_line().await {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match v["type"].as_str() {
                Some("stream_event") if v["parent_tool_use_id"].is_null() => {
                    let e = &v["event"];
                    if e["type"] == "content_block_delta"
                        && e["delta"]["type"] == "text_delta"
                        && let Some(text) = e["delta"]["text"].as_str()
                    {
                        streamed = true;
                        said(Aside::Text { text: text.into() });
                    }
                }
                Some("control_request") if v["request"]["subtype"] == "can_use_tool" => {
                    let id = v["request_id"].as_str().unwrap_or_default();
                    let deny = wire::answer_line(
                        id,
                        v["request"]["input"].clone(),
                        Value::Null,
                        Answer::Deny,
                    );
                    let _ = stdin.send(deny);
                }
                Some("result") => {
                    return match v["subtype"].as_str() {
                        Some("error_max_turns") if !streamed => {
                            Err("Answering that needs a tool. Ask it in the thread instead.".into())
                        }
                        _ if v["is_error"] == true && !streamed => Err(v["result"]
                            .as_str()
                            .filter(|r| !r.trim().is_empty())
                            .unwrap_or("Claude couldn't answer that.")
                            .to_string()),
                        _ => {
                            if let Some(text) = v["result"].as_str().filter(|_| !streamed) {
                                said(Aside::Text { text: text.into() });
                            }
                            Ok(())
                        }
                    };
                }
                _ => {}
            }
        }
        Err(exit_message("Claude", &mut proc.child, &proc.stderr).await)
    }
}
