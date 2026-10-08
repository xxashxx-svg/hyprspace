// The phone: what a paired phone and the desktop say to each other over the bridge, and what
// the UI and the engine say about it. The engine runs the bridge (a TLS WebSocket the user
// switches on in Settings); the UI publishes the thread list as a `Board` and does what the
// phone asks the way a click would. The Android app in mobile/ mirrors these types, so a change
// here changes it too and moves `PROTOCOL`. Shape and reasons: docs/internals/phone.md.

use serde::{Deserialize, Serialize};

use crate::agents::{Agent, AgentInfo};
use crate::run::{Answer, Permission};
use crate::state::{Entry, Scheme};

/// Moves when a message changes shape, so an old phone is told to update rather than misread.
pub const PROTOCOL: u32 = 1;

/// The phone to the desktop, one JSON text frame each.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Up {
    /// The first message from a phone that paired before.
    Hello {
        token: String,
        device: String,
        protocol: u32,
    },
    /// The first message from a phone pairing now, with the code from the desktop's QR or
    /// screen. Answered with `Welcome` carrying the phone's token, or `Denied`.
    Pair {
        code: String,
        device: String,
        protocol: u32,
    },
    /// Stream one thread: its transcript, or its terminal's screen.
    Watch {
        thread: u64,
    },
    Unwatch {
        thread: u64,
    },
    /// Size the thread's terminal for the phone. The desktop keeps showing it, narrower, until
    /// someone types there; `Unfit` or leaving gives it back.
    Fit {
        thread: u64,
        cols: u16,
        rows: u16,
    },
    Unfit {
        thread: u64,
    },
    /// Keystrokes for a terminal thread, as the bytes a terminal would send.
    Keys {
        thread: u64,
        text: String,
    },
    /// Text for a terminal thread, sent as a paste when the program there asked for pastes.
    Paste {
        thread: u64,
        text: String,
    },
    /// Something the desktop does as if it was clicked there.
    Ask {
        ask: Ask,
    },
    Ping,
}

/// What the phone asks the desktop's UI to do. Each one goes through the same code as the
/// click it stands for, so the desktop shows it too.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Ask {
    /// A message to a structured thread.
    Send {
        thread: u64,
        text: String,
    },
    Approve {
        thread: u64,
        request: String,
        answer: Answer,
    },
    Interrupt {
        thread: u64,
    },
    /// A new thread in `space`.
    New {
        space: u64,
        start: NewThread,
    },
    /// Settles a thread, or brings it back.
    Settle {
        thread: u64,
        on: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewThread {
    /// None opens a plain shell.
    pub agent: Option<Agent>,
    /// Empty means the CLI's own default.
    pub model: String,
    pub effort: String,
    pub permission: Permission,
    /// Run the agent in a terminal session rather than a structured one.
    pub terminal: bool,
    pub prompt: String,
}

/// The desktop to the phone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Down {
    /// The phone is in. `token` comes once, right after pairing, for the phone to keep.
    Welcome {
        desktop: String,
        version: String,
        token: Option<String>,
    },
    /// The phone isn't let in. `message` is user-facing.
    Denied {
        message: String,
    },
    Board {
        board: Box<Board>,
    },
    /// A structured thread's journal. `reset` starts it over (the first batch after `Watch`);
    /// otherwise the entries follow the ones sent before.
    Transcript {
        thread: u64,
        entries: Vec<Entry>,
        reset: bool,
    },
    Term {
        frame: TermFrame,
    },
    /// Something the phone asked for didn't happen. `message` is user-facing.
    Failed {
        thread: Option<u64>,
        message: String,
    },
    Pong,
}

/// Everything the phone's home screen shows: the sidebar, what can start a thread, and the
/// colors to draw it in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Board {
    pub spaces: Vec<BoardSpace>,
    /// In sidebar order within each space: highest rank first.
    pub threads: Vec<BoardThread>,
    pub agents: Vec<AgentInfo>,
    pub start: StartPrefs,
    pub theme: BoardTheme,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardSpace {
    pub id: u64,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardThread {
    pub id: u64,
    pub space: u64,
    pub title: String,
    pub kind: BoardKind,
    pub agent: Option<Agent>,
    /// The model's label, as the sidebar shows it.
    pub model: Option<String>,
    pub status: BoardStatus,
    /// One line on what it does now, or what it said last.
    pub doing: Option<String>,
    /// When the current turn began, unix ms, while it works or waits.
    pub since: Option<u64>,
    /// Finished while nobody was looking.
    pub unseen: bool,
    /// The branch, or the folder's name when it isn't a repo.
    pub place: String,
    pub branch: bool,
    /// When it last did something, unix ms.
    pub touched: u64,
    pub shelf: Shelf,
    /// Higher is nearer the top.
    pub rank: i64,
    /// Its session is running on the desktop.
    pub live: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoardKind {
    /// An agent CLI, or a plain shell, in a terminal.
    #[default]
    Terminal,
    Structured,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoardStatus {
    #[default]
    Idle,
    Working,
    /// Blocked on the user.
    Waiting,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Shelf {
    #[default]
    Active,
    Settled,
    Snoozed,
}

/// How a new thread starts unless the phone changes it: the composer's last picks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StartPrefs {
    pub agent: Option<Agent>,
    pub permission: Permission,
    /// Per agent: the model and effort last picked. Empty means the CLI's default.
    pub picks: Vec<(Agent, String, String)>,
    /// Structured sessions are switched on in Settings.
    pub structured: bool,
}

/// The desktop's theme, both sides, so the phone looks like it and follows its own light or
/// dark setting unless the desktop pins one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardTheme {
    pub scheme: Scheme,
    pub light: Palette,
    pub dark: Palette,
}

/// A theme's colors as 0xRRGGBBAA, the way `hyprspace_theme::Color` keeps them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Palette {
    pub bg: u32,
    pub surface1: u32,
    pub surface2: u32,
    pub surface3: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub link: u32,
    pub text1: u32,
    pub text2: u32,
    pub text3: u32,
    pub border0: u32,
    pub border1: u32,
    pub border2: u32,
    pub ink: u32,
    pub busy: u32,
    pub waiting: u32,
    pub ok: u32,
    pub error: u32,
    pub diff_add: u32,
    pub diff_del: u32,
    pub term_bg: u32,
    pub term_fg: u32,
    pub cursor: u32,
    pub ansi: Vec<u32>,
}

/// A terminal thread's screen and scrollback, as changes to the phone's copy. The phone drops
/// `drop` lines from the top, cuts or pads its copy to `len` lines, then puts each line in
/// `lines` at its index. Lines count from the oldest line of scrollback.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TermFrame {
    pub thread: u64,
    pub cols: u16,
    pub rows: u16,
    /// Start from an empty copy.
    pub reset: bool,
    pub drop: u32,
    pub len: u32,
    pub lines: Vec<(u32, Vec<Span>)>,
    /// The cursor's line and column, when it shows.
    pub cursor: Option<(u32, u16)>,
    /// The terminal is sized for the phone right now.
    pub fit: bool,
    /// Whether the program there asked for pastes to be marked (bracketed paste).
    pub paste: bool,
}

