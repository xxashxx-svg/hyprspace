mod agent;
mod ai;
mod devtools;
mod license;
mod loophook;
mod oauth;
mod persist;
mod pty;

use std::collections::HashMap;

use agent::AgentManager;
use persist::Store;
use pty::PtyManager;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::Manager;
use tauri::State;

// async + spawn_blocking: ConPTY open + shell spawn are blocking syscalls (tens of ms each), and
// the launcher fans out N of these in one commit — serialized on the UI thread they froze the window
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn create_pty(
    state: State<'_, PtyManager>,
    id: String,
    cwd: String,
    shell: Option<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
    cols: u16,
    rows: u16,
    on_event: Channel<InvokeResponseBody>,
    auto_respond: bool,
) -> Result<(), String> {
    let mgr = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        mgr.create(id, cwd, shell, args, env, cols, rows, on_event, auto_respond)
    })
    .await
    .map_err(|e| e.to_string())?
}

// async + spawn_blocking so a write that blocks (child not draining stdin) never stalls the UI
// thread. input crosses IPC as base64 (a JSON number-array was ~4-5 bytes per input byte — a
// big paste built a multi-MB JSON string on the UI thread).
#[tauri::command]
async fn write_pty(state: State<'_, PtyManager>, id: String, data: String) -> Result<(), String> {
    use base64::Engine;
    let mgr = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| e.to_string())?;
        mgr.write(&id, &bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}

// async + spawn_blocking: ResizePseudoConsole is cross-process ConPTY IPC that can stall; during a
// window drag every visible pane fires these at ~16/s
#[tauri::command]
async fn resize_pty(state: State<'_, PtyManager>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    let mgr = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || mgr.resize(&id, cols, rows))
        .await
        .map_err(|e| e.to_string())?
}

// xterm flow control (see pty.rs PauseGate): the frontend pauses the PTY reader when xterm's
// write buffer backs up, and resumes once it drains
#[tauri::command]
fn pause_pty(state: State<PtyManager>, id: String) {
    state.set_paused(&id, true);
}

#[tauri::command]
fn resume_pty(state: State<PtyManager>, id: String) {
    state.set_paused(&id, false);
}

#[tauri::command]
fn kill_pty(state: State<PtyManager>, id: String) -> Result<(), String> {
    state.kill(&id)
}


// agent_start also reads keychain secrets (an OS credential-manager RPC) — definitely not UI-thread work
#[tauri::command]
async fn agent_start(
    state: State<'_, AgentManager>,
    id: String,
    cwd: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    secrets: HashMap<String, String>,
    prompt: String,
    on_event: Channel<String>,
) -> Result<(), String> {
    let mgr = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || mgr.start(id, cwd, args, env, secrets, prompt, on_event))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn agent_stop(state: State<'_, AgentManager>, id: String) -> Result<(), String> {
    let mgr = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || mgr.stop(&id))
        .await
        .map_err(|e| e.to_string())
}

// ---- OS keychain: store loop-agent API keys (Windows Credential Manager / macOS Keychain) ----
const KEYCHAIN_SVC: &str = "hyprspace";

