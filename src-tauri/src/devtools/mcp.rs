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

#[derive(Serialize)]
pub struct McpEntry {
    name: String,
    config: Value,
}

fn claude_json_path() -> std::path::PathBuf {
    home_dir().join(".claude.json")
}

// write JSON to `path` atomically (temp file in the same dir + rename) so a crash mid-write or a
// concurrent reader never sees a half-written ~/.claude.json.
fn write_json_atomic(path: &Path, root: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(root).map_err(|e| e.to_string())?;
    let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or(".claude.json");
    let tmp = path.with_file_name(format!("{fname}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

#[tauri::command]
pub async fn mcp_list() -> Vec<McpEntry> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = vec![];
        if let Some(v) = read_json(claude_json_path()) {
            if let Some(map) = v.get("mcpServers").and_then(|m| m.as_object()) {
                for (name, config) in map {
                    out.push(McpEntry {
                        name: name.clone(),
                        config: config.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    })
    .await
    .unwrap_or_default()
}

// upsert a server (preserving every other key in the file). prev_name handles a rename.

#[tauri::command]
pub async fn mcp_set(name: String, config: Value, prev_name: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if name.trim().is_empty() {
            return Err("Server needs a name.".to_string());
        }
        let path = claude_json_path();
        // Read carefully: a MISSING file is a real first run (start fresh), but an existing file we
        // can't read or parse must NOT be clobbered — ~/.claude.json holds the user's auth + every
        // project's history. serde is strict, so a concurrent half-write by a spawned `claude` won't
        // parse; bailing there beats wiping the file (mcp_remove already bails the same way).
        let mut root = match std::fs::read_to_string(&path) {
            Ok(text) => {
                let v: Value = serde_json::from_str(&text)
                    .map_err(|e| format!("~/.claude.json isn't valid JSON ({e}) — not overwriting it."))?;
                if !v.is_object() {
                    return Err("~/.claude.json isn't a JSON object — not overwriting it.".to_string());
                }
                v
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
            Err(e) => return Err(format!("Couldn't read ~/.claude.json: {e}")),
        };
        let obj = root.as_object_mut().unwrap(); // guaranteed an object by the checks above
        let servers = obj.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
        if !servers.is_object() {
            *servers = serde_json::json!({});
        }
        let smap = servers.as_object_mut().unwrap();
        if let Some(prev) = prev_name {
            if prev != name {
                smap.remove(&prev);
            }
        }
        smap.insert(name, config);
        write_json_atomic(&path, &root)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- skills: discover Claude skills/commands so the UI can list + drag them ----

#[tauri::command]
pub async fn mcp_remove(name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = claude_json_path();
        let Some(mut root) = read_json(path.clone()) else {
            return Ok(());
        };
        if let Some(smap) = root.get_mut("mcpServers").and_then(|m| m.as_object_mut()) {
            smap.remove(&name);
        }
        write_json_atomic(&path, &root)
    })
    .await
    .map_err(|e| e.to_string())?
}
