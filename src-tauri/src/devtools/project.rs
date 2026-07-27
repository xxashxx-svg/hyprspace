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
