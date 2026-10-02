// What the app keeps between runs: the spaces in the sidebar, their threads, and the composer's
// last picks. The UI owns the shape; the engine saves and loads it whole. Every field has a
// default so a file written by an older build still loads.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::folder::Opener;
use crate::grid::Grid;
use crate::run::{Launch, Permission, Prompt, RunEvent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppState {
    pub spaces: Vec<Space>,
    /// The next id for a space or a thread. Ids are never reused, so a thread's journal can't
    /// end up under someone else's id.
    pub next_id: u64,
    /// The thread on screen when the app closed.
    pub active: Option<u64>,
    pub sidebar_width: f32,
    pub composer: ComposerPrefs,
    pub appearance: Appearance,
    pub dock: DockPrefs,
    /// What the Open button opens a space's folder in.
    pub open_with: Opener,
    /// The intro was shown, or skipped because this was never a first run.
    pub intro_seen: bool,
    /// The version that last ran. A different one on launch means the app was updated, so it
    /// shows what's new in this one. Empty on a first run, which stays quiet.
    pub seen_version: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            spaces: Vec::new(),
            next_id: 1,
            active: None,
            sidebar_width: 272.0,
            composer: ComposerPrefs::default(),
            appearance: Appearance::default(),
            dock: DockPrefs::default(),
            open_with: Opener::default(),
            intro_seen: false,
            seen_version: String::new(),
        }
    }
}

impl AppState {
    pub fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn space(&self, id: u64) -> Option<&Space> {
        self.spaces.iter().find(|s| s.id == id)
    }

    pub fn space_mut(&mut self, id: u64) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|s| s.id == id)
    }

    /// A thread and the space it sits in.
    pub fn thread(&self, id: u64) -> Option<(&Space, &Thread)> {
        self.spaces
            .iter()
            .find_map(|s| s.threads.iter().find(|t| t.id == id).map(|t| (s, t)))
    }

    pub fn thread_mut(&mut self, id: u64) -> Option<&mut Thread> {
        self.spaces
            .iter_mut()
            .find_map(|s| s.threads.iter_mut().find(|t| t.id == id))
    }
}

/// A project (one folder) or an open space (`cwd` is None), as docs/CONTEXT.md defines them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Space {
    pub id: u64,
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub archived: bool,
    /// Folded shut in the sidebar.
    pub folded: bool,
    /// Newest first.
    pub threads: Vec<Thread>,
    /// The panes on screen and how they are laid out.
    pub grid: Grid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Thread {
    pub id: u64,
    pub title: String,
    pub kind: ThreadKind,
    pub archived: bool,
    /// unix ms
    pub created: u64,
}

impl Default for Thread {
    fn default() -> Self {
        Self {
            id: 0,
            title: String::new(),
            kind: ThreadKind::Terminal {
                cwd: PathBuf::new(),
                run: None,
            },
            archived: false,
            created: 0,
        }
    }
}

impl Thread {
    pub fn cwd(&self) -> &PathBuf {
        match &self.kind {
            ThreadKind::Structured { launch } => &launch.cwd,
            ThreadKind::Terminal { cwd, .. } => cwd,
        }
    }

    /// The agent the thread runs, if any: a plain shell runs none.
    pub fn agent(&self) -> Option<&Launch> {
        match &self.kind {
            ThreadKind::Structured { launch } => Some(launch),
            ThreadKind::Terminal { run, .. } => run.as_ref(),
        }
    }