/// A run of cells drawn alike. Colors: 0 to 255 are the terminal's palette (0 to 15 the theme's
/// own sixteen), `RGB | 0xRRGGBB` a color of its own, and None the default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub t: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<u32>,
    /// `BOLD`, `ITALIC` and the rest, or'd together.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub s: u8,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl Span {
    pub const RGB: u32 = 1 << 24;
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
    pub const INVERSE: u8 = 8;
    pub const DIM: u8 = 16;
    pub const STRIKE: u8 = 32;
    pub const HIDDEN: u8 = 64;
}

/// Between the UI and the engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PhoneCommand {
    /// Switch the bridge on or off, and pick where it listens. Answered with `Status`.
    Enable { on: bool, network: Network },
    /// The thread list changed. The engine passes it on to every phone.
    Board { board: Box<Board> },
    /// Show a pairing code for the next few minutes. Answered with `Pairing`.
    Pair,
    /// Stop showing it.
    StopPairing,
    /// Unpair a phone. Its connection drops and its token stops working.
    Forget { device: String },
    /// The user typed into a terminal the phone had sized: it goes back to the desktop's size.
    Take { id: crate::wire::SessionId },
    /// Which session each thread runs in now, so the engine can match its journals and PTYs to
    /// the threads a phone watches.
    Sessions {
        sessions: Vec<(u64, crate::wire::SessionId)>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PhoneEvent {
    Status {
        status: PhoneStatus,
    },
    /// What the pairing screen shows: the link the QR code holds and the code to type instead.
    Pairing {
        pairing: Option<Pairing>,
    },
    /// A phone asked for something; the UI does it.
    Ask {
        ask: Ask,
    },
    /// The threads phones are watching. The UI starts the session of any that isn't running.
    Watching {
        threads: Vec<u64>,
    },
    /// Who sizes a terminal now: the phone, or the desktop again.
    Fit {
        id: crate::wire::SessionId,
        phone: bool,
    },
}

/// Where the bridge listens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Network {
    /// Every network this computer is on, Tailscale included.
    #[default]
    Everywhere,
    /// Only Tailscale's addresses.
    Tailscale,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PhoneStatus {
    pub on: bool,
    pub port: u16,
    /// The addresses a phone can reach, best first.
    pub addresses: Vec<String>,
    /// Why it isn't listening. User-facing.
    pub error: Option<String>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    /// unix ms
    pub paired: u64,
    pub seen: u64,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pairing {
    /// `hyprspace://pair?...`, what the QR code holds.
    pub link: String,
    /// The same secret, short enough to type.
    pub code: String,
    /// unix ms
    pub expires: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_survive_json_with_short_spans() {
        let up = Up::Ask {
            ask: Ask::New {
                space: 3,
                start: NewThread {
                    agent: Some(Agent::Claude),
                    model: "opus".into(),
                    effort: String::new(),
                    permission: Permission::Ask,
                    terminal: true,
                    prompt: "fix it".into(),
                },
            },
        };
        let json = serde_json::to_string(&up).unwrap();
        assert!(json.contains(r#""type":"ask""#), "{json}");
        assert_eq!(serde_json::from_str::<Up>(&json).unwrap(), up);
        let frame = Down::Term {
            frame: TermFrame {
                thread: 1,
                len: 2,
                lines: vec![(
                    1,
                    vec![Span {
                        t: "hi".into(),
                        fg: Some(2),
                        ..Default::default()
                    }],
                )],
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains(r#"[1,[{"t":"hi","fg":2}]]"#), "{json}");
        assert_eq!(serde_json::from_str::<Down>(&json).unwrap(), frame);
    }
}
