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
// Each agent pane comes over as a terminal thread in its folder's space, on the conversation it
// was on, so opening it picks up where the Tauri app left off. Nothing launches until a thread
// is opened, so a Tauri app still running never shares a live conversation with this one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hyprspace_proto::state::{Appearance, Pick, Scheme};
use hyprspace_proto::{Agent, AppState, Launch, Opener, Permission, Space, Thread, ThreadKind};
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
#[serde(default, rename_all = "camelCase")]
struct Pane {
    id: String,
    title: String,
    cwd: Option<String>,
    provider: String,
    /// The launch command, which holds the permission flag and the effort.
    command: Option<String>,
    model: Option<String>,
    /// The Claude conversation the pane is on. It follows a manual /resume, so it beats `id`.
    claude_session_id: Option<String>,
    draft: bool,
    ephemeral: bool,
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
    let home = crate::util::home_dir();

    for w in workspaces.unwrap_or_default().workspaces {
        let archived = w.archived.unwrap_or(false);
        let open = w.kind.as_deref() == Some("open");
        if !open {
            add(&mut state, &w.name, &w.cwd, archived);
        }
        for pane in w.sessions {
            let cwd = pane
                .cwd
                .clone()
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| w.cwd.clone());
            if open {
                add(&mut state, "", &cwd, archived);
            }
            thread(&mut state, &home, pane, &cwd);
        }
    }
    for space in &mut state.spaces {
        space.threads.sort_by_key(|t| std::cmp::Reverse(t.created));
        // the Tauri sidebar opened a space that had threads
        space.folded = space.threads.is_empty();
    }

    let s = settings.unwrap_or_default();
    state.appearance = Appearance {
        theme: s.theme.unwrap_or(state.appearance.theme),
        scheme: match s.color_scheme.as_deref() {
            Some("light") => Scheme::Light,
            Some("dark") => Scheme::Dark,
            _ => Scheme::System,
        },
        ..Appearance::default()
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

/// A folder as a key: Windows paths don't care about case, and the Tauri app saved whatever the
/// picker gave, with or without a trailing slash.
fn key(p: &Path) -> String {
    let s = p
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Adds a space for `cwd` unless one already has that folder. An empty name takes the folder's.
fn add(state: &mut AppState, name: &str, cwd: &str, archived: bool) {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return;
    }
    let path = PathBuf::from(cwd);
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

/// The value after `flag` in a launch command: `--effort high`, `--permission-mode plan`.
fn flag<'a>(command: &'a str, flag: &str) -> Option<&'a str> {
    let mut words = command.split_whitespace();
    words.find(|w| *w == flag)?;
    words.next().map(|w| w.trim_matches('"'))
}

/// The permission a pane ran with, read off its command.
fn permission(command: &str) -> Permission {
    if command.contains("--dangerously-skip-permissions") || command.contains("--yolo") {
        return Permission::Bypass;
    }
    match flag(command, "--permission-mode") {
        Some("plan") => Permission::Plan,
        Some("acceptEdits") => Permission::Auto,
        _ => Permission::Ask,
    }
}

