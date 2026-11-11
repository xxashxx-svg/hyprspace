// Every fixed row in Settings, for search: where it sits, its name and the line under it, other
// words people type for it, and the saved values its reset puts back. Rows take their name and
// line from here, so search and the page can't disagree. Rows built from data (threads, phones,
// computers) aren't listed; their page and section are.

use std::cmp::Reverse;
use std::ops::Range;

use hyprspace_proto::Agent;

use super::defaults::Field;
use super::shortcuts::{LINES, Line};
use super::{TABS, Tab};

pub(crate) struct Setting {
    pub(crate) tab: Tab,
    pub(crate) section: &'static str,
    pub(crate) title: &'static str,
    pub(crate) desc: &'static str,
    terms: &'static str,
    pub(crate) fields: &'static [Field],
}

/// Where a search hit lands: a page, a section on it, or one row. A whole page has no section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spot {
    pub(crate) tab: Tab,
    pub(crate) section: &'static str,
    pub(crate) title: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub(crate) spot: Spot,
    pub(crate) keys: &'static [&'static str],
}

impl Setting {
    pub(crate) fn spot(&self) -> Spot {
        Spot {
            tab: self.tab,
            section: self.section,
            title: self.title,
        }
    }
}

pub(super) fn line_spot(l: &Line) -> Spot {
    Spot {
        tab: Tab::Shortcuts,
        section: l.section,
        title: l.name,
    }
}

const fn setting(
    tab: Tab,
    section: &'static str,
    title: &'static str,
    desc: &'static str,
    terms: &'static str,
    fields: &'static [Field],
) -> Setting {
    Setting {
        tab,
        section,
        title,
        desc,
        terms,
        fields,
    }
}

use Tab::{About, Agents, Appearance, Computers, General, Phone};

pub(crate) static AGENT: Setting = setting(
    General,
    "New threads",
    "Agent",
    "What new threads start with.",
    "claude codex default",
    &[],
);
pub(crate) static PERMISSION: Setting = setting(
    General,
    "New threads",
    "Permission",
    "How much a new thread may do without asking.",
    "mode plan ask auto edit bypass full access approve",
    &[Field::Permission],
);
pub(crate) static SETTLE: Setting = setting(
    General,
    "Threads",
    "Settle untouched threads",
    "Idle threads move to Settled. Open or working ones stay.",
    "archive idle days week auto",
    &[Field::Settle],
);
pub(crate) static TITLES: Setting = setting(
    General,
    "Threads",
    "Name new threads",
    "Sends a new thread's first message to a small model on your plan for a short title: Haiku for Claude, GPT Luna for Codex.",
    "title rename",
    &[Field::Titles],
);
pub(crate) static SNAPSHOTS: Setting = setting(
    General,
    "Threads",
    "Screenshot shortcut",
    if cfg!(windows) {
        "Ctrl+Alt+S, from any app, captures the window you're in and attaches it to the thread on screen."
    } else {
        "Only on Windows for now."
    },
    "capture screenshot hotkey snapshot image",
    &[],
);
pub(crate) static AGENT_CHECKS: Setting = setting(
    General,
    "Updates",
    "Check for agent updates",
    "When HyprSpace checks for its own update, it also asks registry.npmjs.org for the newest Claude Code and Codex. Only the two package names are sent. Nothing updates until you click Update.",
    "npm version cli upgrade claude codex",
    &[],
);
pub(crate) static SNOOZED: Setting = setting(
    General,
    "Snoozed",
    "Snoozed threads",
    "Every snoozed thread and when it wakes.",
    "snooze wake later",
    &[],
);
pub(crate) static STRUCTURED: Setting = setting(
    General,
    "Experimental",
    "Structured threads",
    "Adds a Structured button to the composer. Still being built.",
    "transcript experimental",
    &[Field::Structured],
);
pub(crate) static NOTIFICATIONS: Setting = setting(
    General,
    "Experimental",
    "Notifications",
    "Tells you when a thread waits on you, fails or finishes, with a system notification, a toast and a count on the taskbar.",
    "notify alert toast badge taskbar dock sound",
    &[],
);
pub(crate) static WORKING: Setting = setting(
    General,
    "Experimental",
    "Working section",
    "Threads move to a Working section in the sidebar while their agent works, and come back to the top when they finish or need you.",
    "sidebar shelf busy running",
    &[Field::Working],
);
pub(crate) static OPENER: Setting = setting(
    General,
    "Folders",
    "Open folders in",
    "Where the Open button goes.",
    "editor vs code cursor explorer finder files",
    &[],
);
pub(crate) static CLEANUP: Setting = setting(
    General,
    "Folders",
    "Clean up worktrees",
    "Removes a worktree's folder once every thread in it has been settled this long, if nothing in it is uncommitted. The branch stays, and the folder comes back when you use the thread again.",
    "delete remove disk space target shelve",
    &[],
);
pub(crate) static SCREEN: Setting = setting(
    General,
    "This Mac",
    "Screen recording",
    "Lets agents, and anything run in a HyprSpace terminal, see the whole screen. macOS may ask you to reopen HyprSpace.",
    "permission capture screenshot",
    &[],
);
pub(crate) static ACCESSIBILITY: Setting = setting(
    General,
    "This Mac",
    "Accessibility",
    "Lets them click, type and read any app on this Mac.",
    "permission click type control",
    &[],
);

