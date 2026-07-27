// Live agent status for terminal panes, fed by Claude Code's own hooks.
//
// Claude can run a command on lifecycle events. We hand each claude pane a scoped `--settings` file
// whose hooks re-invoke THIS binary (`hyprspace-tauri agent-hook <port> <paneId>`); that short-lived
// process reads the hook payload from stdin and POSTs it to a loopback listener the app owns. The
// listener emits a Tauri event, so the sidebar learns about a state change the moment it happens.
//
// Why not the transcript: pane sessions run with transcript saving off, so sidechain records never
// land on disk. Why not marker files (what the retired loophook.rs did): that's poll-based, which is
// fine for a single loop run and far too laggy for a dozen live panes.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

use serde_json::json;
use tauri::{AppHandle, Emitter};

static PORT: OnceLock<u16> = OnceLock::new();

/// pane ids are uuids we mint ourselves, but this value is interpolated into a shell command and a
/// file path — reject anything outside [A-Za-z0-9_-] so neither injection nor traversal is possible.
fn valid_pane_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn hooks_dir() -> PathBuf {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(base).join(".hyprspace").join("agent-hooks")
}

/// Start the loopback listener. Called once at startup; port 0 = let the OS pick a free one, so
/// several HyprSpace windows (or a stale instance) can't fight over a fixed port.
pub fn start(app: AppHandle) {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agent-hooks: couldn't open a loopback port: {e}");
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(_) => return,
    };
    let _ = PORT.set(port);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let app = app.clone();
            // one short-lived connection per hook — handle inline, it's a few hundred bytes
            if let Some(body) = read_request(&mut s) {
                // always answer so the hook process exits promptly and never stalls claude's turn
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = s.flush();
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    // status-line payloads carry usage and arrive every turn — a different event, and
                    // not worth logging (they'd swamp the hook log we keep for debugging).
                    if v.get("statusLine").is_some() {
                        let _ = app.emit("agent-usage", v);
                    } else {
                        log_event(&body);
                        let _ = app.emit("agent-hook", v);
                    }
                }
            }
        }
    });
}

