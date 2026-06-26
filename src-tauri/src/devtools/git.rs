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
pub struct FileChange {
    path: String,
    status: String, // porcelain code: M, A, D, R, ??, etc.
    added: u32,
    removed: u32,
}

#[tauri::command]
pub async fn git_changes(cwd: String) -> Result<Vec<FileChange>, String> {
    tauri::async_runtime::spawn_blocking(move || git_changes_blocking(cwd))
        .await
        .map_err(|e| e.to_string())?
}

fn git_changes_blocking(cwd: String) -> Result<Vec<FileChange>, String> {
    if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return Ok(vec![]); // not a repo → nothing to show, quietly
    }

    // line counts from staged + unstaged numstat, summed per path
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for args in [
        ["diff", "--numstat"].as_slice(),
        ["diff", "--numstat", "--cached"].as_slice(),
    ] {
        if let Ok(s) = git(&cwd, args) {
            for line in s.lines() {
                let mut p = line.split('\t');
                let a = p.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
                let r = p.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
                if let Some(path) = p.next() {
                    let e = counts.entry(path.to_string()).or_insert((0, 0));
                    e.0 += a;
                    e.1 += r;
                }
            }
        }
    }

    let status = git(&cwd, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    let mut files = vec![];
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let code = line[..2].to_string(); // raw XY porcelain code (X=staged, Y=unstaged)
        let raw = &line[3..];
        // renames look like "old -> new"; key off the new path
        let path = raw.split(" -> ").last().unwrap_or(raw).trim_matches('"').to_string();
        let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
        files.push(FileChange { path, status: code, added, removed });
    }
    Ok(files)
}

// Unified diff for one file. Falls back through HEAD → staged → untracked (all-added).

#[tauri::command]
pub async fn git_diff(cwd: String, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || git_diff_blocking(cwd, path))
        .await
        .map_err(|e| e.to_string())?
}

fn git_diff_blocking(cwd: String, path: String) -> Result<String, String> {
    if cwd.is_empty() {
        return Ok(String::new());
    }
    let d = git(&cwd, &["diff", "HEAD", "--", &path]).unwrap_or_default();
    if !d.trim().is_empty() {
        return Ok(d);
    }
    let staged = git(&cwd, &["diff", "--cached", "--", &path]).unwrap_or_default();
    if !staged.trim().is_empty() {
        return Ok(staged);
    }
    // untracked file: diff against nothing so it shows as all-added. --no-index exits 1
    // when the files differ, which is expected here, so read stdout regardless of status.
    let out = Command::new("git")
        .arg("-C")
        .arg(&cwd)
        .args(["diff", "--no-index", "--", "/dev/null", &path])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn push_current(cwd: &str) -> Result<(), String> {
    match git(cwd, &["push"]) {
        Ok(_) => Ok(()),
        Err(e)
            if e.contains("no upstream")
                || e.contains("has no upstream")
                || e.contains("set-upstream") =>
        {
            git(cwd, &["push", "-u", "origin", "HEAD"]).map(|_| ())
        }
        Err(e) => Err(e),
    }
}

// commit with the given message and optionally push. stage_all=true stages everything first
// (topbar "Commit & push"); false commits only what's already staged (Source Control panel).

#[tauri::command]
pub async fn git_commit(cwd: String, message: String, push: bool, stage_all: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
            return Err("Not a git repository.".to_string());
        }
        if stage_all {
            git(&cwd, &["add", "-A"])?;
        }
        if let Err(e) = git(&cwd, &["commit", "-m", &message]) {
            return Err(if e.contains("nothing to commit") {
                "Nothing staged to commit.".to_string()
            } else {
                e
            });
        }
        if push {
            push_current(&cwd)?;
            return Ok("Changes committed and pushed.".to_string());
        }
        Ok("Changes committed.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn git_push(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        push_current(&cwd)?;
        Ok("Pushed to remote.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// open a GitHub PR via the gh CLI (fills title/body from commits); returns the PR URL

#[tauri::command]
pub async fn git_create_pr(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        let mut cmd = Command::new("gh");
        cmd.current_dir(&cwd).args(["pr", "create", "--fill"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let out = cmd
            .output()
            .map_err(|_| "GitHub CLI (gh) not found — install it from cli.github.com.".to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() { "Couldn't create the PR.".to_string() } else { err });
        }
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(if url.is_empty() {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        } else {
            url
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// whether a folder is inside a git work tree — gates the topbar "Commit & push" button

#[tauri::command]
pub async fn git_is_repo(cwd: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        !cwd.is_empty() && git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_ok()
    })
    .await
    .unwrap_or(false)
}

// `git init` for a folder that isn't a repo yet

#[tauri::command]
pub async fn git_init(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        git(&cwd, &["init"])?;
        Ok("Initialized a git repository.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Default)]
pub struct BranchInfo {
    branch: String,
    ahead: u32,
    behind: u32,
    upstream: bool,
    is_repo: bool,
}

// current branch + how far ahead/behind its upstream — for the Source Control header

#[tauri::command]
pub async fn git_branch_info(cwd: String) -> BranchInfo {
    tauri::async_runtime::spawn_blocking(move || {
        let mut bi = BranchInfo::default();
        if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
            return bi;
        }
        bi.is_repo = true;
        bi.branch = git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string();
        // "<behind>\t<ahead>" relative to the upstream, if one is set
        if let Ok(s) = git(&cwd, &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]) {
            let mut it = s.split_whitespace();
            bi.behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            bi.ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            bi.upstream = true;
        }
        bi
    })
    .await
    .unwrap_or_default()
}

// stage / unstage / discard a file (empty path = all, except discard which refuses all)

#[tauri::command]
pub async fn git_file_op(cwd: String, op: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        let all = path.is_empty();
        match op.as_str() {
            "stage" => {
                if all {
                    git(&cwd, &["add", "-A"])?;
                } else {
                    git(&cwd, &["add", "--", &path])?;
                }
            }
            "unstage" => {
                if all {
                    git(&cwd, &["reset", "-q"])?;
                } else {
                    git(&cwd, &["reset", "-q", "--", &path])?;
                }
            }
            "discard" => {
                if all {
                    return Err("Refusing to discard everything at once.".to_string());
                }
                // restore a tracked file to HEAD; if it's untracked (not in HEAD), delete it
                if git(&cwd, &["checkout", "HEAD", "--", &path]).is_err() {
                    let _ = std::fs::remove_file(Path::new(&cwd).join(&path));
                }
            }
            _ => return Err("unknown op".to_string()),
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Create (or reuse) a folder for a new project, optionally seeding README.md / .gitignore.
// create_dir_all is idempotent, so this is safe to call on an existing folder too; we only
// write the seed files when they're absent, never clobbering something already there.
