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
        let mut root = read_json(path.clone()).unwrap_or_else(|| serde_json::json!({}));
        if !root.is_object() {
            root = serde_json::json!({});
        }
        let obj = root.as_object_mut().unwrap();
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
        let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(())
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
        let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