/// Read one HTTP request and return its body. Bounded so a malformed request can't wedge the thread.
fn read_request(s: &mut TcpStream) -> Option<String> {
    s.set_read_timeout(Some(std::time::Duration::from_secs(3))).ok();
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    // headers first
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 1_000_000 {
            return None;
        }
        match s.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => return None,
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
    let len: usize = head
        .split("content-length:")
        .nth(1)
        .and_then(|s| s.split("\r\n").next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + len {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    Some(String::from_utf8_lossy(&buf[head_end..]).to_string())
}

/// Append each received hook to ~/.hyprspace/agent-hooks/events.log (capped). Claude's hook payloads
/// aren't well documented across versions, so this is how we confirm which events actually fire.
///
/// Off unless HYPRSPACE_DEBUG_HOOKS=1: the payloads carry the user's prompts and claude's replies,
/// which has no business being written to a plaintext file on every turn by default.
fn log_event(body: &str) {
    if std::env::var("HYPRSPACE_DEBUG_HOOKS").as_deref() != Ok("1") {
        return;
    }
    let dir = hooks_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("events.log");
    if std::fs::metadata(&path).map(|m| m.len() > 512_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{body}");
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Write (once per pane) the scoped settings file that wires claude's hooks back to us, and return
/// its path for `claude --settings <path>`. Returns None when the listener never came up, so the
/// caller can just launch claude unhooked rather than fail.
#[tauri::command]
pub fn agent_hook_settings(pane_id: String) -> Option<String> {
    if !valid_pane_id(&pane_id) {
        return None;
    }
    let port = *PORT.get()?;
    let exe = std::env::current_exe().ok()?.to_string_lossy().to_string();
    let dir = hooks_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{pane_id}.json"));

    // every hook runs the same command; the event name comes from the payload claude puts on stdin
    let cmd = format!("\"{exe}\" agent-hook {port} \"{pane_id}\"");
    let entry = json!([{ "hooks": [ { "type": "command", "command": cmd } ] }]);
    // A delegation is only observable as the sub-agent tool being called — there's no SubagentStart
    // hook. The tool is "Agent" on current claude and "Task" on older builds, so match either.
    let task = json!([{ "matcher": "Agent|Task", "hooks": [ { "type": "command", "command": cmd } ] }]);
    let settings = json!({
        "hooks": {
            "UserPromptSubmit": entry,   // turn started → working
            "Stop": entry,               // turn ended → done
            "Notification": entry,       // permission / idle prompt → waiting on you
            "SubagentStop": entry,       // a delegated agent finished
            "PreToolUse": task,          // Agent(...) → a sub-agent spawned
        },
        // claude hands the status line a per-turn blob with the account's rate-limit windows,
        // this pane's context fill and its cost — the only local, token-free source for live usage.
        // We tee it and then run whatever status line the user already had (see `delegate`).
        "statusLine": {
            "type": "command",
            "command": format!("\"{exe}\" status-line {port} \"{pane_id}\""),
        },
    });
    std::fs::write(&path, serde_json::to_string_pretty(&settings).ok()?).ok()?;
    Some(path.to_string_lossy().to_string())
}

/// Binary entry point for `agent-hook <port> <paneId>`: read claude's payload from stdin, tag it
/// with the pane it came from, and hand it to the running app. Best-effort and silent — a hook that
/// errors must never interrupt the user's turn.
pub fn run_agent_hook(port: u16, pane_id: &str) {
    if !valid_pane_id(pane_id) {
        return;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let payload: serde_json::Value =
        serde_json::from_str(&input).unwrap_or_else(|_| json!({ "raw": input }));
    post(port, &json!({ "paneId": pane_id, "payload": payload }).to_string());
}

/// Hand one JSON body to the app's loopback listener. Best-effort — never blocks a turn for long.
fn post(port: u16, body: &str) {
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    // connect_timeout, not connect: if the app isn't running (closed, restarted, stale port) windows
    // takes ~2s to refuse a dead loopback port, and this runs inside claude's status line render.
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if let Ok(mut s) = TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(250)) {
        let _ = s.set_write_timeout(Some(std::time::Duration::from_secs(2)));
        let _ = s.write_all(req.as_bytes());
        let _ = s.flush();
    }
}

/// Binary entry point for `status-line <port> <paneId>`: tee claude's status-line payload to the app,
/// then print whatever the user's own status line would have printed, so theirs looks untouched.
pub fn run_status_line(port: u16, pane_id: &str) {
    if !valid_pane_id(pane_id) {
        return;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) {
        post(port, &json!({ "paneId": pane_id, "statusLine": v }).to_string());
    }
    if let Some(out) = delegate(&input) {
        print!("{out}");
        let _ = std::io::stdout().flush();
    }
}

/// Run the status line the user configured in ~/.claude/settings.json, feeding it the same payload.
/// None when they have none — then we print nothing and they simply get no status line, which is
/// what they had before.
fn delegate(input: &str) -> Option<String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let raw = std::fs::read_to_string(PathBuf::from(home).join(".claude").join("settings.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let sl = v.get("statusLine")?;
    if sl.get("type").and_then(|t| t.as_str()) != Some("command") {
        return None;
    }
    let cmd = sl.get("command").and_then(|c| c.as_str())?;
    // belt and braces: never re-enter ourselves if our own command ever lands in the user's settings
    if cmd.is_empty() || cmd.contains("status-line") {
        return None;
    }

    #[cfg(windows)]
    let mut c = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        // raw_arg, not arg: the command already carries its own quotes ( node "C:\...\x.js" ) and
        // rust would escape them into something cmd.exe doesn't understand, yielding no output.
        c.raw_arg("/c").raw_arg(cmd);
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW — we're a GUI binary, don't flash a console
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    };
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Drop a pane's settings file when its pane goes away.
pub fn cleanup(pane_id: &str) {
    if !valid_pane_id(pane_id) {
        return;
    }
    let _ = std::fs::remove_file(hooks_dir().join(format!("{pane_id}.json")));
}