pub(crate) static THEME: Setting = setting(
    Appearance,
    "Theme",
    "Theme",
    "Seven themes, each with a light and a dark side.",
    "color colour dark light system scheme mode accent orchid grove ocean ember iris",
    &[Field::Theme, Field::Scheme],
);
pub(crate) static PICTURE: Setting = setting(
    Appearance,
    "Background",
    "Picture",
    "Shows behind the new thread screen.",
    "wallpaper image photo built in",
    &[],
);
pub(crate) static PICTURE_ZOOM: Setting = setting(
    Appearance,
    "Background",
    "Zoom",
    "",
    "wallpaper picture scale",
    &[],
);
pub(crate) static EFFECT: Setting = setting(
    Appearance,
    "Background",
    "Effect",
    "",
    "wallpaper picture dither ascii halftone scanlines",
    &[],
);
pub(crate) static WALLPAPERS: Setting = setting(
    Appearance,
    "Background",
    "Wallpaper folder",
    "Shuffle picks from it, or from the built in ones without one.",
    "shuffle random pictures",
    &[],
);
pub(crate) static GLASS: Setting = setting(
    Appearance,
    "Background",
    "Glass",
    "The window lets the desktop show through, blurred.",
    "transparent blur mica acrylic translucent",
    &[Field::Glass],
);
pub(crate) static SIDEBAR_ART: Setting = setting(
    Appearance,
    "Sidebar",
    "Picture",
    "Fades into the top of the sidebar.",
    "image art header",
    &[],
);
pub(crate) static UI_FONT: Setting = setting(
    Appearance,
    "Interface",
    "Font",
    "",
    "typeface geist system",
    &[Field::UiFont],
);
pub(crate) static DIFF: Setting = setting(
    Appearance,
    "Interface",
    "Diff colors",
    "",
    "colour blind red green blue orange changes",
    &[Field::Diff],
);
pub(crate) static ANIMATIONS: Setting = setting(
    Appearance,
    "Interface",
    "Animations",
    "",
    "motion reduce transitions",
    &[Field::Animations],
);
pub(crate) static TERM_FONT: Setting = setting(
    Appearance,
    "Terminal",
    "Font",
    "Nerd Fonts show prompt icons.",
    "monospace typeface nerd jetbrains",
    &[Field::TermFont],
);
pub(crate) static TERM_SIZE: Setting = setting(
    Appearance,
    "Terminal",
    "Font size",
    "",
    "text bigger smaller",
    &[Field::TermSize],
);
pub(crate) static LINE_HEIGHT: Setting = setting(
    Appearance,
    "Terminal",
    "Line height",
    "",
    "spacing rows leading",
    &[Field::LineHeight],
);

pub(crate) static CLAUDE: Setting = setting(
    Agents,
    "Installed",
    "Claude",
    "The model and effort new Claude threads start with.",
    "opus sonnet haiku model effort thinking",
    &[Field::Pick(Agent::Claude)],
);
pub(crate) static CODEX: Setting = setting(
    Agents,
    "Installed",
    "Codex",
    "The model and effort new Codex threads start with.",
    "gpt openai model effort reasoning",
    &[Field::Pick(Agent::Codex)],
);

