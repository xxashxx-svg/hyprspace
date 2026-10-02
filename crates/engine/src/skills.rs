// Claude skills and slash commands, from the project folder and the user's home. Copied from
// src-tauri/src/devtools/skills.rs.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::agents::{SkillItem, SkillKind, SkillScope};
use hyprspace_proto::{Event, SkillCommand, SkillEvent};

use crate::util::home_dir;

/// Every skill for `cwd`. Project skills take priority over user ones of the same name.
pub fn list(cwd: &Path) -> Vec<SkillItem> {
    list_in(&home_dir(), cwd)
}

fn list_in(home: &Path, cwd: &Path) -> Vec<SkillItem> {
    let mut out = vec![];
    let mut seen = HashSet::new();
    if !cwd.as_os_str().is_empty() && cwd != home {
        scan(cwd, SkillScope::Project, &mut out, &mut seen);
    }
    scan(home, SkillScope::User, &mut out, &mut seen);
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

fn scan(base: &Path, scope: SkillScope, out: &mut Vec<SkillItem>, seen: &mut HashSet<String>) {
    let mut add = |name: String, kind: SkillKind, content: &str| {
        let command = format!("/{name}");
        if seen.insert(command.clone()) {
            out.push(SkillItem {
                description: frontmatter_field(content, "description").unwrap_or_default(),
                body: strip_frontmatter(content),
                command,
                name,
                scope,
                kind,
            });
        }
    };
    // .claude/skills/<name>/SKILL.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("skills")) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(e.path().join("SKILL.md")) {
                add(
                    e.file_name().to_string_lossy().to_string(),
                    SkillKind::Skill,
                    &content,
                );
            }
        }
    }
    // .claude/commands/<name>.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("commands")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                let content = std::fs::read_to_string(&p).unwrap_or_default();
                add(stem.to_string(), SkillKind::Command, &content);
            }
        }
    }
}

// everything after the YAML frontmatter block (or the whole file if there's none)
fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return content.trim().to_string();
    }
    let mut body = String::new();
    let mut in_fm = true;
    for line in lines {
        if in_fm {
            in_fm = line.trim() != "---";
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    body.trim().to_string()
}

// a single `key: value` line out of a YAML frontmatter block (best-effort)
fn frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
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

// keep a skill name safe as a folder name (no path traversal, no separators)
fn safe_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn base(scope: SkillScope, cwd: &Path) -> Result<PathBuf, String> {
    match scope {
        SkillScope::User => Ok(home_dir()),
        SkillScope::Project if !cwd.as_os_str().is_empty() => Ok(cwd.to_path_buf()),
        SkillScope::Project => Err("No folder for that scope.".into()),
    }
}

// the file backing an item: skills/<name>/SKILL.md, or commands/<name>.md for commands
fn file(base: &Path, kind: SkillKind, name: &str) -> PathBuf {
    let safe = safe_name(name);
    match kind {
        SkillKind::Command => base
            .join(".claude")
            .join("commands")
            .join(format!("{safe}.md")),
        SkillKind::Skill => base
            .join(".claude")
            .join("skills")
            .join(safe)
            .join("SKILL.md"),
    }
}

pub fn read(scope: SkillScope, cwd: &Path, kind: SkillKind, name: &str) -> Result<String, String> {
    std::fs::read_to_string(file(&base(scope, cwd)?, kind, name)).map_err(|e| e.to_string())
}

pub fn write(
    scope: SkillScope,
    cwd: &Path,
    kind: SkillKind,
    name: &str,
    content: &str,
) -> Result<(), String> {
    write_in(&base(scope, cwd)?, kind, name, content)
}

fn write_in(base: &Path, kind: SkillKind, name: &str, content: &str) -> Result<(), String> {
    if safe_name(name).is_empty() {
        return Err("Needs a name (letters, numbers, hyphens).".to_string());
    }
    let file = file(base, kind, name);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, content).map_err(|e| e.to_string())
}

