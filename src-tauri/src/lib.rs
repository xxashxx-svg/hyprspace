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

// does claude have any saved conversation for this folder? lets the UI pick --continue vs fresh
#[tauri::command]
fn claude_has_history(cwd: String) -> bool {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        return false;
    }
    // claude names a project's folder after its cwd with : / \ swapped for -
    let enc: String = cwd
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' => '-',
            _ => c,
        })
        .collect();
    let dir = std::path::Path::new(&home)
        .join(".claude")
        .join("projects")
        .join(enc);
    match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .any(|e| e.path().extension().map_or(false, |x| x == "jsonl")),
        Err(_) => false,
    }
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
            claude_has_history,
            save_state,
            load_state,
            backup_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
