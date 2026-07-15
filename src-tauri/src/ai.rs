// AI naming for open spaces. HyprSpace already runs the `claude` CLI in its panes, so it's
// on PATH and authenticated — we just call it headlessly (`claude -p`) to read recent terminal
// output and hand back a short title, like ChatGPT/Claude titling a conversation. No API key.
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// Async command: the work spawns a `claude` process and blocks on it, so we push it onto a
// blocking thread. Run synchronously this would stall Tauri's main thread and freeze the whole
// window while claude thinks — that's the "Not Responding" hang.
#[tauri::command]
pub async fn ai_name_space(context: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || name_blocking(context))
        .await
        .map_err(|e| e.to_string())?
}

fn name_blocking(context: String) -> Result<String, String> {
    let prompt = build_prompt(&context);
    // try the fast/cheap model first; if that errors (e.g. the alias is unknown on an older
    // CLI), fall back to claude's default model so naming still works.
    let res = match run_claude(&prompt, true) {
        Err(_) => run_claude(&prompt, false),
        ok => ok,
    };
    match res {
        Ok(Some(name)) => Ok(name),
        Ok(None) => Err("no usable name".into()),
        Err(e) => Err(e),
    }
}

// the frontend builds the full prompt (folder hint + filtered activity + instructions) so naming
// can be tuned without a backend rebuild; here we just bound its size for the stdin pipe.
fn build_prompt(context: &str) -> String {
    let chars: Vec<char> = context.chars().collect();
    if chars.len() > 12000 {
        chars[chars.len() - 12000..].iter().collect()
    } else {
        context.to_string()
    }
}

// Ok(Some) = a name, Ok(None) = ran fine but nothing usable (don't retry the model),
// Err = the process itself failed (missing binary / bad flag → worth a fallback attempt).
fn run_claude(prompt: &str, fast: bool) -> Result<Option<String>, String> {
    let mut cmd = base_cmd(fast);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("claude not found: {e}"))?;
    {
        // -p with no prompt arg reads the prompt from stdin; closing it (drop) signals EOF.
        // prompt stays well under the pipe buffer, so this won't deadlock against stdout.
        let mut sin = child.stdin.take().ok_or("no stdin")?;
        sin.write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
    }
    // drain both pipes on reader threads while we poll — a verbose stderr (auth error, update
    // banner) bigger than the pipe buffer would otherwise block claude and burn the whole timeout
    let drain = |r: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut buf);
            }
            buf
        })
    };
    use std::io::Read;
    let out_t = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err_t = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    // bound the wait so a wedged claude can't pin naming forever
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(st) => break st,
            None => {
                if start.elapsed() > Duration::from_secs(30) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("claude naming timed out".into());
                }
                std::thread::sleep(Duration::from_millis(80));
            }
        }
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    if !status.success() {
        return Err(String::from_utf8_lossy(&stderr).trim().to_string());
    }
    let name = sanitize(&String::from_utf8_lossy(&stdout));
    if name.is_empty() || name.eq_ignore_ascii_case("workspace") {
        return Ok(None); // model said it's unclear → keep the folder-name placeholder
    }
    Ok(Some(name))
}

fn base_cmd(fast: bool) -> Command {
    // on Windows `claude` is a .cmd shim, so go through cmd /C to resolve it off PATH
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg("claude");
        c
    } else {
        Command::new("claude")
    };
    c.arg("-p");
    if fast {
        c.arg("--model").arg("haiku");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW — no console flash
    }
    c
}

// pull the model's answer down to a clean tab title: first non-empty line, no wrapping
// quotes/markdown, no trailing punctuation, single-spaced, capped.
fn sanitize(raw: &str) -> String {
    let line = raw
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let line = line.trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '*' || c == '#');
    let line = line.trim().trim_end_matches(|c: char| ".,:;!".contains(c));
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(48).collect::<String>().trim().to_string()
}