pub(crate) static PHONE_ON: Setting = setting(
    Phone,
    "Access",
    "Let your phone connect",
    "Direct and encrypted, no server. Works while HyprSpace is open.",
    "mobile android bridge remote",
    &[],
);
pub(crate) static NETWORKS: Setting = setting(
    Phone,
    "Access",
    "Networks",
    "Wi-Fi at home, Tailscale away.",
    "wifi tailscale tailnet lan anywhere relay iroh n0 remote",
    &[],
);
pub(crate) static PHONES: Setting = setting(
    Phone,
    "Phones",
    "Phones",
    "Pair a phone, and the phones paired so far.",
    "pair qr code add revoke android",
    &[],
);
pub(crate) static PHONE_CAN: Setting = setting(
    Phone,
    "The app",
    "What it can do",
    "See your threads, terminals and files, send messages, answer approvals, start threads, and commit and push.",
    "mobile",
    &[],
);
pub(crate) static PHONE_SECURITY: Setting = setting(
    Phone,
    "The app",
    "Security",
    "TLS pinned to this computer's certificate. The pairing code is never sent.",
    "tls certificate encryption",
    &[],
);

pub(crate) static PEERS: Setting = setting(
    Computers,
    "Computers you can use",
    "Computers you can use",
    "The computers paired with this one.",
    "remote machine forget",
    &[],
);
pub(crate) static ADD_PEER: Setting = setting(
    Computers,
    "Add a computer",
    "Add a computer",
    "Paste the link another computer shows, or type its code.",
    "remote machine pair link code",
    &[],
);
pub(crate) static HOST: Setting = setting(
    Computers,
    "Let other computers use this one",
    "Let other computers use this one",
    "Shows a code to type on the other computer.",
    "remote machine host share pair",
    &[],
);

pub(crate) static VERSION: Setting = setting(
    About,
    "HyprSpace",
    "HyprSpace",
    "The version and its updates.",
    "version update upgrade release check",
    &[],
);
pub(crate) static INTRO: Setting = setting(
    About,
    "HyprSpace",
    "Intro",
    "",
    "tour onboarding welcome replay",
    &[],
);
pub(crate) static SOURCE: Setting = setting(
    About,
    "Links",
    "Source code",
    "github.com/xxashxx-svg/hyprspace",
    "github repo",
    &[],
);
pub(crate) static REPORT: Setting = setting(
    About,
    "Links",
    "Report a problem",
    "",
    "bug issue feedback github",
    &[],
);

/// In page order, so equal matches list the way the pages do.
static SETTINGS: &[&Setting] = &[
    &AGENT,
    &PERMISSION,
    &SETTLE,
    &SNAPSHOTS,
    &TITLES,
    &AGENT_CHECKS,
    &SNOOZED,
    &STRUCTURED,
    &NOTIFICATIONS,
    &WORKING,
    &OPENER,
    &CLEANUP,
    &SCREEN,
    &ACCESSIBILITY,
    &THEME,
    &PICTURE,
    &PICTURE_ZOOM,
    &EFFECT,
    &WALLPAPERS,
    &GLASS,
    &SIDEBAR_ART,
    &UI_FONT,
    &DIFF,
    &ANIMATIONS,
    &TERM_FONT,
    &TERM_SIZE,
    &LINE_HEIGHT,
    &CLAUDE,
    &CODEX,
    &PHONE_ON,
    &NETWORKS,
    &PHONES,
    &PHONE_CAN,
    &PHONE_SECURITY,
    &PEERS,
    &ADD_PEER,
    &HOST,
    &VERSION,
    &INTRO,
    &SOURCE,
    &REPORT,
];

/// The settings this computer shows.
pub(super) fn settings() -> impl Iterator<Item = &'static Setting> {
    SETTINGS
        .iter()
        .copied()
        .filter(|s| cfg!(target_os = "macos") || s.section != "This Mac")
}

pub(crate) fn tab_label(tab: Tab) -> &'static str {
    TABS.iter().find(|e| e.tab == tab).map_or("", |e| e.label)
}

