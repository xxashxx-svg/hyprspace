// Fake `codex app-server`: JSON-RPC over stdio, shapes from codex-cli 0.159.3's generated
// protocol types.

use serde_json::{Value, json};

use crate::{emit, ignore_everything, read};

fn reply(id: &Value, result: Value) {
    emit(json!({ "id": id, "result": result }));
}

fn notify(method: &str, params: Value) {
    emit(json!({ "method": method, "params": params }));
}

struct Fake {
    thread: String,
    turns: u32,
}

impl Fake {
    fn say(&self, item: &str, text: &str) {
        notify(
            "item/agentMessage/delta",
            json!({ "threadId": self.thread, "turnId": "t", "itemId": item, "delta": text }),
        );
    }

    fn start_turn(&mut self, id: &Value) -> String {
        self.turns += 1;
        let turn = format!("t-{}", self.turns);
        reply(
            id,
            json!({ "turn": { "id": turn, "items": [], "status": "inProgress" } }),
        );
        notify(
            "turn/started",
            json!({ "threadId": self.thread, "turn": { "id": turn } }),
        );
        turn
    }

    fn complete(&self, turn: &str, status: &str, error: Value) {
        notify(
            "turn/completed",
            json!({ "threadId": self.thread,
                    "turn": { "id": turn, "status": status, "error": error } }),
        );
    }

    fn item(&self, phase: &str, item: Value) {
        notify(phase, json!({ "threadId": self.thread, "item": item }));
    }
}

fn text_of(params: &Value) -> String {
    params["input"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["text"].as_str())
        .collect()
}

pub fn run() {
    let hello = read().unwrap_or_default();
    let named = hello["params"]["clientInfo"]["name"] == "hyprspace";
    let experimental = hello["params"]["capabilities"]["experimentalApi"] == true;
    if hello["method"] != "initialize" || !named || !experimental {
        eprintln!("bad initialize: {hello}");
        std::process::exit(2);
    }
    reply(&hello["id"], json!({ "userAgent": "fake-codex" }));
    if read().unwrap_or_default()["method"] != "initialized" {
        std::process::exit(2);
    }
    let open = read().unwrap_or_default();
    let params = open["params"].clone();
    let thread = match open["method"].as_str() {
        Some("thread/start") => "th-1".to_string(),
        Some("thread/resume") if params["threadId"] == "gone" => {
            emit(json!({ "id": open["id"],
                "error": { "code": -32600, "message": "no rollout found for thread id gone" } }));
            ignore_everything();
        }
        Some("thread/resume") => params["threadId"].as_str().unwrap_or_default().to_string(),
        _ => std::process::exit(2),
    };
    let model = params["model"].as_str().unwrap_or("gpt-fake");
    let cwd = params["cwd"].as_str().unwrap_or("/resumed");
    reply(
        &open["id"],
        json!({ "thread": { "id": thread }, "model": model, "cwd": cwd }),
    );
    let mut fake = Fake { thread, turns: 0 };
    while let Some(msg) = read() {
        if msg["method"] != "turn/start" {
            continue;
        }
        let text = text_of(&msg["params"]);
        let turn = fake.start_turn(&msg["id"]);
        if text.contains("args") {
            let seen = json!({ "thread": params, "turn": msg["params"] });
            fake.say("m1", &seen.to_string());
            fake.complete(&turn, "completed", Value::Null);
        } else if text.contains("hello") {
            hello_turn(&fake, &turn);
        } else if text.contains("approve") {
            approve(&fake, &turn);
        } else if text.contains("late-steer") {
            late_steer(&mut fake, &turn);
        } else if text.contains("steer") {
            fake.say("m1", "first");
            let steer = read().unwrap_or_default();
            let expected = steer["params"]["expectedTurnId"] == turn.as_str();
            reply(&steer["id"], json!({ "turnId": turn }));
            let said = format!(
                "steered({}): {}",
                if expected { "same turn" } else { "wrong turn" },
                text_of(&steer["params"])
            );
            fake.say("m1", &said);
            fake.complete(&turn, "completed", Value::Null);
        } else if text.contains("wedge") {
            fake.say("m1", "working");
            ignore_everything();
        } else if text.contains("interrupt") {
            fake.say("m1", "working");
            let stop = read().unwrap_or_default();
            if stop["method"] != "turn/interrupt" || stop["params"]["turnId"] != turn.as_str() {
                fake.say("m1", "expected turn/interrupt");
            }
            reply(&stop["id"], json!({}));
            fake.complete(&turn, "interrupted", Value::Null);
        } else if text.contains("image") {
            let paths: Vec<&str> = msg["params"]["input"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|i| i["type"] == "localImage")
                .filter_map(|i| i["path"].as_str())
                .collect();
            fake.say("m1", &format!("images: {}", paths.join(",")));
            fake.complete(&turn, "completed", Value::Null);
        } else if text.contains("crash") {
            fake.say("m1", "partial");
            eprintln!("fake codex crashed");
            std::process::exit(3);
        } else if text.contains("fail") {
            fake.complete(&turn, "failed", json!({ "message": "boom" }));
        } else {
            fake.say("m1", &format!("echo: {text}"));
            fake.complete(&turn, "completed", Value::Null);
        }
    }
}