// keychain calls are out-of-process RPCs (Credential Manager / Keychain) that can stall on slow
// credential providers — spawn_blocking so a slow keychain never blocks the window
#[tauri::command]
async fn secret_set(name: String, value: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        keyring::Entry::new(KEYCHAIN_SVC, &name)
            .and_then(|e| e.set_password(&value))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn secret_has(name: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        keyring::Entry::new(KEYCHAIN_SVC, &name)
            .and_then(|e| e.get_password())
            .is_ok()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn secret_clear(name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = keyring::Entry::new(KEYCHAIN_SVC, &name).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default()
}

// Alt+V image paste: we read the clipboard image ourselves and drop it to a temp PNG, then the
// frontend types the path into the prompt. Doing the read here (not letting the agent CLI do it)
// dodges the Windows first-clipboard-read race that made the first paste silently miss. Returns
// the file path, or None when the clipboard holds no image.
#[tauri::command]
async fn clipboard_image_to_temp(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let (rgba, w, h) = match app.clipboard().read_image() {
        Ok(img) => (img.rgba().to_vec(), img.width(), img.height()),
        Err(_) => return Ok(None), // no image on the clipboard — caller falls back to a normal paste
    };
    // an empty clipboard image isn't an error, just nothing to paste; and cap the pixel count so a
    // giant copy (a 20000x20000 export = 1.6GB of rgba) can't OOM-abort the whole app mid-encode
    if w == 0 || h == 0 {
        return Ok(None);
    }
    if w as u64 * h as u64 > 40_000_000 {
        return Err("that image is too large to paste".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let buf = image::RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| "clipboard image had an unexpected size".to_string())?;
        let dir = std::env::temp_dir().join("hyprspace-clip");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // keep the dir from growing forever. 7 days, not hours: an open image tab (and an unsent
        // prompt) references the file by path, so deleting it out from under a still-open tab is
        // what makes an image go "couldn't open" later. only ever touch our own clip-*.png.
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let cutoff = std::time::SystemTime::now()
                .checked_sub(std::time::Duration::from_secs(7 * 24 * 3600));
            for e in rd.flatten() {
                let ours = e
                    .file_name()
                    .to_str()
                    .map(|n| n.starts_with("clip-") && n.ends_with(".png"))
                    .unwrap_or(false);
                if !ours {
                    continue;
                }
                if let (Some(cutoff), Ok(meta)) = (cutoff, e.metadata()) {
                    if meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = dir.join(format!("clip-{stamp}.png"));
        // fast PNG: the default encoder does adaptive per-scanline filtering + full deflate, which is
        // 0.5-2s on a screenshot-sized buffer. these are throwaway temp files the agent reads once, so
        // trade a bigger file for speed — Fast (fdeflate) + no filter search drops it to tens of ms.
        use image::ImageEncoder;
        let file = std::io::BufWriter::new(std::fs::File::create(&path).map_err(|e| e.to_string())?);
        image::codecs::png::PngEncoder::new_with_quality(
            file,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        )
        .write_image(buf.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
        // forward slashes: Windows + the agent CLIs both accept them, and they dodge escaping
        Ok(Some(path.to_string_lossy().replace('\\', "/")))
    })
    .await
    .map_err(|e| e.to_string())?
}

// friendly name of the shell we spawn by default (for the status bar)
#[tauri::command]
fn shell_name() -> String {
    if cfg!(windows) {
        return "powershell".to_string();
    }
    let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    sh.rsplit('/').next().unwrap_or("bash").to_string()
}

// ~/.claude/projects/<cwd with : / \ swapped for -> — where claude keeps a folder's transcripts
fn claude_project_dir(cwd: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        return None;
    }
    // claude encodes a cwd by replacing EVERY non-alphanumeric char with '-', not just separators.
    // verified against ~/.claude/projects: all 131 dirs are [A-Za-z0-9-] only, and
    // C:\Users\x\.hyprspace\... lands at C--Users-x--hyprspace-... (the dot becomes a dash too).
    // getting this wrong silently breaks resume for any path with a dot/space — i.e. every worktree.
    let enc: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(
        std::path::Path::new(&home)
            .join(".claude")
            .join("projects")
            .join(enc),
    )
}

// Resolve Claude Code's `[Image #N]` terminal marker to the cached image file. Claude stores pasted
// images at ~/.claude/image-cache/<session-id>/<N>.<ext>, numbered across the session. We find the
// pane cwd's newest session (its .jsonl stem IS the session id + the image-cache dir name), then look
// up <N>.* there. Returns the file path, or None if there's no such image yet.
#[tauri::command]
async fn claude_image_path(cwd: String, n: u32, session_id: Option<String>) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .filter(|h| !h.is_empty())?;
        let cache = std::path::Path::new(&home).join(".claude").join("image-cache");
        // the pane pins its own id as claude's --session-id, so that's the right cache dir. only
        // guess (newest transcript in the folder) when it isn't there — e.g. a session forked by
        // /clear, or a pane that was already running before this existed. guessing is what made
        // two panes in one folder open each other's images.
        let by_pane = session_id
            .as_deref()
            .map(|s| cache.join(s))
            .filter(|d| d.is_dir());
        let dir = match by_pane {
            Some(d) => d,
            None => {
                let proj = claude_project_dir(&cwd)?;
                let newest = std::fs::read_dir(&proj)
                    .ok()?
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"))
                    .filter_map(|e| {
                        let modified = e.metadata().ok()?.modified().ok()?;
                        let stem = e.path().file_stem()?.to_string_lossy().into_owned();
                        Some((modified, stem))
                    })
                    .max_by_key(|(m, _)| *m)
                    .map(|(_, stem)| stem)?;
                cache.join(newest)
            }
        };
        let target = n.to_string();
        for e in std::fs::read_dir(&dir).ok()?.flatten() {
            let p = e.path();
            if p.is_file() && p.file_stem().and_then(|s| s.to_str()) == Some(target.as_str()) {
                // skip a 0-byte file: claude may still be writing it, and caching that miss beats
                // handing the viewer a truncated png it can't decode
                if e.metadata().map(|m| m.len() == 0).unwrap_or(false) {
                    return None;
                }
                return Some(p.to_string_lossy().replace('\\', "/"));
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

fn folder_has_transcript(dir: &std::path::Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .any(|e| e.path().extension().map_or(false, |x| x == "jsonl")),
        Err(_) => false,
    }
}

// does claude have any saved conversation for this folder? (scans a dir that can hold a LOT
// of transcripts — same blocking-thread treatment as its siblings below)
#[tauri::command]
async fn claude_has_history(cwd: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        claude_project_dir(&cwd).map_or(false, |d| folder_has_transcript(&d))
    })
    .await
    .unwrap_or(false)
}

// How a restored claude pane should come back. claude stores each chat as <session-id>.jsonl,
// and our pane's id IS that session id, so we can check for this pane's own transcript:
//   "resume"   → claude --resume <id>   (its own chat exists — the normal path)
//   "continue" → claude --continue      (no transcript for this id, but the folder has other
//                history — covers panes created before we owned the session id)
//   "fresh"    → start clean
// These scan the claude history folder (which can hold a LOT of transcripts for a busy project),
// and sync Tauri commands run on the UI thread — so push the filesystem work onto a blocking
// thread. Run synchronously, a big folder would stall the whole window ("Not Responding").
#[tauri::command]
async fn claude_resume_mode(cwd: String, session_id: String) -> String {
    tauri::async_runtime::spawn_blocking(move || resume_mode_blocking(&cwd, &session_id))
        .await
        .unwrap_or_else(|_| "fresh".into())
}

fn resume_mode_blocking(cwd: &str, session_id: &str) -> String {
    let Some(dir) = claude_project_dir(cwd) else {
        return "fresh".into();
    };
    if dir.join(format!("{session_id}.jsonl")).exists() {
        "resume".into()
    } else if folder_has_transcript(&dir) {
        "continue".into()
    } else {
        "fresh".into()
    }
}

// A folder's conversations as (session_id, modified_ms) — newest first, capped, so a huge history
// folder can't bloat the IPC payload or stall the poll. Lets the UI follow which chat a pane is on.
#[tauri::command]
async fn claude_sessions(cwd: String) -> Vec<(String, u64)> {
    tauri::async_runtime::spawn_blocking(move || sessions_blocking(&cwd))
        .await
        .unwrap_or_default()
}

fn sessions_blocking(cwd: &str) -> Vec<(String, u64)> {
    let Some(dir) = claude_project_dir(cwd) else {
        return vec![];
    };
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "jsonl") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let mtime = e
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    out.push((stem.to_string(), mtime));
                }
            }
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1)); // newest first
    out.truncate(40); // the tracker only needs the latest; don't ship the whole history
    out
}

