mod ai;
mod devtools;
mod license;
mod oauth;
mod persist;
mod pty;

use std::collections::HashMap;

use persist::Store;
use pty::PtyManager;
use tauri::ipc::{Channel, InvokeResponseBody};
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
) -> Result<(), String> {
    state.create(id, cwd, shell, args, env, cols, rows, on_event)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(PtyManager::default())
        .manage(Store::new())
        .invoke_handler(tauri::generate_handler![
            create_pty,
            write_pty,
            resize_pty,
            kill_pty,
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
            devtools::git_changes,
            devtools::git_diff,
            devtools::detect_run_cmd,
            devtools::worktree_create,
            devtools::worktree_remove,
            devtools::worktree_list,
            ai::ai_name_space,
            oauth::oauth_listen
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
