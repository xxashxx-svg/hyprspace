// Live agent status for terminal sessions, fed by Claude Code's own hooks. Copied from
// src-tauri/src/agenthook.rs.
//
// Claude can run a command on lifecycle events. We hand each claude terminal session a scoped
// `--settings` file whose hooks re-invoke THIS binary (`hyprspace agent-hook <port> <session>`);
// that short-lived process reads the hook payload from stdin and POSTs it to a loopback listener
// the app owns, so the sidebar learns about a state change the moment it happens.
//
// Why not the transcript: terminal sessions may run with transcript saving off, so sidechain
// records never land on disk. Why not marker files: that's poll-based, far too laggy for a dozen
// live sessions.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};

/// One message from a hook process. Both carry `{ "session": id, ... }`.
#[derive(Debug, Clone, PartialEq)]
pub enum Hook {
    /// A lifecycle hook: `{ session, payload }`.
    Agent(Value),
    /// Claude's per-turn status line blob with rate limits and context fill: `{ session, statusLine }`.
    StatusLine(Value),
}

/// Session ids are interpolated into a shell command and a file path, so anything outside
/// [A-Za-z0-9_-] is rejected: neither injection nor traversal is possible.
fn valid_session(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn hooks_dir() -> PathBuf {
    crate::util::home_dir()
        .join(".hyprspace")
        .join("agent-hooks")
}

/// Start the loopback listener and return its port. Port 0 lets the OS pick a free one, so
/// several app windows (or a stale instance) can't fight over a fixed port.
pub fn listen(on: impl Fn(Hook) + Send + 'static) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            // one short-lived connection per hook; handle inline, it's a few hundred bytes
            let Some(body) = read_request(&mut s) else {
                continue;
            };
            // always answer so the hook process exits promptly and never stalls claude's turn
            let _ =
                s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = s.flush();
            if let Ok(v) = serde_json::from_str::<Value>(&body) {
                // status-line payloads carry usage and arrive every turn; they'd swamp the hook log
                if v.get("statusLine").is_some() {
                    on(Hook::StatusLine(v));
                } else {
                    log_event(&body);
                    on(Hook::Agent(v));
                }
            }
        }
    });
    Ok(port)
}

/// Read one HTTP request and return its body. Bounded so a malformed request can't wedge the thread.
pub(crate) fn read_request(s: &mut TcpStream) -> Option<String> {
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 1_000_000 {
            return None;
        }
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
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
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    Some(String::from_utf8_lossy(&buf[head_end..]).to_string())
}

