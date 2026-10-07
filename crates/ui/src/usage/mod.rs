// Plan limits and usage: the ring in the top bar (`meter.rs`) and Settings' Usage
// view (`page.rs`). One `Limits` entity holds every reading for both, so opening Settings never
// fetches anything the meter didn't.
//
// Asking cadence follows CLAUDE.md's Usage section: the live endpoints every 180s for Claude and
// 60s for Codex. The engine enforces those as floors too (engine/src/usage/live.rs), so nothing
// here can ask faster by mistake. Codex's session files are read only while its live reading has
// nothing, and Claude's status line arrives on its own from terminal sessions.

mod activity;
mod chart;
mod limits;
mod meter;
pub mod model;
mod overview;
mod page;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{Context, Hsla, Task};
use hyprspace_proto::usage::ProviderUsage;
use hyprspace_proto::{Agent, Client, Command, UsageCommand, UsageEvent};

use model::{Heard, Readings};

pub(crate) use meter::ring;
pub(crate) use page::masked;

const CLAUDE_EVERY: Duration = Duration::from_secs(180);
const CODEX_EVERY: Duration = Duration::from_secs(60);
const FILES_EVERY: Duration = Duration::from_secs(60);
/// Reset countdowns are the only thing that moves between readings.
const TICK: Duration = Duration::from_secs(30);

/// The providers Settings' Activity view reads, in its order.
pub const PROVIDERS: [(&str, &str); 4] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("opencode", "OpenCode"),
    ("grok", "Grok"),
];

/// Limits or Activity, in Settings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Limits,
    Activity,
}

pub struct Limits {
    client: Client,
    pub(crate) readings: Readings,
    asked: HashMap<&'static str, Instant>,
    /// The popover under the ring, and which provider's tab it shows.
    pub(crate) open: Option<gpui::Point<gpui::Pixels>>,
    pub(crate) tab: Agent,
    pub(crate) view: View,
    /// Activity: each provider's local files, and the scans still running.
    pub(crate) local: HashMap<String, ProviderUsage>,
    pub(crate) pending: HashSet<String>,
    /// Activity's chart: by agent or model, the lines hidden, the day under the pointer.
    pub(crate) chart: chart::State,
    loaded: bool,
    _poll: Task<()>,
}

impl Limits {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |l, cx| l.tick(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(TICK).await;
            }
        });
        Self {
            client,
            readings: Readings::default(),
            asked: HashMap::new(),
            open: None,
            tab: Agent::Claude,
            view: View::Limits,
            local: HashMap::new(),
            pending: HashSet::new(),
            chart: chart::State::default(),
            loaded: false,
            _poll: poll,
        }
    }

    fn due(&mut self, what: &'static str, every: Duration) -> bool {
        let due = self.asked.get(what).is_none_or(|t| t.elapsed() >= every);
        if due {
            self.asked.insert(what, Instant::now());
        }
        due
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.due("claude", CLAUDE_EVERY) {
            self.client.send(Command::Usage(UsageCommand::Live {
                agent: Agent::Claude,
            }));
        }
        if self.due("codex", CODEX_EVERY) {
            self.client.send(Command::Usage(UsageCommand::Live {
                agent: Agent::Codex,
            }));
        }
        if self.readings.codex_needs_files() && self.due("codex-files", FILES_EVERY) {
            self.client.send(Command::Usage(UsageCommand::Local {
                provider: "codex".into(),
            }));
        }
        cx.notify();
    }

    pub fn event(&mut self, e: UsageEvent, cx: &mut Context<Self>) {
        match e {
            UsageEvent::Live { agent, usage } => match agent {
                Agent::Claude => self.readings.claude = Some(*usage),
                Agent::Codex => self.readings.codex = Some(*usage),
            },
            UsageEvent::Local { provider, usage } => {
                self.pending.remove(&provider);
                if let Some(u) = usage {
                    if provider == "codex" {
                        self.readings.codex_files = Some((*u).clone());
                    }
                    self.local.insert(provider, *u);
                }
            }
            UsageEvent::StatusLine { id, report } => {
                let at = crate::time::now_ms() as i64;
                self.readings.heard.insert(id, Heard { at, report });
            }
        }
        cx.notify();
    }

    /// Reads every provider's files for Activity. The scans are the slow part, so they wait
    /// until the view is first opened, and each card lands as its scan finishes.
    pub(crate) fn load_activity(&mut self, cx: &mut Context<Self>) {
        self.loaded = true;
        for (id, _) in PROVIDERS {
            self.pending.insert(id.to_string());
            self.client.send(Command::Usage(UsageCommand::Local {
                provider: id.into(),
            }));
        }
        cx.notify();
    }

    pub(crate) fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        self.view = view;
        if view == View::Activity && !self.loaded {
            self.load_activity(cx);
        }
        cx.notify();
    }
}

/// A provider's signature color, by its CLI name.
pub(crate) fn brand(id: &str) -> Hsla {
    crate::colors::hsla(hyprspace_theme::brand(id).0)
}
