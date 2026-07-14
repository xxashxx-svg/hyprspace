// Subscription-loop support for the Loops "Claude (hooks)" backend. The loop runs as ONE
// `claude -p` session on the user's subscription; a Stop hook (this same binary, invoked as
// `hyprspace-tauri loop-hook <config>`) fires after each turn and decides continue-vs-stop, so the
// session self-loops until a stop condition is met. We hand Claude the hook via a scoped settings
// file (`--settings`). (No `--bare`: that flag disables the subscription OAuth login.)
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn loops_tmp_dir(run_id: &str) -> PathBuf {
    // run_id is like "loop:<uuid>" — keep it a safe single path token
    let safe: String = run_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    std::env::temp_dir().join("hyprspace-loops").join(if safe.is_empty() { "_".into() } else { safe })
}

#[derive(Serialize)]
#[allow(dead_code)] // deregistered command; kept for the binary's loop-hook entry point + history
pub struct HookFiles {
    settings: String, // pass to `claude --settings <path>`
    counter: String, // the engine polls this for the live iteration count
    done: String, // the hook writes the stop reason here when done; the engine polls it to tear down
    output: String, // the hook writes Claude's real responses (JSON array) here; the engine shows them
}

// Build the scoped settings file (+ its sidecar config + counter) for one hook-driven run and return
// the file paths. The loop engine passes `settings` to `claude --settings <path>`.
#[tauri::command]
#[allow(dead_code)] // no longer registered (claude-hooks backend retired); left intact intentionally
pub fn prepare_hook_settings(
    run_id: String,
    max_iterations: u32,
    until_check: Option<String>,
    sentinel: Option<String>,
    cwd: String,
    reason: String,
) -> Result<HookFiles, String> {
    let dir = loops_tmp_dir(&run_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let counter = dir.join("counter");
    let config = dir.join("config.json");
    let settings = dir.join("settings.json");
    let done = dir.join("done");
    let output = dir.join("output.json");

    let _ = fs::write(&counter, "0"); // fresh count for this run
    let _ = fs::remove_file(&done); // no stale "done" from a prior run
    let _ = fs::write(&output, "[]"); // fresh transcript snapshot

    let cfg = json!({
        "counter": counter.to_string_lossy(),
        "done": done.to_string_lossy(),
        "output": output.to_string_lossy(),
        "max": max_iterations,
        "check": until_check,
        "sentinel": sentinel,
        "cwd": cwd,
        "reason": reason,
    });
    fs::write(&config, serde_json::to_string(&cfg).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    // the Stop hook just re-invokes THIS binary; serde escapes the backslashes/quotes for the JSON.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let hook_cmd = format!(
        "\"{}\" loop-hook \"{}\"",
        exe.to_string_lossy(),
        config.to_string_lossy()
    );
    let stg = json!({
        "hooks": { "Stop": [ { "matcher": "*", "hooks": [ { "type": "command", "command": hook_cmd } ] } ] }
    });
    fs::write(&settings, serde_json::to_string_pretty(&stg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    Ok(HookFiles {
        settings: settings.to_string_lossy().to_string(),
        counter: counter.to_string_lossy().to_string(),
        done: done.to_string_lossy().to_string(),
        output: output.to_string_lossy().to_string(),
    })
}

// Pull Claude's real responses out of a transcript JSONL, in order. Each line is one JSON object;
// assistant turns have message.role == "assistant" and a content[] of typed blocks — we keep the
// "text" blocks (the actual answer) and skip thinking/tool_use. Tolerant: bad lines are skipped.
fn extract_assistant_texts(transcript_path: &str) -> Vec<String> {
    let Ok(text) = fs::read_to_string(transcript_path) else {
        return vec![];
    };
    let mut out = vec![];
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(msg) = v.get("message") else { continue };
        if msg.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            continue;
        }
        let Some(blocks) = msg.get("content").and_then(|c| c.as_array()) else {
            continue;
        };
        let mut buf = String::new();
        for b in blocks {
            if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(tx) = b.get("text").and_then(|t| t.as_str()) {
                    if !buf.is_empty() {
                        buf.push('\n');
                    }
                    buf.push_str(tx);
                }
            }
        }
        let trimmed = buf.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    out
}

// best-effort cleanup of a run's temp files when the loop stops
#[tauri::command]
pub fn cleanup_hook_run(run_id: String) {
    let _ = fs::remove_dir_all(loops_tmp_dir(&run_id));
}

#[derive(Serialize)]
pub struct NotifyFiles {
    settings: String, // pass to `claude --settings <path>`
    marker: String, // the hook appends a line per notification; the engine polls this to ping the user
    done: String, // the Stop hook writes here when Claude's turn ends; the engine polls it to finish the run
}

// Build a settings file with a Notification hook for an INTERACTIVE-terminal loop. Claude fires the
// Notification hook when it needs the user (a permission request, or input idle), so we re-invoke this
// binary as `loop-notify <marker>` to record the message; the loop engine polls the marker and raises
// a HyprSpace notification so the user knows to go answer the terminal.
#[tauri::command]
pub fn prepare_notify_settings(run_id: String) -> Result<NotifyFiles, String> {
    let dir = loops_tmp_dir(&run_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let settings = dir.join("notify-settings.json");
    let marker = dir.join("notify");
    let done = dir.join("done");
    let _ = fs::remove_file(&marker); // no stale notifications from a prior run
    let _ = fs::remove_file(&done); // no stale "done" from a prior run

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    // Notification hook → "needs you" pings; Stop hook → the turn ended, so the engine can finish the
    // run (interactive Claude goes idle instead of exiting, so a Stop marker is how we learn it's done).
    let notify_cmd = format!("\"{}\" loop-notify \"{}\"", exe, marker.to_string_lossy());
    let done_cmd = format!("\"{}\" loop-done \"{}\"", exe, done.to_string_lossy());
    let stg = json!({
        "hooks": {
            "Notification": [ { "hooks": [ { "type": "command", "command": notify_cmd } ] } ],
            "Stop": [ { "matcher": "*", "hooks": [ { "type": "command", "command": done_cmd } ] } ]
        }
    });
    fs::write(&settings, serde_json::to_string_pretty(&stg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    Ok(NotifyFiles {
        settings: settings.to_string_lossy().to_string(),
        marker: marker.to_string_lossy().to_string(),
        done: done.to_string_lossy().to_string(),
    })
}

// Notification-hook entry point (binary invoked as `loop-notify <marker>`). Reads the hook's stdin
// JSON for the `message` and appends it as a line to the marker file the engine polls.
pub fn run_loop_notify(marker_path: Option<String>) {
    if let Some(marker) = marker_path {
        let mut input = String::new();
        let _ = std::io::stdin().read_to_string(&mut input);
        let v: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));
        let msg = v
            .get("message")
            .and_then(|m| m.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Claude needs your input");
        use std::io::Write as _;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&marker) {
            let _ = writeln!(f, "{}", msg.replace(['\n', '\r'], " "));
        }
    }
    std::process::exit(0);
}

// Stop-hook entry point (binary invoked as `loop-done <marker>`). Interactive Claude goes idle rather
// than exiting when it finishes, so its Stop hook writes this marker to tell the engine the run is done.
pub fn run_loop_done(marker_path: Option<String>) {
    if let Some(marker) = marker_path {
        let _ = fs::write(&marker, "done");
    }
    std::process::exit(0);
}

fn run_shell(cmd: &str, cwd: &str) -> i32 {
    let mut c = if cfg!(windows) {
        let mut x = Command::new("cmd");
        x.arg("/c").arg(cmd);
        x
    } else {
        let mut x = Command::new("sh");
        x.arg("-c").arg(cmd);
        x
    };
    if !cwd.is_empty() {
        c.current_dir(cwd);
    }
    #[cfg(windows)]
    c.creation_flags(0x08000000); // CREATE_NO_WINDOW
    match c.status() {
        Ok(s) => s.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

// The Stop-hook entry point (binary invoked as `loop-hook <config.json>`). Reads the hook's stdin
// JSON (for transcript_path), bumps the iteration counter, and decides: allow stop (exit 0, no
// output) or keep going (print {"decision":"block","reason":...}). The max-iterations cap here is
// the hard backstop — a hook loop can never run forever.
pub fn run_loop_hook(config_path: Option<String>) {
    std::process::exit(decide(config_path));
}

// write the "done" marker (with the stop reason) and allow Claude to stop. Interactive Claude won't
// exit — it goes idle — so this marker is how the engine learns the loop finished and tears down.
fn allow_stop(cfg: &Value, reason: &str) -> i32 {
    if let Some(done) = cfg.get("done").and_then(|v| v.as_str()) {
        let _ = fs::write(done, reason);
    }
    0
}

fn decide(config_path: Option<String>) -> i32 {
    let Some(cfg_path) = config_path else { return 0 }; // no config → let Claude stop
    let Ok(text) = fs::read_to_string(&cfg_path) else { return 0 };
    let Ok(cfg) = serde_json::from_str::<Value>(&text) else { return 0 };

    // the Stop hook gets a JSON event on stdin; we only need transcript_path (for the sentinel scan)
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let stdin_json: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));
    let transcript = stdin_json.get("transcript_path").and_then(|v| v.as_str());

    // snapshot Claude's real responses for the engine (this is what the Runs tab shows). Do it before
    // the max-iteration return so the final turn's answer is captured too.
    if let (Some(tp), Some(out)) = (transcript, cfg.get("output").and_then(|v| v.as_str())) {
        let facts = extract_assistant_texts(tp);
        let _ = fs::write(out, serde_json::to_string(&facts).unwrap_or_else(|_| "[]".into()));
    }

    // hard cap: bump the counter, stop once we hit max iterations
    let counter = cfg.get("counter").and_then(|v| v.as_str()).unwrap_or("");
    let n = fs::read_to_string(counter).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0) + 1;
    if !counter.is_empty() {
        let _ = fs::write(counter, n.to_string());
    }
    let max = cfg.get("max").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
    if n >= max {
        return allow_stop(&cfg, &format!("reached {max} iterations"));
    }

    let cwd = cfg.get("cwd").and_then(|v| v.as_str()).unwrap_or("");

    // until-check: the loop is done once this command exits 0 (e.g. the tests finally pass)
    if let Some(check) = cfg.get("check").and_then(|v| v.as_str()) {
        if !check.trim().is_empty() && run_shell(check, cwd) == 0 {
            return allow_stop(&cfg, "check passed");
        }
    }

    // sentinel: stop once the token shows up in the transcript
    if let Some(tok) = cfg.get("sentinel").and_then(|v| v.as_str()) {
        if !tok.is_empty() {
            if let Some(tp) = transcript {
                if fs::read_to_string(tp).map(|s| s.contains(tok)).unwrap_or(false) {
                    return allow_stop(&cfg, "sentinel reached");
                }
            }
        }
    }

    // not done → block the stop and feed Claude a nudge to keep working
    let reason = cfg
        .get("reason")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Keep working on the task — it isn't finished yet.");
    println!("{}", json!({ "decision": "block", "reason": reason }));
    0
}