// persistence does real disk I/O incl. an fsync (1-50ms, worse on HDD/AV-scanned machines) and
// the chat blob can be several MB — never on the UI thread
#[tauri::command]
async fn save_state(store: State<'_, Store>, name: String, data: String) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.save(&name, &data))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn load_state(store: State<'_, Store>, name: String) -> Result<Option<String>, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.load(&name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn backup_state(store: State<'_, Store>, name: String) -> Result<(), String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.backup(&name))
        .await
        .map_err(|e| e.to_string())?
}

// macOS/Linux GUI launches (Finder/Dock) hand the app a minimal PATH with no Homebrew, npm
// globals, nvm, etc. — so `claude`/`gemini`/`codex` look "not installed" even when they're there,
// and the home chat + loops can't spawn them. Resolve the user's real login-shell PATH once and
// adopt it, so every child process we spawn inherits it. No-op on Windows.
#[cfg(not(windows))]
fn fix_path_env() {
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // -ilc so both the login file (.zprofile → brew) and the interactive rc (.zshrc → nvm) get
        // sourced. the markers fence our value off from any banner the rc files might print.
        let out = Command::new(&shell)
            .args(["-ilc", "printf '__HP__%s__HP__' \"$PATH\""])
            .output();
        let _ = tx.send(out);
    });
    // a slow or wedged rc file shouldn't stall launch — give up after a few seconds
    let out = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(o)) => o,
        _ => return,
    };
    let s = String::from_utf8_lossy(&out.stdout);
    let resolved = match (s.find("__HP__"), s.rfind("__HP__")) {
        (Some(a), Some(b)) if b > a => s[a + 6..b].trim(),
        _ => return,
    };
    if resolved.is_empty() {
        return;
    }
    // keep whatever we already had too, in case the shell PATH somehow drops a system dir
    let cur = std::env::var("PATH").unwrap_or_default();
    let mut seen: std::collections::HashSet<&str> = resolved.split(':').collect();
    let mut merged = resolved.to_string();
    for p in cur.split(':') {
        if !p.is_empty() && seen.insert(p) {
            merged.push(':');
            merged.push_str(p);
        }
    }
    std::env::set_var("PATH", merged);
}