/// Append each received hook to ~/.hyprspace/agent-hooks/events.log (capped). Claude's hook
/// payloads aren't well documented across versions, so this is how we confirm which events fire.
///
/// Off unless HYPRSPACE_DEBUG_HOOKS=1: the payloads carry the user's prompts and claude's replies,
/// which have no business being written to a plaintext file on every turn by default.
fn log_event(body: &str) {
    if std::env::var("HYPRSPACE_DEBUG_HOOKS").as_deref() != Ok("1") {
        return;
    }
    let dir = hooks_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("events.log");
    if std::fs::metadata(&path)
        .map(|m| m.len() > 512_000)
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{body}");
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Write the scoped settings file that wires claude's hooks back to the listener on `port`, and
/// return its path for `claude --settings <path>`. `exe` is the binary that handles the
/// `agent-hook` and `status-line` subcommands. None means launch claude unhooked rather than fail.
pub fn write_settings(dir: &Path, port: u16, exe: &Path, session: &str) -> Option<PathBuf> {
    if !valid_session(session) {
        return None;
    }
    std::fs::create_dir_all(dir).ok()?;
    let path = dir.join(format!("{session}.json"));
    let exe = exe.to_string_lossy();

    // every hook runs the same command; the event name comes from the payload claude puts on stdin
    let cmd = format!("\"{exe}\" agent-hook {port} \"{session}\"");
    let entry = json!([{ "hooks": [ { "type": "command", "command": cmd } ] }]);
    let settings = json!({
        "hooks": {
            "UserPromptSubmit": entry,   // turn started: working
            "Stop": entry,               // turn ended: done
            // startup / resume / clear / compact. Without it a local slash command strands the row:
            // /clear submits a prompt (so we go working) but never produces an assistant turn, so no
            // Stop ever closes it and the thread counts up forever.
            "SessionStart": entry,
            "Notification": entry,       // a permission block (waiting on you) or an idle nudge (ignored)
            "SubagentStop": entry,       // a delegated agent finished
            // Every tool, not just delegations. It is what the activity line is made of ("Bash
            // cargo check"), it is how a delegation is spotted at all (there is no SubagentStart
            // hook, only the Agent/Task tool being called), and the pair of them is the only thing
            // that can end a permission block: approving one produces no hook of its own, so
            // without PostToolUse the row keeps saying "needs your permission" for the rest of the
            // turn while claude works away.
            "PreToolUse": entry,
            "PostToolUse": entry,
        },
        // claude hands the status line a per-turn blob with the account's rate-limit windows,
        // this session's context fill and its cost: the only local, token-free source for live
        // usage. We tee it and then run whatever status line the user already had (`delegate`).
        "statusLine": {
            "type": "command",
            "command": format!("\"{exe}\" status-line {port} \"{session}\""),
        },
    });
    std::fs::write(&path, serde_json::to_string_pretty(&settings).ok()?).ok()?;
    Some(path)
}

/// Drop a session's settings file when the session goes away.
/// A `claude` script that runs the real one with the session's hooks (`HYPRSPACE_CLAUDE_SETTINGS`)
/// unless the command names its own settings. It takes itself off PATH first, so it finds the
/// real `claude` and never itself. Returns the folder to put in front of PATH.
#[cfg(not(windows))]
pub fn claude_shim(dir: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    const SCRIPT: &str = r#"#!/bin/sh
# HyprSpace: claude with this terminal's hooks, so the sidebar follows a claude started by hand.
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PATH=$(printf '%s' "$PATH" | tr ':' '\n' | grep -vxF "$here" | paste -sd: -)
export PATH
for a in "$@"; do
  [ "$a" = "--settings" ] && exec claude "$@"
done
[ -n "$HYPRSPACE_CLAUDE_SETTINGS" ] && exec claude --settings "$HYPRSPACE_CLAUDE_SETTINGS" "$@"
exec claude "$@"
"#;
    let bin = dir.join("bin");
    let path = bin.join("claude");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(SCRIPT) {
        std::fs::create_dir_all(&bin).ok()?;
        std::fs::write(&path, SCRIPT).ok()?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).ok()?;
    }
    Some(bin)
}

pub fn cleanup(dir: &Path, session: &str) {
    if valid_session(session) {
        let _ = std::fs::remove_file(dir.join(format!("{session}.json")));
    }
}

/// Entry point for `hyprspace agent-hook <port> <session>`: read claude's payload from stdin, tag
/// it with its session, and hand it to the running app. Best-effort and silent: a hook that
/// errors must never interrupt the user's turn.
pub fn run_agent_hook(port: u16, session: &str) {
    if !valid_session(session) {
        return;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let payload: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({ "raw": input }));
    post(
        port,
        &json!({ "session": session, "payload": payload }).to_string(),
    );
}

/// Entry point for `hyprspace status-line <port> <session>`: tee claude's status-line payload to
/// the app, then print whatever the user's own status line would have printed, so theirs looks
/// untouched.
pub fn run_status_line(port: u16, session: &str) {
    if !valid_session(session) {
        return;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let Ok(v) = serde_json::from_str::<Value>(&input) {
        post(
            port,
            &json!({ "session": session, "statusLine": v }).to_string(),
        );
    }
    if let Some(out) = delegate(&crate::util::home_dir(), &input) {
        print!("{out}");
        let _ = std::io::stdout().flush();
    }
}

/// Hand one JSON body to the app's loopback listener. Best-effort: never blocks a turn for long.
fn post(port: u16, body: &str) {
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    // connect_timeout, not connect: if the app isn't running (closed, restarted, stale port)
    // Windows takes about 2s to refuse a dead loopback port, and this runs inside claude's status
    // line render
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    if let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) {
        let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
        let _ = s.write_all(req.as_bytes());
        let _ = s.flush();
    }
}

