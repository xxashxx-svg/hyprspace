mod agent;
mod ai;
mod chat;
mod devtools;
mod license;
mod loophook;
mod oauth;
mod persist;
mod pty;
mod services;

use std::collections::HashMap;

use agent::AgentManager;
use chat::ChatManager;
use services::ServiceManager;
use persist::Store;
use pty::PtyManager;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::Manager;
use tauri::State;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn create_pty(
    state: State<PtyManager>,
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
    state.create(id, cwd, shell, args, env, cols, rows, on_event, auto_respond)
}

#[tauri::command]
fn write_pty(state: State<PtyManager>, id: String, data: Vec<u8>) -> Result<(), String> {
    state.write(&id, &data)
}

#[tauri::command]
fn resize_pty(state: State<PtyManager>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    state.resize(&id, cols, rows)
}

#[tauri::command]
fn kill_pty(state: State<PtyManager>, id: String) -> Result<(), String> {
    state.kill(&id)
}

#[tauri::command]
fn chat_start(
    state: State<ChatManager>,
    id: String,
    cwd: String,
    args: Vec<String>,
    on_event: Channel<String>,
) -> Result<(), String> {
    state.start(id, cwd, args, on_event)
}

#[tauri::command]
fn chat_turn(state: State<ChatManager>, id: String, message: String) -> Result<(), String> {
    state.turn(&id, message)
}

#[tauri::command]
fn chat_stop(state: State<ChatManager>, id: String) {
    state.stop(&id);
}

#[tauri::command]
fn service_start(
    state: State<ServiceManager>,
    id: String,
    cwd: String,
    command: String,
    env: HashMap<String, String>,
    on_event: Channel<String>,
) -> Result<(), String> {
    state.start(id, cwd, command, env, on_event)
}

#[tauri::command]
fn service_stop(state: State<ServiceManager>, id: String) {
    state.stop(&id);
}

#[tauri::command]
fn agent_start(
    state: State<AgentManager>,
    id: String,
    cwd: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    secrets: HashMap<String, String>,
    prompt: String,
    on_event: Channel<String>,
) -> Result<(), String> {
    state.start(id, cwd, args, env, secrets, prompt, on_event)
}

#[tauri::command]
fn agent_stop(state: State<AgentManager>, id: String) {
    state.stop(&id);
}

// ---- OS keychain: store loop-agent API keys (Windows Credential Manager / macOS Keychain) ----
const KEYCHAIN_SVC: &str = "hyprspace";

#[tauri::command]
fn secret_set(name: String, value: String) -> Result<(), String> {
    keyring::Entry::new(KEYCHAIN_SVC, &name)
        .and_then(|e| e.set_password(&value))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn secret_has(name: String) -> bool {
    keyring::Entry::new(KEYCHAIN_SVC, &name)
        .and_then(|e| e.get_password())
        .is_ok()
}

#[tauri::command]
fn secret_clear(name: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SVC, &name).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn get_home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default()
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
    let enc: String = cwd
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' => '-',
            _ => c,
        })
        .collect();
    Some(
        std::path::Path::new(&home)
            .join(".claude")
            .join("projects")
            .join(enc),
    )
}

fn folder_has_transcript(dir: &std::path::Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .any(|e| e.path().extension().map_or(false, |x| x == "jsonl")),
        Err(_) => false,
    }
}

// does claude have any saved conversation for this folder?
#[tauri::command]
fn claude_has_history(cwd: String) -> bool {
    claude_project_dir(&cwd).map_or(false, |d| folder_has_transcript(&d))
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

#[tauri::command]
fn save_state(store: State<Store>, name: String, data: String) -> Result<(), String> {
    store.save(&name, &data)
}

#[tauri::command]
fn load_state(store: State<Store>, name: String) -> Result<Option<String>, String> {
    store.load(&name)
}

#[tauri::command]
fn backup_state(store: State<Store>, name: String) -> Result<(), String> {
    store.backup(&name)
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

    // adopt the user's real shell PATH so a GUI launch on macOS/Linux can find the provider CLIs
    fix_path_env();

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(PtyManager::default())
        .manage(ChatManager::default())
        .manage(ServiceManager::default())
        .manage(AgentManager::default())
        .manage(Store::new())
        .invoke_handler(tauri::generate_handler![
            create_pty,
            write_pty,
            resize_pty,
            kill_pty,
            chat_start,
            chat_turn,
            chat_stop,
            service_start,
            service_stop,
            agent_start,
            agent_stop,
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
            devtools::write_file,
            devtools::provider_status,
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
            loophook::prepare_hook_settings,
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
                    app.state::<ChatManager>().kill_all();
                    app.state::<ServiceManager>().kill_all();
                    app.state::<AgentManager>().kill_all();
                }
                _ => {}
            }
        });
}
