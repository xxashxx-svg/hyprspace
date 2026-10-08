// What the meter and Settings' Usage view show, worked out from whatever readings came in. No
// drawing and no requests here. Follows the Tauri app's src/lib/limits.ts and src/stores/usage.ts,
// so both views can never disagree.
//
// Three sources, best first: the account's live limits (the usage endpoints, polled under the
// engine's floors), what Claude's status line pushes each turn in a terminal session, and the
// windows Codex writes into its session files.

use std::collections::HashMap;

use hyprspace_proto::usage::{LiveBar, LiveExtra, LiveUsage, ProviderUsage, StatusReport};
use hyprspace_proto::{Agent, SessionId};

/// A status line older than this is no longer burning anything.
pub const STALE_MS: i64 = 30 * 60_000;

/// One limit window as both views draw it.
#[derive(Debug, Clone, PartialEq)]
pub struct Win {
    pub key: String,
    pub label: String,
    /// 0-100, used.
    pub pct: f64,
    /// unix ms
    pub resets_at: Option<i64>,
    pub window_ms: Option<i64>,
    /// The provider's own call, trusted over anything inferred.
    pub severity: Option<String>,
    /// The number is the previous window's, so this one's is not known yet.
    pub stale: bool,
}

impl Win {
    fn from_bar(b: &LiveBar) -> Self {
        Self {
            key: b.id.clone(),
            label: b.label.clone(),
            pct: b.percent.clamp(0.0, 100.0),
            resets_at: b.resets_at,
            window_ms: b.window_ms,
            severity: b.severity.clone(),
            stale: false,
        }
    }

    /// Once the reset passes, the last number is the old window's final one (usually near
    /// 100%), and the new one is unknown until the next reading. Say so instead of a stale red.
    pub fn expired(&self, now: i64) -> bool {
        self.stale || self.resets_at.is_some_and(|r| r <= now)
    }

    /// How much of the window's time is still to come, 0 to 100.
    pub fn time_left_pct(&self, now: i64) -> Option<f64> {
        let (len, reset) = (self.window_ms?, self.resets_at?);
        let left = reset - now;
        (left > 0 && left <= len).then(|| left as f64 / len as f64 * 100.0)
    }

    /// Claude's own warning shape: spending against how far through the window you are, not a
    /// flat line. 89% with hours left is a problem; 89% with minutes left is not.
    pub fn tone(&self, now: i64) -> Tone {
        if self.expired(now) {
            return Tone::Calm;
        }
        match self.severity.as_deref() {
            Some("critical") => return Tone::Crit,
            Some("warning") => return Tone::Warn,
            Some("normal") if self.pct < 90.0 => return Tone::Calm,
            _ => {}
        }
        if self.pct >= 90.0 {
            return Tone::Crit;
        }
        if let Some(left) = self.time_left_pct(now) {
            let ahead = self.pct - (100.0 - left);
            if ahead > 14.0 {
                return Tone::Crit;
            }
            if ahead > 4.0 {
                return Tone::Warn;
            }
        }
        if self.pct >= 75.0 {
            Tone::Warn
        } else {
            Tone::Calm
        }
    }

