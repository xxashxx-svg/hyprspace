// A scripted stand-in for the agent CLIs, spawned by the harness tests in place of `claude` and
// `codex app-server`. It plays one scenario per prompt, picked by a keyword in the prompt text,
// and checks what the harness writes back. Anything wrong comes back as reply text the test
// asserts on. Modeled on zeron's tests/fixtures/fake-claude.sh and fake-codex.sh (MIT, see
// THIRD_PARTY_NOTICES.md), in Rust so it runs on Windows too.

mod claude;
mod codex;

use std::io::{BufRead, Write};

use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "app-server") {
        codex::run();
    } else {
        claude::run(&args);
    }
}

fn emit(v: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

/// The next JSON line on stdin, or None at EOF.
fn read() -> Option<Value> {
    let mut line = String::new();
    loop {
        line.clear();
        if std::io::stdin().lock().read_line(&mut line).ok()? == 0 {
            return None;
        }
        if let Ok(v) = serde_json::from_str(line.trim()) {
            return Some(v);
        }
    }
}

/// A run the harness wedged on: keep reading so stdin never fills, answer nothing.
fn ignore_everything() -> ! {
    while read().is_some() {}
    std::process::exit(0)
}
