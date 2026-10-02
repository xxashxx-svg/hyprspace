// Fake `claude --print --input-format stream-json ...`. Frame shapes from claude 2.1.287.

use serde_json::{Value, json};

use crate::{emit, ignore_everything, read};

fn delta(kind: &str, field: &str, text: &str, parent: Value) {
    emit(json!({
        "type": "stream_event",
        "parent_tool_use_id": parent,
        "event": { "type": "content_block_delta", "index": 0,
                   "delta": { "type": kind, field: text } },
    }));
}

fn say(text: &str) {
    delta("text_delta", "text", text, Value::Null);
}

fn result(subtype: &str, text: &str, errors: Value) {
    emit(json!({
        "type": "result", "subtype": subtype, "is_error": subtype != "success",
        "duration_ms": 5, "result": text, "errors": errors,
        "usage": { "input_tokens": 10, "output_tokens": 20 },
        "session_id": "ignored",
    }));
}

fn done(text: &str) {
    result("success", text, json!([]));
}

fn prompt_text(msg: &Value) -> String {
    let content = &msg["message"]["content"];
    match content.as_str() {
        Some(s) => s.to_string(),
        None => content
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect(),
    }
}

fn control(id: &str, tool: &str, input: Value, rules: Value) -> Value {
    emit(json!({
        "type": "control_request", "request_id": id,
        "request": { "subtype": "can_use_tool", "tool_name": tool, "input": input,
                     "permission_suggestions": rules },
    }));
    read().unwrap_or_default()
}

pub fn run(args: &[String]) {
    let flag = |name: &str| {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).cloned()
    };
    let session = flag("--resume").unwrap_or_else(|| "fake-session".into());
    let model = flag("--model").unwrap_or_else(|| "claude-fake".into());
    let cwd = std::env::current_dir().unwrap_or_default();
    while let Some(msg) = read() {
        if msg["type"] != "user" {
            continue;
        }
        // the real CLI starts every turn with an init frame; the harness reports only the first
        emit(json!({
            "type": "system", "subtype": "init", "model": model,
            "session_id": session, "cwd": cwd, "tools": ["Bash"],
        }));
        let text = prompt_text(&msg);
        if text.contains("args") {
            let reply = format!("{}\ncwd={}", args.join(" "), cwd.display());
            say(&reply);
            done(&reply);
        } else if text.contains("hello") {
            hello();
        } else if text.contains("approve") {
            approve();
        } else if text.contains("absorb") {
            let _steer = read();
            say("absorbed");
            done("absorbed");
        } else if text.contains("steer-tool") {
            steer_after_tool();
        } else if text.contains("steer") {
            steer();
        } else if text.contains("wedge") {
            say("working");
            ignore_everything();
        } else if text.contains("interrupt") {
            say("working");
            let req = read().unwrap_or_default();
            if req["request"]["subtype"] != "interrupt" {
                say("expected an interrupt");
            }
            emit(json!({ "type": "control_response",
                "response": { "subtype": "success", "request_id": req["request_id"] } }));
            result(
                "error_during_execution",
                "",
                json!(["[ede_diagnostic] aborted"]),
            );
        } else if text.contains("image") {
            let blocks = msg["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let images: Vec<&str> = blocks
                .iter()
                .filter(|b| b["type"] == "image")
                .filter_map(|b| b["source"]["media_type"].as_str())
                .collect();
            let reply = format!("images: {}", images.join(","));
            say(&reply);
            done(&reply);
        } else if text.contains("crash") {
            say("partial");
            eprintln!("fake claude crashed");
            std::process::exit(3);
        } else if text.contains("fail") {
            result("error_max_turns", "", json!([]));
        } else {
            let reply = format!("echo: {text}");
            say(&reply);
            done(&reply);
        }
    }
}

fn hello() {
    delta("thinking_delta", "thinking", "pondering", Value::Null);
    say("Hel");
    say("lo");
    // a subagent's text never reaches the main reply
    delta("text_delta", "text", "SUBAGENT", json!("toolu_agent"));
    emit(json!({
        "type": "assistant", "parent_tool_use_id": null,
        "message": { "content": [
            { "type": "text", "text": "Hello" },
            { "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "ls" } },
            { "type": "tool_use", "id": "t2", "name": "Edit",
              "input": { "file_path": "/a.rs", "old_string": "a", "new_string": "b" } },
        ] },
    }));
    emit(json!({
        "type": "user", "parent_tool_use_id": null,
        "message": { "content": [
            { "type": "tool_result", "tool_use_id": "t1", "content": "file.txt", "is_error": false },
            { "type": "tool_result", "tool_use_id": "t2", "is_error": true,
              "content": [{ "type": "text", "text": "no such file" }] },
        ] },
    }));
    emit(json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "allowed" } }));
    done("Hello");
}

fn approve() {
    let mut notes = Vec::new();
    let rules = json!([{ "type": "addRules", "rules": [{ "toolName": "Bash" }] }]);
    let r1 = control("r1", "Bash", json!({ "command": "ls" }), rules.clone());
    let ok = &r1["response"];
    if ok["request_id"] != "r1"
        || ok["response"]["behavior"] != "allow"
        || ok["response"]["updatedInput"]["command"] != "ls"
        || ok["response"]["updatedPermissions"] != rules
    {
        notes.push(format!("bad always allow: {r1}"));
    }
    let r2 = control(
        "r2",
        "Write",
        json!({ "file_path": "/x", "content": "y" }),
        Value::Null,
    );
    if r2["response"]["response"]["behavior"] != "deny" {
        notes.push(format!("bad deny: {r2}"));
    }
    let reply = if notes.is_empty() {
        "approvals ok".to_string()
    } else {
        notes.join("; ")
    };
    say(&reply);
    done(&reply);
}

// A `now` steer cuts the turn short with a result, the CLI echoes the steer, then answers it.
fn steer() {
    say("first");
    let steer = read().unwrap_or_default();
    let priority = steer["priority"].as_str().unwrap_or("none").to_string();
    result(
        "error_during_execution",
        "",
        json!(["[ede_diagnostic] cut"]),
    );
    emit(steer.clone());
    let reply = format!("steered({priority}): {}", prompt_text(&steer));
    say(&reply);
    done(&reply);
}

// With a tool open, the steer must wait for it (`next`) and lands in the same turn.
fn steer_after_tool() {
    emit(json!({
        "type": "assistant", "parent_tool_use_id": null,
        "message": { "content": [
            { "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "sleep 1" } },
        ] },
    }));
    let steer = read().unwrap_or_default();
    let priority = steer["priority"].as_str().unwrap_or("none").to_string();
    emit(json!({
        "type": "user", "parent_tool_use_id": null,
        "message": { "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": "" }] },
    }));
    emit(steer.clone());
    let reply = format!("steered({priority}): {}", prompt_text(&steer));
    say(&reply);
    done(&reply);
}