pub fn delete(scope: SkillScope, cwd: &Path, kind: SkillKind, name: &str) -> Result<(), String> {
    delete_in(&base(scope, cwd)?, kind, name)
}

fn delete_in(base: &Path, kind: SkillKind, name: &str) -> Result<(), String> {
    let safe = safe_name(name);
    if safe.is_empty() {
        return Ok(());
    }
    let res = match kind {
        SkillKind::Command => std::fs::remove_file(file(base, kind, name)),
        SkillKind::Skill => {
            std::fs::remove_dir_all(base.join(".claude").join("skills").join(&safe))
        }
    };
    match res {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// Answers a Skills request on the blocking pool. A write or delete that worked is followed by
/// the folder's fresh list, so the view never shows a skill that is gone.
pub fn handle(cmd: SkillCommand, tx: UnboundedSender<Event>) {
    let send = move |e: SkillEvent| {
        let _ = tx.unbounded_send(Event::Skills(e));
    };
    tokio::task::spawn_blocking(move || {
        let (cwd, result) = match cmd {
            SkillCommand::List { cwd } => {
                let items = list(&cwd);
                return send(SkillEvent::List { cwd, items });
            }
            SkillCommand::Read {
                cwd,
                scope,
                kind,
                name,
            } => {
                let content = read(scope, &cwd, kind, &name);
                return send(SkillEvent::Read {
                    scope,
                    name,
                    content,
                });
            }
            SkillCommand::Write {
                cwd,
                scope,
                kind,
                name,
                content,
                replaces,
            } => {
                let result =
                    write(scope, &cwd, kind, &name, &content).and_then(|()| match replaces {
                        Some((old_scope, old)) if old_scope != scope || old != name => {
                            delete(old_scope, &cwd, kind, &old)
                        }
                        _ => Ok(()),
                    });
                (cwd, result)
            }
            SkillCommand::Delete {
                cwd,
                scope,
                kind,
                name,
            } => {
                let result = delete(scope, &cwd, kind, &name);
                (cwd, result)
            }
        };
        let ok = result.is_ok();
        send(SkillEvent::Done {
            error: result.err(),
        });
        if ok {
            let items = list(&cwd);
            send(SkillEvent::List { cwd, items });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL: &str = "---\nname: x\ndescription: \"Does x\"\n---\n\nRun x.\n";

    #[test]
    fn reads_frontmatter() {
        assert_eq!(
            frontmatter_field(SKILL, "description").as_deref(),
            Some("Does x")
        );
        assert_eq!(frontmatter_field(SKILL, "missing"), None);
        assert_eq!(strip_frontmatter(SKILL), "Run x.");
        assert_eq!(strip_frontmatter("plain\n"), "plain");
    }

    #[test]
    fn names_cannot_traverse() {
        assert_eq!(safe_name("../../etc"), "etc");
        assert_eq!(safe_name(" my skill "), "my-skill");
    }

    #[test]
    fn project_skills_shadow_user_ones() {
        let home = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_in(home.path(), SkillKind::Skill, "deploy", SKILL).unwrap();
        write_in(home.path(), SkillKind::Command, "Notes", "just text").unwrap();
        write_in(proj.path(), SkillKind::Skill, "deploy", "project version").unwrap();

        let got = list_in(home.path(), proj.path());
        let rows: Vec<_> = got
            .iter()
            .map(|s| (s.command.as_str(), s.scope, s.kind, s.body.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "/deploy",
                    SkillScope::Project,
                    SkillKind::Skill,
                    "project version"
                ),
                ("/Notes", SkillScope::User, SkillKind::Command, "just text"),
            ]
        );

        delete_in(proj.path(), SkillKind::Skill, "deploy").unwrap();
        delete_in(proj.path(), SkillKind::Skill, "deploy").unwrap();
        assert_eq!(list_in(home.path(), proj.path())[0].scope, SkillScope::User);
        assert!(write_in(home.path(), SkillKind::Skill, "///", "x").is_err());
    }
}