#[cfg(windows)]
fn fix_path_env() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Stop-hook helper: when a "Claude (hooks)" loop fires its Stop hook, Claude re-invokes THIS
    // binary as `loop-hook <config>`. Handle it and exit before any Tauri/window setup.
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(String::as_str) == Some("loop-hook") {
        loophook::run_loop_hook(argv.get(2).cloned());
        return; // unreachable — run_loop_hook exits the process
    }
    if argv.get(1).map(String::as_str) == Some("loop-notify") {
        loophook::run_loop_notify(argv.get(2).cloned());
        return; // unreachable — run_loop_notify exits the process
    }
    if argv.get(1).map(String::as_str) == Some("loop-done") {
        loophook::run_loop_done(argv.get(2).cloned());
        return; // unreachable — run_loop_done exits the process
    }

    // adopt the user's real shell PATH so a GUI launch on macOS/Linux can find the provider CLIs
    fix_path_env();

    // dev builds share the installed app's identity, so both fight over the same WebView2
    // user-data folder — the second instance hangs windowless on the lock. give debug builds
    // their own stable profile (one extra sign-in, then it sticks) so dev + installed coexist.
    #[cfg(all(windows, debug_assertions))]
    if std::env::var("WEBVIEW2_USER_DATA_FOLDER").is_err() {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        if !home.is_empty() {
            let dir = std::path::Path::new(&home).join(".hyprspace").join("dev-webview");
            let _ = std::fs::create_dir_all(&dir);
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|_app| {
            // macOS 26 leaves the native traffic-light buttons hidden even with decorations + the
            // Overlay title-bar style, so the window ends up with no close/min/max controls. Re-assert
            // the style, then explicitly un-hide the three standard window buttons through AppKit.
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSWindow, NSWindowButton};
                use tauri::{Manager, TitleBarStyle};
                for win in _app.webview_windows().values() {
                    let _ = win.set_title_bar_style(TitleBarStyle::Overlay);
                    if let Ok(ptr) = win.ns_window() {
                        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
                        for b in [
                            NSWindowButton::CloseButton,
                            NSWindowButton::MiniaturizeButton,
                            NSWindowButton::ZoomButton,
                        ] {
                            if let Some(btn) = unsafe { ns.standardWindowButton(b) } {
                                btn.setHidden(false);
                            }
                        }
                    }
                }
            }
            Ok(())
        })
        .manage(PtyManager::default())
        .manage(AgentManager::default())
        .manage(Store::new())
        .invoke_handler(tauri::generate_handler![
            create_pty,
            write_pty,
            resize_pty,
            pause_pty,
            resume_pty,
            kill_pty,
            agent_start,
            agent_stop,
            clipboard_image_to_temp,
            secret_set,
            secret_has,
            secret_clear,
            get_home_dir,
            shell_name,
            claude_has_history,
            claude_resume_mode,
            claude_sessions,
            save_state,
            load_state,
            backup_state,
            license::activate_license,
            license::license_status,
            license::entitlement_verify,
            devtools::git_changes,
            devtools::git_diff,
            devtools::detect_run_cmd,
            devtools::run_check,
            devtools::git_commit,
            devtools::git_push,
            devtools::git_create_pr,
            devtools::git_pr_defaults,
            devtools::git_is_repo,
            devtools::git_init,
            devtools::git_init_repo,
            devtools::git_branch_info,
            devtools::git_file_op,
            devtools::create_project_dir,
            devtools::reveal_path,
            devtools::list_dir,
            devtools::read_file,
            devtools::read_image_file,
            devtools::path_exists,
            claude_image_path,
            devtools::write_file,
            devtools::file_op,
            devtools::find_files,
            devtools::provider_status,
            devtools::provider_usage_one,
            devtools::mcp_list,
            devtools::mcp_set,
            devtools::mcp_remove,
            devtools::list_skills,
            devtools::skill_read,
            devtools::skill_write,
            devtools::skill_delete,
            devtools::worktree_create,
            devtools::worktree_remove,
            devtools::worktree_list,
            ai::ai_name_space,
            oauth::oauth_listen,
            loophook::cleanup_hook_run,
            loophook::prepare_notify_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // On exit, kill every PTY session so ConPTY hosts (OpenConsole.exe) don't orphan and
            // busy-spin — the root cause of the recurring high-CPU / "Not Responding".
            match event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    app.state::<PtyManager>().kill_all();
                    app.state::<AgentManager>().kill_all();
                }
                _ => {}
            }
        });
}