    /// "42m", "4h 42m", "4d 1h", or "resetting" once it is due.
    pub fn reset_label(&self, now: i64) -> String {
        let Some(reset) = self.resets_at else {
            return String::new();
        };
        let ms = reset - now;
        if ms <= 0 {
            return "resetting".into();
        }
        let m = (ms as f64 / 60_000.0).round() as i64;
        if m < 60 {
            return format!("{m}m");
        }
        let h = m / 60;
        if h < 48 {
            format!("{h}h {}m", m % 60)
        } else {
            format!("{}d {}h", h / 24, h % 24)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tone {
    Calm,
    Warn,
    Crit,
}

/// One provider's limits.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub agent: Agent,
    pub plan: Option<String>,
    /// unix ms of the reading
    pub updated_at: Option<i64>,
    pub windows: Vec<Win>,
    /// Why the numbers are old: rate limited, the last good reading, a file's age.
    pub note: Option<String>,
    pub extra: Option<LiveExtra>,
}

/// A live reading as a block. None when it carries no windows.
pub fn live_block(agent: Agent, u: &LiveUsage) -> Option<Block> {
    if u.bars.is_empty() {
        return None;
    }
    Some(Block {
        agent,
        plan: u.plan.clone(),
        updated_at: (u.at > 0).then_some(u.at),
        windows: u.bars.iter().map(Win::from_bar).collect(),
        note: u.note.clone(),
        extra: u.extra.clone(),
    })
}

/// Codex's windows from its session files. They are only written mid-session, so the block says
/// how old they are.
pub fn files_block(u: &ProviderUsage, now: i64) -> Option<Block> {
    let windows: Vec<Win> = [("primary", &u.primary), ("secondary", &u.secondary)]
        .into_iter()
        .filter_map(|(key, w)| {
            let w = w.as_ref()?;
            Some(Win {
                key: key.into(),
                label: window_name(w.window_minutes),
                pct: w.used_percent.clamp(0.0, 100.0),
                resets_at: (w.resets_at > 0).then_some(w.resets_at * 1000),
                window_ms: (w.window_minutes > 0).then_some(w.window_minutes as i64 * 60_000),
                severity: None,
                stale: false,
            })
        })
        .collect();
    if windows.is_empty() {
        return None;
    }
    let updated_at = (u.updated_at > 0).then_some(u.updated_at * 1000);
    let note = updated_at.map(
        |at| match crate::time::ago(at as u64, now as u64).as_str() {
            "now" => "Just updated.".to_string(),
            age => format!("As of {age} ago."),
        },
    );
    Some(Block {
        agent: Agent::Codex,
        plan: u.plan.clone(),
        updated_at,
        windows,
        note,
        extra: None,
    })
}

/// A window named by its own length rather than assuming which plan this is.
fn window_name(minutes: u64) -> String {
    match minutes {
        0 => "Limit".into(),
        m if m % 1440 == 0 && m / 1440 == 7 => "This week".into(),
        m if m % 1440 == 0 => format!("{} days", m / 1440),
        m if (m as f64 / 60.0).round() == 5.0 => "Session · 5h".into(),
        m => format!("{} hours", (m as f64 / 60.0).round()),
    }
}

/// A status line as it arrived.
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    pub at: i64,
    pub report: StatusReport,
}

/// Every terminal session's status line folded into one account-level block. The windows are
/// account-wide, so the freshest report that has them wins rather than a sum; Claude never says
/// which session spent what. The second value is true when that report is old.
fn status_block(heard: &HashMap<SessionId, Heard>, now: i64) -> Option<(Block, bool)> {
    let newest = heard
        .values()
        .filter(|h| !h.report.windows.is_empty())
        .max_by_key(|h| h.at)?;
    // A report saying "resets at R" describes [R - len, R]. If it arrived before that window
    // began, it is the old window's last number carrying an already rolled-forward reset.
    let windows = newest
        .report
        .windows
        .iter()
        .map(|b| {
            let mut w = Win::from_bar(b);
            if let (Some(r), Some(len)) = (w.resets_at, w.window_ms) {
                w.stale = newest.at < r - len;
            }
            w
        })
        .collect();
    // the models in use right now, newest first, without the marketing suffix
    let mut live: Vec<&Heard> = heard
        .values()
        .filter(|h| now - h.at < STALE_MS && h.report.model.is_some())
        .collect();
    live.sort_by_key(|h| std::cmp::Reverse(h.at));
    let mut models: Vec<String> = Vec::new();
    for h in live {
        let m = h.report.model.as_deref().unwrap_or_default();
        let m = m.split(" (").next().unwrap_or(m).trim().to_string();
        if !models.contains(&m) {
            models.push(m);
        }
    }
    let plan = models.first().map(|first| match models.len() {
        1 => first.clone(),
        n => format!("{first} +{}", n - 1),
    });
    let block = Block {
        agent: Agent::Claude,
        plan,
        updated_at: Some(newest.at),
        windows,
        note: None,
        extra: None,
    };
    Some((block, now - newest.at >= STALE_MS))
}

/// Everything the readings say right now.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Picture {
    pub claude: Option<Block>,
    pub codex: Option<Block>,
    /// No Claude session has reported in a while, so the status-line numbers may be old.
    pub claude_stale: bool,
    /// Why a provider has no numbers at all, when its endpoint said.
    pub claude_missing: Option<String>,
    pub codex_missing: Option<String>,
}

/// The readings the views work from, as they last arrived.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    pub claude: Option<LiveUsage>,
    pub codex: Option<LiveUsage>,
    pub heard: HashMap<SessionId, Heard>,
    pub codex_files: Option<ProviderUsage>,
}

impl Readings {
    pub fn picture(&self, now: i64) -> Picture {
        let from_status = status_block(&self.heard, now);
        let live_claude = self
            .claude
            .as_ref()
            .and_then(|u| live_block(Agent::Claude, u));
        let claude_stale = live_claude.is_none() && from_status.as_ref().is_some_and(|s| s.1);
        // the live reading wins, but only the status line knows which model is running now
        let claude = match live_claude {
            Some(mut b) => {
                if let Some(plan) = from_status.as_ref().and_then(|s| s.0.plan.clone()) {
                    b.plan = Some(plan);
                }
                Some(b)
            }
            None => from_status.map(|s| s.0),
        };
        let codex = self
            .codex
            .as_ref()
            .and_then(|u| live_block(Agent::Codex, u))
            .or_else(|| self.codex_files.as_ref().and_then(|u| files_block(u, now)));
        let note = |u: &Option<LiveUsage>| u.as_ref().and_then(|u| u.note.clone());
        Picture {
            claude_missing: claude.is_none().then(|| note(&self.claude)).flatten(),
            codex_missing: codex.is_none().then(|| note(&self.codex)).flatten(),
            claude,
            codex,
            claude_stale,
        }
    }