fn hello_turn(fake: &Fake, turn: &str) {
    let th = &fake.thread;
    notify(
        "item/reasoning/summaryTextDelta",
        json!({ "threadId": th, "itemId": "r1", "summaryIndex": 0, "delta": "plan" }),
    );
    notify(
        "item/reasoning/summaryTextDelta",
        json!({ "threadId": th, "itemId": "r1", "summaryIndex": 1, "delta": "more" }),
    );
    fake.say("m1", "Hel");
    fake.say("m1", "lo");
    // a subagent's thread never reaches this one's transcript
    notify(
        "item/agentMessage/delta",
        json!({ "threadId": "child", "itemId": "c", "delta": "CHILD" }),
    );
    let cmd = json!({ "type": "commandExecution", "id": "c1", "command": "ls",
        "status": "inProgress", "aggregatedOutput": null, "exitCode": null });
    fake.item("item/started", cmd);
    fake.item(
        "item/completed",
        json!({ "type": "commandExecution", "id": "c1", "command": "ls",
            "status": "completed", "aggregatedOutput": "a.txt", "exitCode": 0 }),
    );
    let edit = json!({ "type": "fileChange", "id": "f1", "status": "completed", "changes": [
        { "path": "/a.rs", "kind": { "type": "update", "move_path": null },
          "diff": "@@ -1 +1 @@\n-a\n+b" }] });
    fake.item("item/started", edit.clone());
    fake.item("item/completed", edit);
    // a message that never streamed arrives whole
    fake.item(
        "item/completed",
        json!({ "type": "agentMessage", "id": "m2", "text": "Done." }),
    );
    notify(
        "thread/tokenUsage/updated",
        json!({ "threadId": th, "turnId": turn, "tokenUsage": {
            "last": { "inputTokens": 10, "outputTokens": 20, "totalTokens": 30 },
            "total": { "inputTokens": 10, "outputTokens": 20, "totalTokens": 30 },
            "modelContextWindow": 1000 } }),
    );
    notify(
        "error",
        json!({ "threadId": th, "willRetry": true, "error": { "message": "reconnecting" } }),
    );
    fake.complete(turn, "completed", Value::Null);
}

fn approve(fake: &Fake, turn: &str) {
    let mut notes = Vec::new();
    emit(
        json!({ "id": 100, "method": "item/commandExecution/requestApproval", "params": {
        "threadId": fake.thread, "turnId": turn, "itemId": "c1",
        "command": "rm x", "reason": "needs write access" } }),
    );
    let a1 = read().unwrap_or_default();
    if a1["id"] != 100 || a1["result"]["decision"] != "accept" {
        notes.push(format!("bad accept: {a1}"));
    }
    fake.item(
        "item/started",
        json!({ "type": "fileChange", "id": "f1", "status": "inProgress", "changes": [
            { "path": "/b.rs", "kind": { "type": "add" }, "diff": "+new" }] }),
    );
    emit(
        json!({ "id": "req-101", "method": "item/fileChange/requestApproval", "params": {
        "threadId": fake.thread, "turnId": turn, "itemId": "f1" } }),
    );
    let a2 = read().unwrap_or_default();
    if a2["id"] != "req-101" || a2["result"]["decision"] != "decline" {
        notes.push(format!("bad decline: {a2}"));
    }
    // a request the harness has no answer for still gets an error back
    emit(
        json!({ "id": 102, "method": "item/tool/requestUserInput", "params": {
        "threadId": fake.thread } }),
    );
    let a3 = read().unwrap_or_default();
    if a3["id"] != 102 || a3.get("error").is_none() {
        notes.push(format!("bad unknown-request answer: {a3}"));
    }
    let said = if notes.is_empty() {
        "approvals ok".to_string()
    } else {
        notes.join("; ")
    };
    fake.say("m1", &said);
    fake.complete(turn, "completed", Value::Null);
}

// The turn ends just as the steer arrives: the steer is refused, and the harness must start
// the next turn with it instead.
fn late_steer(fake: &mut Fake, turn: &str) {
    let steer = read().unwrap_or_default();
    emit(json!({ "id": steer["id"],
        "error": { "code": -32600, "message": "no active turn to steer" } }));
    fake.say("m1", "first");
    fake.complete(turn, "completed", Value::Null);
    let next = read().unwrap_or_default();
    if next["method"] != "turn/start" {
        fake.say("m2", "expected turn/start");
    }
    let turn = fake.start_turn(&next["id"]);
    fake.say("m2", &format!("late: {}", text_of(&next["params"])));
    fake.complete(&turn, "completed", Value::Null);
}