    /// The journal the engine keeps this thread's transcript in.
    pub fn journal(&self) -> String {
        format!("thread-{}", self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ThreadKind {
    /// An agent over its machine protocol. `launch.resume` holds the CLI's thread id once it
    /// started, so the thread picks the conversation up again after a restart.
    Structured { launch: Launch },
    /// A shell in `cwd`. With `run`, the shell starts that agent's CLI interactively. For Claude,
    /// `run.resume` is the conversation id the thread claimed, so it comes back after a restart.
    Terminal {
        cwd: PathBuf,
        #[serde(default)]
        run: Option<Launch>,
    },
}

/// The right dock: whether it is out, how wide, and which tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DockPrefs {
    pub open: bool,
    pub width: f32,
    pub tab: DockTab,
}

impl Default for DockPrefs {
    fn default() -> Self {
        Self {
            open: false,
            width: 320.0,
            tab: DockTab::Files,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DockTab {
    #[default]
    Files,
    Git,
}

/// The theme (an id from `hyprspace_theme::THEMES`), which side of it to show, and the font
/// terminal sessions draw with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: String,
    pub scheme: Scheme,
    /// A font family by name. Empty means the bundled JetBrains Mono.
    pub terminal_font: String,
    pub terminal_font_size: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "t3".into(),
            scheme: Scheme::System,
            terminal_font: String::new(),
            terminal_font_size: 13.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scheme {
    /// Follow the system's light or dark setting.
    #[default]
    System,
    Light,
    Dark,
}

/// The composer's last picks, so the next thread starts the same way.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ComposerPrefs {
    pub agent: Option<Agent>,
    pub permission: Permission,
    pub picks: Vec<Pick>,
    /// Start agents in a terminal session instead of a structured one.
    pub terminal: bool,
}

/// A model and effort picked for one agent. Empty means the CLI's own default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub agent: Agent,
    pub model: String,
    pub effort: String,
}

impl ComposerPrefs {
    pub fn pick(&self, agent: Agent) -> Pick {
        self.picks
            .iter()
            .find(|p| p.agent == agent)
            .cloned()
            .unwrap_or(Pick {
                agent,
                model: String::new(),
                effort: String::new(),
            })
    }

    pub fn set_pick(&mut self, pick: Pick) {
        self.picks.retain(|p| p.agent != pick.agent);
        self.picks.push(pick);
    }
}

/// One line of a thread's journal: what the user sent and answered, and what the run reported.
/// Replaying the lines in order rebuilds the transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Entry {
    Prompt {
        prompt: Prompt,
    },
    Answer {
        request: String,
        answer: crate::run::Answer,
    },
    Run {
        event: RunEvent,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_and_partial_files_still_load() {
        let s: AppState = serde_json::from_str(r#"{"spaces":[{"id":3,"name":"x"}]}"#).unwrap();
        assert_eq!(s.next_id, 1);
        assert_eq!(s.spaces[0].cwd, None);
        assert_eq!(s.sidebar_width, 272.0);
        assert_eq!(s.appearance.theme, "t3");
        assert_eq!(s.appearance.terminal_font_size, 13.0);
    }

    #[test]
    fn finds_threads_and_hands_out_fresh_ids() {
        let mut s = AppState::default();
        let space = s.take_id();
        let thread = s.take_id();
        s.spaces.push(Space {
            id: space,
            threads: vec![Thread {
                id: thread,
                kind: ThreadKind::Structured {
                    launch: Launch::new(Agent::Codex, "/w"),
                },
                ..Default::default()
            }],
            ..Default::default()
        });
        let (sp, t) = s.thread(thread).unwrap();
        assert_eq!((sp.id, t.cwd()), (space, &PathBuf::from("/w")));
        assert_eq!(t.journal(), format!("thread-{thread}"));
        let back: AppState = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn terminal_threads_saved_before_agents_ran_in_them_still_load() {
        let t: Thread =
            serde_json::from_str(r#"{"id":1,"kind":{"type":"terminal","cwd":"/w"}}"#).unwrap();
        assert_eq!(t.agent(), None);
        assert_eq!(t.cwd(), &PathBuf::from("/w"));
    }

    #[test]
    fn picks_default_to_the_cli() {
        let mut p = ComposerPrefs::default();
        assert_eq!(p.pick(Agent::Claude).model, "");
        p.set_pick(Pick {
            agent: Agent::Claude,
            model: "m".into(),
            effort: "high".into(),
        });
        p.set_pick(Pick {
            agent: Agent::Claude,
            model: "n".into(),
            effort: String::new(),
        });
        assert_eq!(p.picks.len(), 1);
        assert_eq!(p.pick(Agent::Claude).model, "n");
    }
}
