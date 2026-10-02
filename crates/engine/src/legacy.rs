// The Tauri app's saved state, brought over once so an updating user keeps their spaces. Its
// store sits beside ours (`~/.hyprspace/v2`, written by src-tauri/src/persist.rs with the shapes
// in src/stores/workspace.ts and settings.ts). This module only ever reads it: the Tauri app may
// still be installed or running, and v2 stays its own.
//
// What comes over (docs/adr/0012-carrying-over-tauri-state.md): every project folder as a space,
// in order, archived ones still archived; the folders an open space's panes ran in, as spaces of
// their own (ADR 0008 dropped open spaces); theme and light or dark; each agent's model and
// effort, the last agent used and its permission mode; the sidebar and dock widths; the Open
// button's app; whether the intro was seen; and the last version that ran, for What's new.
// Panes don't come over as threads: the composer's resume list already offers every
// conversation the CLIs saved for a folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hyprspace_proto::state::{Appearance, Pick, Scheme};
use hyprspace_proto::{Agent, AppState, Opener, Permission, Space};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Workspaces {
    workspaces: Vec<Workspace>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Workspace {
    name: String,
    cwd: String,
    /// "project" or "open"; files from before open spaces have none.
    kind: Option<String>,
    sessions: Vec<Pane>,
    archived: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Pane {
    cwd: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    theme: Option<String>,
    color_scheme: Option<String>,
    claude_permission: Option<String>,
    codex_mode: Option<String>,
    agent_model: HashMap<String, String>,
    agent_effort: HashMap<String, String>,
    last_provider: Option<String>,
    editor: Option<String>,
    rail_width: Option<f32>,
    dock_width: Option<f32>,
    onboarded: Option<bool>,
}

/// Reads one of the Tauri app's files. Absent or unreadable is None: a half-readable store still
/// brings over what it can.
fn read<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Option<T> {
    let raw = std::fs::read_to_string(dir.join(format!("{name}.json"))).ok()?;
    serde_json::from_str(&raw).ok()
}

/// The GPUI state made from the Tauri app's store in `dir`, or None when there is nothing there
/// to bring over.
pub fn import(dir: &Path) -> Option<AppState> {
    let workspaces: Option<Workspaces> = read(dir, "workspaces");
    let settings: Option<Settings> = read(dir, "settings");
    if workspaces.is_none() && settings.is_none() {
        return None;
    }
    let mut state = AppState::default();

    for w in workspaces.unwrap_or_default().workspaces {
        let archived = w.archived.unwrap_or(false);
        if w.kind.as_deref() == Some("open") {
            for pane in w.sessions {
                let cwd = pane.cwd.unwrap_or_default();
                add(&mut state, "", &cwd, archived);
            }
        } else {
            add(&mut state, &w.name, &w.cwd, archived);
        }
    }

    let s = settings.unwrap_or_default();
    state.appearance = Appearance {
        theme: s.theme.unwrap_or(state.appearance.theme),
        scheme: match s.color_scheme.as_deref() {
            Some("light") => Scheme::Light,
            Some("dark") => Scheme::Dark,
            _ => Scheme::System,
        },
    };
    let last = match s.last_provider.as_deref() {
        Some("claude") => Some(Agent::Claude),
        Some("codex") => Some(Agent::Codex),
        Some("gemini") => Some(Agent::Gemini),
        _ => None,
    };
    state.composer.agent = last;
    // the composer keeps one permission mode, so take the one the last agent ran with
    let mode = match last {
        Some(Agent::Codex) => s.codex_mode.as_deref(),
        _ => s.claude_permission.as_deref(),
    };
    state.composer.permission = match mode {
        Some("plan") => Permission::Plan,
        Some("acceptEdits" | "auto") => Permission::Auto,
        Some("bypass") => Permission::Bypass,
        _ => Permission::Ask,
    };
    for agent in [Agent::Claude, Agent::Codex] {
        let get = |m: &HashMap<String, String>| m.get(agent.cli()).cloned().unwrap_or_default();
        let (model, effort) = (get(&s.agent_model), get(&s.agent_effort));
        if !model.is_empty() || !effort.is_empty() {
            state.composer.set_pick(Pick {
                agent,
                model,
                effort,
            });
        }
    }
    state.open_with = match s.editor.as_deref() {
        Some("cursor") => Opener::Cursor,
        Some("files") => Opener::Files,
        _ => Opener::VsCode,
    };
    if let Some(w) = s.rail_width {
        state.sidebar_width = w;
    }
    if let Some(w) = s.dock_width {
        state.dock.width = w;
    }
    state.intro_seen = s.onboarded.unwrap_or(false) || !state.spaces.is_empty();
    // a bare version, or the same as a JSON string
    if let Ok(raw) = std::fs::read_to_string(dir.join("lastSeenVersion.json")) {
        state.seen_version = raw.trim().trim_matches('"').to_string();
    }
    Some(state)
}

/// Adds a space for `cwd` unless one already has that folder. An empty name takes the folder's.
fn add(state: &mut AppState, name: &str, cwd: &str, archived: bool) {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return;
    }
    let path = PathBuf::from(cwd);
    let key = |p: &Path| {
        let s = p
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();
        // Windows paths don't care about case, and the Tauri app saved whatever the picker gave
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    let have = state
        .spaces
        .iter()
        .any(|s| s.cwd.as_deref().is_some_and(|c| key(c) == key(&path)));
    if have {
        return;
    }
    let name = match name.trim() {
        "" => path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.to_string()),
        n => n.to_string(),
    };
    let id = state.take_id();
    state.spaces.push(Space {
        id,
        name,
        cwd: Some(path),
        archived,
        // the Tauri sidebar showed one space open at a time
        folded: true,
        ..Default::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tauri-v2")
    }

    /// Every file in a folder with its bytes, to prove a read changed nothing.
    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let bytes = std::fs::read(&p).unwrap();
                (p, bytes)
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn brings_over_projects_settings_and_the_last_version() {
        let before = snapshot(&fixture());
        let s = import(&fixture()).unwrap();
        assert_eq!(snapshot(&fixture()), before, "v2 must stay untouched");

        let spaces: Vec<_> = s
            .spaces
            .iter()
            .map(|sp| {
                (
                    sp.name.as_str(),
                    sp.cwd.as_ref().unwrap().to_string_lossy().to_string(),
                    sp.archived,
                )
            })
            .collect();
        assert_eq!(
            spaces,
            vec![
                ("api", "/work/api".to_string(), false),
                ("Old site", "/work/site".to_string(), true),
                // the open space's two panes: one new folder, one already a project
                ("notes", "/home/me/notes".to_string(), false),
                ("legacy", "/work/legacy".to_string(), false),
            ]
        );
        // ids are unique and the counter moved past them
        let ids: Vec<_> = s.spaces.iter().map(|sp| sp.id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert_eq!(s.next_id, 5);
        assert!(s.spaces.iter().all(|sp| sp.threads.is_empty()));

        assert_eq!(s.appearance.theme, "iris");
        assert_eq!(s.appearance.scheme, Scheme::Light);
        assert_eq!(s.composer.agent, Some(Agent::Codex));
        assert_eq!(s.composer.permission, Permission::Bypass);
        let claude = s.composer.pick(Agent::Claude);
        assert_eq!(
            (claude.model.as_str(), claude.effort.as_str()),
            ("claude-opus-5-5", "high")
        );
        let codex = s.composer.pick(Agent::Codex);
        assert_eq!(
            (codex.model.as_str(), codex.effort.as_str()),
            ("gpt-6-sol", "")
        );
        assert_eq!(s.open_with, Opener::Files);
        assert_eq!(s.sidebar_width, 342.0);
        assert_eq!(s.dock.width, 380.0);
        assert!(s.intro_seen);
        assert_eq!(s.seen_version, "0.21.1");
    }

    #[test]
    fn nothing_to_bring_over_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(import(dir.path()), None);
        assert_eq!(import(&dir.path().join("missing")), None);
    }

    #[test]
    fn a_broken_workspaces_file_still_brings_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("workspaces.json"), "{not json").unwrap();
        std::fs::write(dir.path().join("settings.json"), r#"{"theme":"ocean"}"#).unwrap();
        let s = import(dir.path()).unwrap();
        assert!(s.spaces.is_empty());
        assert_eq!(s.appearance.theme, "ocean");
        assert_eq!(s.composer.permission, Permission::Ask);
        assert!(!s.intro_seen);
    }
}
