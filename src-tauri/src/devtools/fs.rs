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
pub async fn reveal_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if path.trim().is_empty() {
            return Err("No folder.".to_string());
        }
        // resolve to a real existing dir before launching anything. this is the security guard:
        // a "vscode://" / "http://" scheme or a "-flag" string won't canonicalize to a folder,
        // so the launcher never sees a URI to hand off or an option it could misread as a flag.
        let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
        if !canon.is_dir() {
            return Err("Not a folder.".to_string());
        }
        // strip Windows' \\?\ verbatim prefix so explorer opens the folder normally
        let s = canon.to_string_lossy();
        #[cfg(windows)]
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
        let s = s.to_string();
        if s.starts_with('-') {
            return Err("Invalid path.".to_string());
        }
        #[cfg(windows)]
        let mut cmd = Command::new("explorer");
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(all(unix, not(target_os = "macos")))]
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&s);
        // explorer returns a nonzero exit even on success, so just launch it and don't wait
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- file tree: list one directory level for the Files panel ----

#[derive(Serialize)]
pub struct DirEntry {
    name: String,
    dir: bool,
}

// list a single directory level (folders first, then files; case-insensitive alphabetical).
// lazy — the UI calls this per folder on expand, so a huge tree is never read all at once.

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let rd = std::fs::read_dir(&path).map_err(|e| e.to_string())?;
        let mut out: Vec<DirEntry> = vec![];
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            out.push(DirEntry { name, dir });
        }
        out.sort_by(|a, b| {
            (b.dir as u8)
                .cmp(&(a.dir as u8))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

// Read a text file for the editor. Caps at ~2MB and rejects binary (NUL-containing) files so the
// editor never tries to load a huge blob or a compiled artifact. Returns the UTF-8 text.

#[tauri::command]
pub async fn read_file(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if meta.len() > 2_000_000 {
            return Err("file is too large to open in the editor (>2MB)".into());
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.contains(&0) {
            return Err("looks like a binary file".into());
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Save the editor's contents back to disk.

#[tauri::command]
pub async fn write_file(path: String, content: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, content).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

// ---- provider status (version + signed-in account/plan) for Settings → Providers ----
