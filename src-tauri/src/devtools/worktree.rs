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
pub struct Worktree {
    path: String,
    branch: String,
}

// Create an isolated worktree off the workspace repo so an agent can work without
// colliding with others. Worktrees live under ~/.hyprspace/worktrees/<repo>-<branch>
// (outside the repo, so they don't pollute its own git status).

#[tauri::command]
pub async fn worktree_create(cwd: String, name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || worktree_create_blocking(cwd, name))
        .await
        .map_err(|e| e.to_string())?
}

fn worktree_create_blocking(cwd: String, name: String) -> Result<String, String> {
    if cwd.is_empty() {
        return Err("no workspace folder".into());
    }
    let root = git(&cwd, &["rev-parse", "--show-toplevel"]).map_err(|_| "not a git repo")?;
    let root = root.trim();
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let branch = format!("hs/{}", safe.trim_matches('-'));

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let repo_name = Path::new(root)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    let wt = Path::new(&home)
        .join(".hyprspace")
        .join("worktrees")
        .join(format!("{repo_name}-{}", branch.replace('/', "-")));
    let wt = wt.to_string_lossy().to_string();

    // idempotent: a stable-named caller (e.g. a loop that re-runs) reuses its worktree instead of
    // erroring. existing dir → reuse as-is; existing branch but no dir → re-attach; else create new.
    if Path::new(&wt).exists() {
        return Ok(wt);
    }
    let branch_exists = git(&cwd, &["rev-parse", "--verify", &format!("refs/heads/{branch}")]).is_ok();
    if branch_exists {
        git(&cwd, &["worktree", "add", &wt, &branch])?;
    } else {
        git(&cwd, &["worktree", "add", "-b", &branch, &wt, "HEAD"])?;
    }
    Ok(wt)
}

#[tauri::command]
pub async fn worktree_remove(cwd: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        git(&cwd, &["worktree", "remove", "--force", &path]).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn worktree_list(cwd: String) -> Result<Vec<Worktree>, String> {
    tauri::async_runtime::spawn_blocking(move || worktree_list_blocking(cwd))
        .await
        .map_err(|e| e.to_string())
}

fn worktree_list_blocking(cwd: String) -> Vec<Worktree> {
    if cwd.is_empty() {
        return vec![];
    }
    let out = git(&cwd, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut res = vec![];
    let mut path = String::new();
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = p.to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            res.push(Worktree {
                path: path.clone(),
                branch: b.replace("refs/heads/", ""),
            });
        }
    }
    res
}

// Best-guess command to run the project (for the Run panel).