    /// True when Codex's live reading has nothing, so its session files are worth reading.
    pub fn codex_needs_files(&self) -> bool {
        self.codex.as_ref().is_none_or(|u| u.bars.is_empty())
    }
}

impl Picture {
    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.claude.iter().chain(self.codex.iter())
    }

    pub fn spent_until(&self, agent: Agent, now: i64) -> Option<i64> {
        self.blocks()
            .filter(|b| b.agent == agent)
            .flat_map(|b| &b.windows)
            .filter(|w| w.pct >= 99.5)
            .filter_map(|w| w.resets_at)
            .filter(|at| *at > now)
            .max()
    }

    /// The window the ring reports: the most urgent across every provider, so it never sits
    /// calmly on Claude's session while Codex is about to run out. A spent window can't move and
    /// can't be acted on, so it only counts when every window is spent.
    pub fn worst(&self, now: i64) -> Option<&Win> {
        let all: Vec<&Win> = self
            .blocks()
            .flat_map(|b| &b.windows)
            .filter(|w| !w.expired(now))
            .collect();
        let live: Vec<&Win> = all.iter().copied().filter(|w| w.pct < 100.0).collect();
        let pool = if live.is_empty() { all } else { live };
        pool.into_iter().reduce(|a, b| {
            let (ta, tb) = (a.tone(now), b.tone(now));
            if tb > ta || (tb == ta && b.pct > a.pct) {
                b
            } else {
                a
            }
        })
    }

    pub fn any_window(&self) -> bool {
        self.blocks().any(|b| !b.windows.is_empty())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_spent_window_says_when_the_agent_can_go_again() {
        let win = |pct: f64, at: i64| Win {
            key: "w".into(),
            label: "w".into(),
            pct,
            resets_at: Some(at),
            window_ms: None,
            severity: None,
            stale: false,
        };
        let block = |agent, windows| Block {
            agent,
            plan: None,
            updated_at: None,
            windows,
            note: None,
            extra: None,
        };
        let p = Picture {
            claude: Some(block(Agent::Claude, vec![win(100.0, 500), win(40.0, 900)])),
            codex: Some(block(Agent::Codex, vec![win(80.0, 700)])),
            claude_stale: false,
            claude_missing: None,
            codex_missing: None,
        };
        assert_eq!(p.spent_until(Agent::Claude, 100), Some(500));
        assert_eq!(p.spent_until(Agent::Claude, 600), None);
        assert_eq!(p.spent_until(Agent::Codex, 100), None);
    }
    use hyprspace_proto::usage::UsageWindow;

    use super::*;

    const NOW: i64 = 1_800_000_000_000;
    const HOUR: i64 = 3_600_000;

    fn win(pct: f64, reset_in: Option<i64>, len: Option<i64>) -> Win {
        Win {
            key: "k".into(),
            label: "K".into(),
            pct,
            resets_at: reset_in.map(|r| NOW + r),
            window_ms: len,
            severity: None,
            stale: false,
        }
    }

    fn bar(id: &str, pct: f64) -> LiveBar {
        LiveBar {
            id: id.into(),
            label: id.into(),
            percent: pct,
            ..Default::default()
        }
    }

    #[test]
    fn tone_follows_pace_not_a_flat_line() {
        // 60% used with 4 of 5 hours to go is running hot
        assert_eq!(
            win(60., Some(4 * HOUR), Some(5 * HOUR)).tone(NOW),
            Tone::Crit
        );
        // 60% with 30 minutes left is fine
        assert_eq!(
            win(60., Some(HOUR / 2), Some(5 * HOUR)).tone(NOW),
            Tone::Calm
        );
        // a little ahead of the clock warns
        assert_eq!(
            win(30., Some(4 * HOUR), Some(5 * HOUR)).tone(NOW),
            Tone::Warn
        );
        // no length known: flat thresholds
        assert_eq!(win(80., None, None).tone(NOW), Tone::Warn);
        assert_eq!(win(95., None, None).tone(NOW), Tone::Crit);
        // a spent window says nothing
        assert_eq!(win(100., Some(-1), Some(HOUR)).tone(NOW), Tone::Calm);
        let mut told = win(95., None, None);
        told.severity = Some("normal".into());
        assert_eq!(told.tone(NOW), Tone::Crit);
        told.pct = 50.;
        assert_eq!(told.tone(NOW), Tone::Calm);
        told.severity = Some("warning".into());
        assert_eq!(told.tone(NOW), Tone::Warn);
    }

    #[test]
    fn reset_labels() {
        let w = |ms| win(1., Some(ms), None).reset_label(NOW);
        assert_eq!(w(42 * 60_000), "42m");
        assert_eq!(w(4 * HOUR + 42 * 60_000), "4h 42m");
        assert_eq!(w(97 * HOUR), "4d 1h");
        assert_eq!(w(-5), "resetting");
        assert_eq!(win(1., None, None).reset_label(NOW), "");
    }

    #[test]
    fn live_wins_and_borrows_the_model_from_the_status_line() {
        let mut r = Readings::default();
        r.heard.insert(
            SessionId(1),
            Heard {
                at: NOW - 1000,
                report: StatusReport {
                    model: Some("Opus 5.5 (1M context)".into()),
                    model_id: None,
                    windows: vec![bar("five_hour", 10.)],
                },
            },
        );
        let p = r.picture(NOW);
        let c = p.claude.as_ref().unwrap();
        assert_eq!(c.plan.as_deref(), Some("Opus 5.5"));
        assert_eq!(c.windows[0].pct, 10.);
        assert!(!p.claude_stale);

        r.claude = Some(LiveUsage {
            ok: true,
            at: NOW,
            plan: Some("Max 20x".into()),
            bars: vec![bar("session", 40.), bar("weekly", 5.)],
            ..Default::default()
        });
        let p = r.picture(NOW);
        let c = p.claude.as_ref().unwrap();
        assert_eq!(c.windows.len(), 2);
        assert_eq!(c.windows[0].pct, 40.);
        assert_eq!(c.plan.as_deref(), Some("Opus 5.5"));
        assert_eq!(p.worst(NOW).unwrap().pct, 40.);
    }

    #[test]
    fn an_old_status_line_is_marked_and_a_signed_out_endpoint_explains_itself() {
        let mut r = Readings {
            claude: Some(LiveUsage {
                note: Some("Run claude in a terminal to sign in".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let p = r.picture(NOW);
        assert_eq!(p.claude, None);
        assert_eq!(
            p.claude_missing.as_deref(),
            Some("Run claude in a terminal to sign in")
        );
        r.heard.insert(
            SessionId(2),
            Heard {
                at: NOW - STALE_MS,
                report: StatusReport {
                    model: None,
                    model_id: None,
                    windows: vec![bar("five_hour", 10.)],
                },
            },
        );
        let p = r.picture(NOW);
        assert!(p.claude.is_some());
        assert!(p.claude_stale);
        assert_eq!(p.claude_missing, None);
    }

    #[test]
    fn a_window_reported_before_it_began_is_stale() {
        let mut r = Readings::default();
        let mut b = bar("five_hour", 100.);
        b.window_ms = Some(5 * HOUR);
        b.resets_at = Some(NOW + 6 * HOUR);
        r.heard.insert(
            SessionId(1),
            Heard {
                at: NOW,
                report: StatusReport {
                    windows: vec![b],
                    ..Default::default()
                },
            },
        );
        let p = r.picture(NOW);
        assert!(p.claude.unwrap().windows[0].expired(NOW));
    }

    #[test]
    fn codex_falls_back_to_its_files_with_their_age() {
        let mut r = Readings {
            codex: Some(LiveUsage {
                note: Some("Rate limited. Retrying in 2m".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        r.codex_files = Some(ProviderUsage {
            plan: Some("ChatGPT Plus".into()),
            primary: Some(UsageWindow {
                used_percent: 20.,
                window_minutes: 300,
                resets_at: 0,
            }),
            secondary: Some(UsageWindow {
                used_percent: 3.,
                window_minutes: 10_080,
                resets_at: 0,
            }),
            updated_at: (NOW - 3 * HOUR) / 1000,
            ..Default::default()
        });
        assert!(r.codex_needs_files());
        let p = r.picture(NOW);
        let c = p.codex.unwrap();
        let labels: Vec<_> = c.windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["Session · 5h", "This week"]);
        assert_eq!(c.note.as_deref(), Some("As of 3h ago."));
        assert_eq!(p.codex_missing, None);
    }

    #[test]
    fn the_ring_skips_spent_windows_unless_nothing_else_is_left() {
        let p = Picture {
            claude: Some(Block {
                agent: Agent::Claude,
                plan: None,
                updated_at: None,
                windows: vec![win(100., Some(HOUR), None), win(30., Some(HOUR), None)],
                note: None,
                extra: None,
            }),
            ..Default::default()
        };
        assert_eq!(p.worst(NOW).unwrap().pct, 30.);
        let spent = Picture {
            claude: Some(Block {
                windows: vec![win(100., Some(HOUR), None)],
                ..p.claude.clone().unwrap()
            }),
            ..Default::default()
        };
        assert_eq!(spent.worst(NOW).unwrap().pct, 100.);
    }
}
