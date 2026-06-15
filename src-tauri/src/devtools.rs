// Dev cockpit backend: git changes/diff for the Review dock, and dev-command detection
// for the Run panel. All best-effort — a non-repo or missing git returns empty, never errors.
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Serialize)]
pub struct FileChange {
    path: String,
    status: String, // porcelain code: M, A, D, R, ??, etc.
    added: u32,
    removed: u32,
}

fn git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// Changed files in the active workspace's repo, with +/- line counts where git has them.
#[tauri::command]
pub fn git_changes(cwd: String) -> Result<Vec<FileChange>, String> {
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
        let code = line[..2].trim().to_string();
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
pub fn git_diff(cwd: String, path: String) -> Result<String, String> {
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

// Best-guess command to run the project (for the Run panel).
#[tauri::command]
pub fn detect_run_cmd(cwd: String) -> String {
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