/// The status line command the user configured in ~/.claude/settings.json, if any.
fn user_status_line(home: &Path) -> Option<String> {
    let v = crate::util::read_json(&home.join(".claude").join("settings.json"))?;
    let sl = v.get("statusLine")?;
    if sl.get("type").and_then(Value::as_str) != Some("command") {
        return None;
    }
    let cmd = sl.get("command").and_then(Value::as_str)?;
    // belt and braces: never re-enter ourselves if our own command ever lands in the user's settings
    if cmd.is_empty() || cmd.contains("status-line") {
        return None;
    }
    Some(cmd.to_string())
}

/// Run the user's status line, feeding it the same payload. None when they have none: then we
/// print nothing and they simply get no status line, which is what they had before.
fn delegate(home: &Path, input: &str) -> Option<String> {
    let cmd = user_status_line(home)?;

    #[cfg(windows)]
    let mut c = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        // raw_arg, not arg: the command already carries its own quotes ( node "C:\...\x.js" ) and
        // rust would escape them into something cmd.exe doesn't understand, yielding no output.
        c.raw_arg("/c").raw_arg(&cmd);
        crate::util::no_window(&mut c);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(&cmd);
        c
    };
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = c.spawn().ok()?;
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn session_ids_are_tokens() {
        assert!(valid_session("a1-b_2"));
        assert!(!valid_session(""));
        assert!(!valid_session("../x"));
        assert!(!valid_session("a\" & calc"));
        assert!(!valid_session(&"a".repeat(65)));
    }

    #[test]
    fn listener_routes_hooks_and_status_lines() {
        let (tx, rx) = mpsc::channel();
        let port = listen(move |h| {
            let _ = tx.send(h);
        })
        .unwrap();
        post(
            port,
            r#"{"session":"s1","payload":{"hook_event_name":"Stop"}}"#,
        );
        post(port, r#"{"session":"s1","statusLine":{"cost":1}}"#);
        let a = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let b = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(&a, Hook::Agent(v) if v["payload"]["hook_event_name"] == "Stop"));
        assert!(matches!(&b, Hook::StatusLine(v) if v["statusLine"]["cost"] == 1));
    }

    #[test]
    fn settings_file_points_every_hook_at_us() {
        let dir = tempfile::tempdir().unwrap();
        let exe = Path::new("/apps/hyprspace");
        let path = write_settings(dir.path(), 4242, exe, "s1").unwrap();
        let v = crate::util::read_json(&path).unwrap();
        for hook in [
            "UserPromptSubmit",
            "Stop",
            "SessionStart",
            "Notification",
            "SubagentStop",
            "PreToolUse",
            "PostToolUse",
        ] {
            let cmd = v["hooks"][hook][0]["hooks"][0]["command"].as_str().unwrap();
            assert!(cmd.ends_with("agent-hook 4242 \"s1\""), "{hook}: {cmd}");
        }
        assert!(
            v["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("status-line 4242")
        );
        assert!(write_settings(dir.path(), 1, exe, "../x").is_none());
        cleanup(dir.path(), "s1");
        assert!(!path.exists());
    }

    #[test]
    fn user_status_line_skips_our_own_command() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        let write = |cmd: &str| {
            let v = json!({ "statusLine": { "type": "command", "command": cmd } });
            std::fs::write(
                home.path().join(".claude").join("settings.json"),
                v.to_string(),
            )
            .unwrap();
        };
        write("node line.js");
        assert_eq!(
            user_status_line(home.path()).as_deref(),
            Some("node line.js")
        );
        write("\"hyprspace\" status-line 1 \"x\"");
        assert_eq!(user_status_line(home.path()), None);
    }
}
