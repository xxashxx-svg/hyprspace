#![allow(unused_imports)]
use super::*;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use serde::Serialize;
use serde_json::Value;
use base64::Engine;

#[tauri::command]
pub async fn detect_run_cmd(cwd: String) -> String {
    tauri::async_runtime::spawn_blocking(move || detect_run_cmd_blocking(cwd))
        .await
        .unwrap_or_default()
}

fn detect_run_cmd_blocking(cwd: String) -> String {
    let p = Path::new(&cwd);
    if let Ok(s) = std::fs::read_to_string(p.join("package.json")) {
        if s.contains("\"dev\"") {
            return "npm run dev".into();
        }
        if s.contains("\"start\"") {
            return "npm start".into();
        }
    }
    if p.join("Cargo.toml").exists() {
        return "cargo run".into();
    }
    String::new()
}

// Run a one-shot shell command in `cwd` and return its exit code (-1 if it couldn't start).
// Used by the Loops "until check passes" guard — e.g. loop until `cargo build` exits 0.

#[tauri::command]
pub async fn run_check(cwd: String, command: String) -> i32 {
    tauri::async_runtime::spawn_blocking(move || run_check_blocking(&cwd, &command))
        .await
        .unwrap_or(-1)
}

fn run_check_blocking(cwd: &str, command: &str) -> i32 {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    if !cwd.is_empty() {
        cmd.current_dir(cwd);
    }
    cmd.env_remove("NoDefaultCurrentDirectoryInExePath"); // same fix as the agent/service spawns
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

// ---- git write ops for the topbar "Commit & push" menu ----

// push the current branch; if it has no upstream yet, set one to origin on first push

#[tauri::command]
pub async fn create_project_dir(
    path: String,
    readme: Option<String>,
    gitignore: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if path.trim().is_empty() {
            return Err("No folder path.".to_string());
        }
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        if let Some(body) = readme {
            let p = Path::new(&path).join("README.md");
            if !p.exists() {
                std::fs::write(&p, body).map_err(|e| e.to_string())?;
            }
        }
        if let Some(body) = gitignore {
            let p = Path::new(&path).join(".gitignore");
            if !p.exists() {
                std::fs::write(&p, body).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// open a folder in the OS file manager (Explorer / Finder / xdg-open)