/// Adds an agent pane as a terminal thread in its folder's space. Viewer tabs, unsent drafts,
/// automation runs and bare shells stay behind.
fn thread(state: &mut AppState, home: &Path, pane: Pane, cwd: &str) {
    let agent = match pane.provider.as_str() {
        "claude" => Agent::Claude,
        "codex" => Agent::Codex,
        "gemini" => Agent::Gemini,
        _ => return,
    };
    if pane.draft || pane.ephemeral || cwd.trim().is_empty() {
        return;
    }
    let path = PathBuf::from(cwd.trim());
    let command = pane.command.unwrap_or_default();
    // Only Claude panes knew their conversation. It is the one the pane followed, or the one its
    // command resumed (a pane opened from the resume list), or the one its own id started. The
    // first with a transcript on disk wins; a pane that never started keeps its own id, which
    // names the conversation it will start.
    let transcript =
        |id: &str| crate::sessions::claude_project_dir(home, &path).join(format!("{id}.jsonl"));
    let candidates: Vec<String> = [
        pane.claude_session_id.clone(),
        flag(&command, "--resume").map(str::to_string),
        Some(pane.id.clone()),
    ]
    .into_iter()
    .flatten()
    .filter(|id| !id.is_empty())
    .collect();
    let resume = (agent == Agent::Claude)
        .then(|| {
            candidates
                .iter()
                .find(|id| transcript(id).exists())
                .or(candidates.first())
                .cloned()
        })
        .flatten();
    // the transcript's last write is when the thread last did anything
    let created = resume
        .as_ref()
        .and_then(|id| {
            std::fs::metadata(transcript(id))
                .and_then(|m| m.modified())
                .ok()
        })
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    let launch = Launch {
        model: pane
            .model
            .filter(|m| !m.is_empty())
            .or_else(|| flag(&command, "--model").map(str::to_string)),
        effort: flag(&command, "--effort").map(str::to_string),
        permission: permission(&command),
        resume,
        ..Launch::new(agent, path.clone())
    };
    let Some(i) = state
        .spaces
        .iter()
        .position(|s| s.cwd.as_deref().is_some_and(|c| key(c) == key(&path)))
    else {
        return;
    };
    let id = state.take_id();
    let title = match pane.title.trim() {
        "" => agent.name().to_string(),
        t => t.to_string(),
    };
    state.spaces[i].threads.push(Thread {
        id,
        title,
        kind: ThreadKind::Terminal {
            cwd: path,
            run: Some(launch),
        },
        created,
        touched: created,
        ..Thread::default()
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
        // ids are unique across spaces and threads, and the counter moved past them
        let mut ids: Vec<_> = s
            .spaces
            .iter()
            .flat_map(|sp| std::iter::once(sp.id).chain(sp.threads.iter().map(|t| t.id)))
            .collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert_eq!(s.next_id, n as u64 + 1);

        // each agent pane is a terminal thread in its folder's space, on its own conversation,
        // with the model and permission its command ran with
        let runs = |sp: &Space| -> Vec<(String, Agent, Option<String>, Option<String>)> {
            sp.threads
                .iter()
                .map(|t| match &t.kind {
                    ThreadKind::Terminal { run: Some(l), .. } => {
                        (t.title.clone(), l.agent, l.resume.clone(), l.model.clone())
                    }
                    other => panic!("not an agent thread: {other:?}"),
                })
                .collect()
        };
        assert_eq!(
            runs(&s.spaces[0]),
            vec![
                (
                    "api".into(),
                    Agent::Claude,
                    Some("s1".into()),
                    Some("claude-opus-5-5".into())
                ),
                // from the open space, into the space that already had its folder
                ("api".into(), Agent::Claude, Some("s3".into()), None),
            ]
        );
        assert_eq!(
            runs(&s.spaces[2]),
            vec![("notes".into(), Agent::Codex, None, None)]
        );
        // a space with threads opens; one without stays folded, and nothing is on screen yet
        assert!(!s.spaces[0].folded && s.spaces[3].folded);
        assert!(s.spaces[1].threads.is_empty() && s.spaces[3].threads.is_empty());

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
    fn a_command_tells_the_permission_and_the_effort() {
        let cmd = r#"claude --resume x --dangerously-skip-permissions --model "claude-opus-5-5[1m]" --effort high"#;
        assert_eq!(permission(cmd), Permission::Bypass);
        assert_eq!(flag(cmd, "--effort"), Some("high"));
        assert_eq!(flag(cmd, "--model"), Some("claude-opus-5-5[1m]"));
        assert_eq!(
            permission("claude --permission-mode plan"),
            Permission::Plan
        );
        assert_eq!(permission("claude"), Permission::Ask);
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
