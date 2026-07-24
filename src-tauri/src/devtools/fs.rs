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
        // precompute the sort key — to_lowercase inside the comparator allocated O(n log n) strings
        let mut keyed: Vec<(String, DirEntry)> =
            out.into_iter().map(|e| (e.name.to_lowercase(), e)).collect();
        keyed.sort_by(|(ka, a), (kb, b)| (b.dir as u8).cmp(&(a.dir as u8)).then_with(|| ka.cmp(kb)));
        Ok(keyed.into_iter().map(|(_, e)| e).collect())
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

// Does a path exist on disk? The terminal's image-path linkifier calls this (Orca-style) so a link
// only lights up for a file that's actually there. Cheap, so it can run on hover.
#[tauri::command]
pub async fn path_exists(path: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || Path::new(&path).exists())
        .await
        .unwrap_or(false)
}

// Read an image file for the in-app image viewer as a data URL. Caps at 25MB; picks a mime from
// the extension; base64-encodes the bytes so binary crosses IPC safely.

#[tauri::command]
pub async fn read_image_file(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if meta.len() > 25_000_000 {
            return Err("image is too large to open (>25MB)".into());
        }
        let mime = match Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("bmp") => "image/bmp",
            Some("svg") => "image/svg+xml",
            _ => "application/octet-stream",
        };
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{mime};base64,{b64}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

// Recursive filename search for the Files panel's "Find files" box. Case-insensitive substring
// match on the name; skips heavy build/vcs dirs; capped on results AND visited dirs so a giant
// repo can't wedge the walk. Returns paths relative to root, files only.

#[tauri::command]
pub async fn find_files(root: String, query: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Ok(vec![]);
        }
        const MAX_HITS: usize = 100;
        const MAX_DIRS: usize = 4000;
        let skip = [
            "node_modules", ".git", "target", "dist", "build", ".next", "out", ".venv", "__pycache__",
        ];
        let mut hits = vec![];
        let mut visited = 0usize;
        let mut stack = vec![std::path::PathBuf::from(&root)];
        while let Some(dir) = stack.pop() {
            visited += 1;
            if hits.len() >= MAX_HITS || visited > MAX_DIRS {
                break;
            }
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if hits.len() >= MAX_HITS {
                    break;
                }
                let name = e.file_name().to_string_lossy().to_string();
                let p = e.path();
                // file_type() is free from the dir entry; is_dir() was an extra stat per entry
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if !skip.contains(&name.as_str()) {
                        stack.push(p);
                    }
                } else if name.to_lowercase().contains(&q) {
                    if let Ok(rel) = p.strip_prefix(&root) {
                        hits.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(hits)
    })
    .await
    .map_err(|e| e.to_string())?
}

// File ops for the Files panel's context menu (new file/folder, rename, delete). Rename/create
// refuse to clobber an existing target; delete is recursive for folders (the UI confirms first).

#[tauri::command]
pub async fn file_op(op: String, path: String, to: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        match op.as_str() {
            "create-file" => {
                if p.exists() {
                    return Err("something with that name already exists".into());
                }
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(p, "").map_err(|e| e.to_string())
            }
            "create-dir" => {
                if p.exists() {
                    return Err("something with that name already exists".into());
                }
                std::fs::create_dir_all(p).map_err(|e| e.to_string())
            }
            "rename" => {
                let to = to.ok_or("missing rename target")?;
                let tp = std::path::Path::new(&to);
                if tp.exists() {
                    return Err("something with that name already exists".into());
                }
                std::fs::rename(p, tp).map_err(|e| e.to_string())
            }
            "delete" => {
                if p.is_dir() {
                    std::fs::remove_dir_all(p).map_err(|e| e.to_string())
                } else {
                    std::fs::remove_file(p).map_err(|e| e.to_string())
                }
            }
            other => Err(format!("unknown file op: {other}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- provider status (version + signed-in account/plan) for Settings → Providers ----