/// Every word has to appear in the name, section, page, line or other words. Names that match
/// best come first, and shortcuts come after settings.
pub(crate) fn search(query: &str) -> Vec<Hit> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let phrase = words.join(" ");
    let pages = TABS.iter().map(|e| {
        let spot = Spot {
            tab: e.tab,
            section: "",
            title: e.label,
        };
        (spot, &[][..], e.desc.to_string(), false)
    });
    let rows = settings().map(|s| (s.spot(), &[][..], format!("{} {}", s.desc, s.terms), false));
    let lines = LINES.iter().map(|l| {
        let keys = l.keys.iter().map(|k| k.join(" ")).collect::<Vec<_>>();
        let more = format!("{} shortcut key {}", l.desc, keys.join(" "));
        (line_spot(l), l.keys[0], more, true)
    });
    let mut ranked: Vec<(i32, usize, Hit)> = pages
        .chain(rows)
        .chain(lines)
        .enumerate()
        .filter_map(|(i, (spot, keys, more, shortcut))| {
            let title = spot.title.to_lowercase();
            let hay =
                format!("{title} {} {} {more}", spot.section, tab_label(spot.tab)).to_lowercase();
            if !words.iter().all(|w| hay.contains(w)) {
                return None;
            }
            let rank = if title == phrase {
                5
            } else if title.starts_with(&phrase) {
                4
            } else if title.contains(&phrase) {
                3
            } else if words.iter().all(|w| title.contains(w)) {
                2
            } else {
                1
            };
            let rank = if shortcut { rank - 10 } else { rank };
            Some((rank, i, Hit { spot, keys }))
        })
        .collect();
    ranked.sort_by_key(|&(rank, i, _)| (Reverse(rank), i));
    ranked.into_iter().map(|(_, _, hit)| hit).collect()
}

/// The parts of `title` the query's words match, for drawing them brighter.
pub(crate) fn matched(title: &str, query: &str) -> Vec<Range<usize>> {
    let hay = title.to_ascii_lowercase();
    let mut lit = vec![false; hay.len()];
    for w in query.to_ascii_lowercase().split_whitespace() {
        for (at, _) in hay.match_indices(w) {
            lit[at..at + w.len()].fill(true);
        }
    }
    let mut out: Vec<Range<usize>> = Vec::new();
    for (i, on) in lit.into_iter().enumerate() {
        match out.last_mut() {
            Some(r) if on && r.end == i => r.end = i + 1,
            _ if on => out.push(i..i + 1),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(q: &str) -> Vec<(&'static str, &'static str)> {
        search(q)
            .into_iter()
            .map(|h| (h.spot.section, h.spot.title))
            .collect()
    }

    #[test]
    fn every_spot_is_one_place() {
        let mut spots: Vec<Spot> = SETTINGS.iter().map(|s| s.spot()).collect();
        spots.extend(LINES.iter().map(line_spot));
        for (i, a) in spots.iter().enumerate() {
            assert!(!spots[i + 1..].contains(a), "{a:?} is listed twice");
        }
    }

    #[test]
    fn names_that_match_come_first() {
        let font = titles("font");
        assert_eq!(
            font[..3],
            [
                ("Interface", "Font"),
                ("Terminal", "Font"),
                ("Terminal", "Font size")
            ]
        );
        assert_eq!(titles("phone")[0], ("", "Phone"));
    }

    #[test]
    fn other_words_find_a_setting() {
        assert_eq!(titles("dark")[0], ("Theme", "Theme"));
        assert_eq!(titles("snooze")[0], ("Snoozed", "Snoozed threads"));
        assert!(titles("opus").contains(&("Installed", "Claude")));
    }

    #[test]
    fn shortcuts_come_after_settings() {
        let zoom = titles("zoom");
        assert_eq!(zoom[0], ("Background", "Zoom"));
        assert!(zoom.contains(&("App", "Zoom in")));
        let hit = search("command palette")[0];
        assert_eq!(hit.spot.title, "Command palette");
        assert!(!hit.keys.is_empty());
    }

    #[test]
    fn every_word_has_to_match() {
        assert!(search("font zebra").is_empty());
        assert!(search("   ").is_empty());
        assert_eq!(titles("terminal size")[0], ("Terminal", "Font size"));
    }

    #[test]
    fn matches_are_marked_where_they_are() {
        assert_eq!(matched("Font size", "size fo"), vec![0..2, 5..9]);
        assert_eq!(matched("Zoom", "oo o"), vec![1..3]);
        assert!(matched("Glass", "").is_empty());
    }
}
