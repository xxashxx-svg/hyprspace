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
        "modelUsage": { "claude-opus-5-5": { "inputTokens": 10, "contextWindow": 1000 } },
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
        } else if text.contains("background") {
            background();
        } else if text.contains("subagent") {
            subagent();
        } else if text.contains("approve") {
            approve();
        } else if text.contains("todo") {
            emit(json!({
                "type": "assistant", "parent_tool_use_id": null,
                "message": { "content": [{ "type": "tool_use", "id": "td1", "name": "TodoWrite",
                    "input": { "todos": [
                        { "content": "Read the handler", "status": "completed", "activeForm": "Reading the handler" },
                        { "content": "Fix the redirect", "status": "in_progress", "activeForm": "Fixing the redirect" },
                        { "content": "Run the tests", "status": "pending", "activeForm": "Running the tests" } ] } }] },
            }));
            emit(json!({
                "type": "user", "parent_tool_use_id": null,
                "message": { "content": [{ "type": "tool_result", "tool_use_id": "td1", "content": "ok" }] },
            }));
            say("tasks set");
            done("tasks set");
        } else if text.contains("plan") {
            let r = control(
                "p1",
                "ExitPlanMode",
                json!({ "plan": "## Plan

1. Read `auth/login.ts`
2. Add a `safe()` check
3. Cover it with a test" }),
                Value::Null,
            );
            let reply = format!(
                "plan {}",
                r["response"]["response"]["behavior"]
                    .as_str()
                    .unwrap_or("?")
            );
            say(&reply);
            done(&reply);
        } else if text.contains("question") {
            let r = control(
                "q1",
                "AskUserQuestion",
                json!({ "questions": [{ "question": "Which db?", "header": "DB",
                    "options": [{ "label": "Postgres" }, { "label": "SQLite" }],
                    "multiSelect": false }] }),
                Value::Null,
            );
            let got = &r["response"]["response"]["updatedInput"]["answers"]["Which db?"];
            let reply = format!("picked {}", got.as_str().unwrap_or("nothing"));
            say(&reply);
            done(&reply);
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
        ],
        "usage": { "input_tokens": 5, "cache_creation_input_tokens": 100,
                   "cache_read_input_tokens": 300, "output_tokens": 15 } },
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

fn assistant(parent: Value, content: Value) {
    emit(json!({
        "type": "assistant", "parent_tool_use_id": parent, "message": { "content": content },
    }));
}

fn tool_result(parent: Value, id: &str, content: Value, extra: Value) {
    let mut frame = json!({
        "type": "user", "parent_tool_use_id": parent,
        "message": { "content": [
            { "type": "tool_result", "tool_use_id": id, "content": content, "is_error": false },
        ] },
    });
    if let (Some(f), Some(e)) = (frame.as_object_mut(), extra.as_object()) {
        f.extend(e.clone());
    }
    emit(frame);
}

fn notification(id: &str, status: &str, summary: &str) {
    emit(json!({
        "type": "system", "subtype": "task_notification", "task_id": "task-1",
        "tool_use_id": id, "status": status, "summary": summary,
    }));
}

fn spawn(id: &str, parent: Value, description: &str, background: bool) {
    assistant(
        parent,
        json!([{ "type": "tool_use", "id": id, "name": "Agent", "input": {
            "description": description, "subagent_type": "general-purpose",
            "prompt": "Four lines.", "run_in_background": background } }]),
    );
}

// A subagent that runs in the foreground: its own traffic tagged with the Agent call's id, a
// subagent of its own, then the framed report as the call's result.
fn subagent() {
    let agent = json!("toolu_agent");
    spawn("toolu_agent", Value::Null, "Write a short poem", false);
    emit(json!({
        "type": "system", "subtype": "task_started", "task_id": "task-1",
        "tool_use_id": "toolu_agent", "subagent_type": "general-purpose",
    }));
    // the subagent's opening prompt, echoed on its own feed
    emit(json!({
        "type": "user", "parent_tool_use_id": agent,
        "message": { "content": [{ "type": "text", "text": "Four lines." }] },
    }));
    assistant(
        agent.clone(),
        json!([
            { "type": "text", "text": "Let me look." },
            { "type": "tool_use", "id": "s1", "name": "Bash", "input": { "command": "ls" } },
        ]),
    );
    tool_result(agent.clone(), "s1", json!("a.txt"), json!({}));
    spawn("s2", agent.clone(), "Read it", false);
    assistant(
        json!("s2"),
        json!([{ "type": "tool_use", "id": "s3", "name": "Read",
                 "input": { "file_path": "/a.txt" } }]),
    );
    tool_result(json!("s2"), "s3", json!("hello"), json!({}));
    tool_result(agent.clone(), "s2", json!("read"), json!({}));
    assistant(
        agent,
        json!([{ "type": "text", "text": "Roses are red.\nDone." }]),
    );
    tool_result(
        Value::Null,
        "toolu_agent",
        json!([{ "type": "text", "text": "[Subagent hand-back] The text below is the final \
            report of a subagent. The report follows:\n  Roses are red.\n  Done.\nagentId: a1 \
            (use SendMessage with to: 'a1')\n<usage>subagent_tokens: 5\ntool_uses: 2</usage>" }]),
        json!({}),
    );
    // said again after the result; the call already ended
    notification("toolu_agent", "completed", "again");
    say("Wrote it.");
    done("Wrote it.");
}

// A background subagent: its call returns a launch note, the run ends, the subagent works on,
// and its notification makes the CLI take a turn of its own.
fn background() {
    spawn("toolu_bg", Value::Null, "List files", true);
    tool_result(
        Value::Null,
        "toolu_bg",
        json!([{ "type": "text", "text": "Async agent launched successfully.\nagentId: b1" }]),
        json!({ "tool_use_result": { "isAsync": true, "status": "async_launched" } }),
    );
    say("Started it.");
    done("Started it.");
    let agent = json!("toolu_bg");
    assistant(
        agent.clone(),
        json!([{ "type": "tool_use", "id": "b1", "name": "Bash", "input": { "command": "ls" } }]),
    );
    tool_result(agent.clone(), "b1", json!("a.txt"), json!({}));
    assistant(agent, json!([{ "type": "text", "text": "Found a.txt." }]));
    // a background shell command ends the same way and is not a subagent
    notification("toolu_shell", "completed", "exit 0");
    notification("toolu_bg", "completed", "Found a.txt.");
    emit(json!({
        "type": "system", "subtype": "init", "model": "claude-fake",
        "session_id": "fake-session", "tools": ["Bash"],
    }));
    say("It found a.txt.");
    done("It found a.txt.");
}
