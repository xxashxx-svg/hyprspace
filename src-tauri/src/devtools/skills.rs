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
pub struct SkillItem {
    name: String,
    command: String, // "/name"
    description: String,
    body: String, // markdown after the frontmatter — the instructions, for non-Claude agents
    scope: String, // "project" | "user"
    kind: String,  // "skill" | "command"
}

// everything after the YAML frontmatter block (or the whole file if there's none)

fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.lines();
    if lines.next().map(|l| l.trim()) == Some("---") {
        let mut body = String::new();
        let mut in_fm = true;
        for line in lines {
            if in_fm {
                if line.trim() == "---" {
                    in_fm = false;
                }
                continue;
            }
            body.push_str(line);
            body.push('\n');
        }
        return body.trim().to_string();
    }
    content.trim().to_string()
}

// pull a single `key: value` line out of a markdown YAML frontmatter block (best-effort)

fn frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return None;
    }
    let pfx = format!("{key}:");
    for line in lines {
        let t = line.trim();
        if t == "---" {
            break;
        }
        if let Some(rest) = t.strip_prefix(&pfx) {
            return Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn scan_skills(base: &Path, scope: &str, out: &mut Vec<SkillItem>, seen: &mut std::collections::HashSet<String>) {
    // .claude/skills/<name>/SKILL.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("skills")) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let cmd = format!("/{name}");
            if seen.contains(&cmd) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(e.path().join("SKILL.md")) {
                seen.insert(cmd.clone());
                out.push(SkillItem {
                    description: frontmatter_field(&content, "description").unwrap_or_default(),
                    body: strip_frontmatter(&content),
                    command: cmd,
                    name,
                    scope: scope.to_string(),
                    kind: "skill".to_string(),
                });
            }
        }
    }
    // .claude/commands/<name>.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("commands")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "md") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let cmd = format!("/{stem}");
                    if seen.contains(&cmd) {
                        continue;
                    }
                    seen.insert(cmd.clone());
                    let content = std::fs::read_to_string(&p).unwrap_or_default();
                    out.push(SkillItem {
                        description: frontmatter_field(&content, "description").unwrap_or_default(),
                        body: strip_frontmatter(&content),
                        command: cmd,
                        name: stem.to_string(),
                        scope: scope.to_string(),
                        kind: "command".to_string(),
                    });
                }
            }
        }
    }
}

// project skills (active folder) take priority over user-global ones of the same name

#[tauri::command]
pub async fn list_skills(cwd: String) -> Vec<SkillItem> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = vec![];
        let mut seen = std::collections::HashSet::new();
        let home = home_dir();
        let c = Path::new(&cwd);
        if !cwd.trim().is_empty() && c != home {
            scan_skills(c, "project", &mut out, &mut seen);
        }
        scan_skills(&home, "user", &mut out, &mut seen);
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    })
    .await
    .unwrap_or_default()
}

// keep a skill name safe as a folder name (no path traversal, no separators)

fn safe_skill_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn skill_base(scope: &str, cwd: &str) -> Option<std::path::PathBuf> {
    match scope {
        "user" => Some(home_dir()),
        "project" if !cwd.trim().is_empty() => Some(std::path::PathBuf::from(cwd)),
        _ => None,
    }
}

// the file backing an item: skills/<name>/SKILL.md, or commands/<name>.md for legacy commands

fn skill_file(base: &std::path::Path, kind: &str, name: &str) -> std::path::PathBuf {
    let safe = safe_skill_name(name);
    if kind == "command" {
        base.join(".claude").join("commands").join(format!("{safe}.md"))
    } else {
        base.join(".claude").join("skills").join(safe).join("SKILL.md")
    }
}

#[tauri::command]
pub async fn skill_read(scope: String, cwd: String, name: String, kind: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        std::fs::read_to_string(skill_file(&base, &kind, &name)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skill_write(
    scope: String,
    cwd: String,
    name: String,
    content: String,
    kind: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if safe_skill_name(&name).is_empty() {
            return Err("Needs a name (letters, numbers, hyphens).".to_string());
        }
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        let file = skill_file(&base, &kind, &name);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, content).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skill_delete(scope: String, cwd: String, name: String, kind: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let safe = safe_skill_name(&name);
        if safe.is_empty() {
            return Ok(());
        }
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        if kind == "command" {
            let p = base.join(".claude").join("commands").join(format!("{safe}.md"));
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
        } else {
            let dir = base.join(".claude").join("skills").join(&safe);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
