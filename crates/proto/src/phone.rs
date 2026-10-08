// The phone: what a paired phone and the desktop say to each other over the bridge, and what
// the UI and the engine say about it. The engine runs the bridge (a TLS WebSocket the user
// switches on in Settings); the UI publishes the thread list as a `Board` and does what the
// phone asks the way a click would. The Android app in mobile/ mirrors these types, so a change
// here changes it too and moves `PROTOCOL`. Shape and reasons: docs/internals/phone.md.

use serde::{Deserialize, Serialize};

use crate::agents::{Agent, AgentInfo};
use crate::run::{Answer, Permission};
use crate::state::{Entry, Scheme, Snooze};

/// Moves when a message changes shape, so an old phone is told to update rather than misread.
pub const PROTOCOL: u32 = 2;

/// The phone to the desktop, one JSON text frame each.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Up {
    /// The first message from a phone that paired before.
    Hello {
        token: String,
        device: String,
        protocol: u32,
        /// The app's version and the commit it was built from, "0.24.4 (5321b19)".
        #[serde(default)]
        app: String,
    },
    /// The first message from a phone pairing now. The code from the desktop's QR or screen
    /// never travels: `proof` is HMAC-SHA256 keyed with the code over the certificate's
    /// fingerprint as the phone saw it, base64url. Someone in the middle shows another
    /// certificate, so their relay fails and they learn nothing they can use. Answered with
    /// `Welcome` carrying the phone's token, or `Denied`.
    Pair {
        proof: String,
        device: String,
        protocol: u32,
        #[serde(default)]
        app: String,
    },
    /// The phone forgot this computer: the computer forgets the phone too.
    Leave,
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
    /// A photo from the phone, base64. Answered with `Uploaded`, whose path a `Send` can carry.
    Upload {
        id: u64,
        data: String,
    },
    /// The folders inside `path` on the computer, empty for the home folder. Answered with
    /// `Folders`.
    Folders {
        path: String,
    },
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
        /// Photos the phone uploaded, by the paths `Uploaded` gave back.
        #[serde(default)]
        images: Vec<String>,
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
        /// A folder to start in instead of `space`. It becomes a space if it isn't one yet.
        #[serde(default)]
        folder: Option<String>,
        start: NewThread,
    },
    Snooze {
        thread: u64,
        until: Snooze,
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
        /// Answering a pairing, the computer's own proof that it knows the code: HMAC-SHA256
        /// keyed with the code over "desktop " and the fingerprint. Someone posing as the
        /// computer can't make it.
        #[serde(default)]
        proof: Option<String>,
    },
    /// The phone isn't let in. `message` is user-facing. With `forget`, the computer removed
    /// this phone, which forgets the computer in turn.
    Denied {
        message: String,
        #[serde(default)]
        forget: bool,
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
    Folders {
        path: String,
        parent: Option<String>,
        dirs: Vec<String>,
    },
    Uploaded {
        id: u64,
        path: Option<String>,
        error: Option<String>,
    },
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
    /// Someone used this computer's keyboard or mouse in the last two minutes, so the phone holds
    /// back its notifications. The engine fills it in; the UI's board leaves it false.
    pub present: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardSpace {
    pub id: u64,
    pub name: String,
    pub path: String,
    /// The fill and lettering of its tag, light side then dark, 0xRRGGBBAA.
    pub tag: Vec<u32>,
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
    /// The app version it last connected with.
    pub app: String,
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
                folder: None,
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

    /// One of every message, in the shapes the Android app reads and writes. The same lines sit
    /// in mobile/app/src/test/resources/wire, where the app's tests decode them, so a change on
    /// either side that the other misses fails a test. `HYPRSPACE_WRITE_FIXTURES=1` rewrites them.
    #[test]
    fn the_phone_reads_and_writes_these_exact_lines() {
        use crate::agents::{AgentCatalog, AgentInfo, ModelInfo, ProviderStatus};
        use crate::run::{ChangeKind, FileChange, Prompt, RunEvent, RunStatus, Tool};
        use crate::state::Entry;
        let tool = |kind: &str| match kind {
            "command" => Tool::Command {
                command: "cd \"C:\\w\" && npm test".into(),
            },
            "read" => Tool::Read {
                path: "src/a.ts".into(),
            },
            "edit" => Tool::Edit {
                changes: vec![FileChange {
                    path: "src/a.ts".into(),
                    kind: ChangeKind::Update,
                    diff: "@@\n-a\n+b".into(),
                }],
            },
            "search" => Tool::Search {
                pattern: "next".into(),
                path: None,
            },
            "web" => Tool::Web {
                target: "https://example.com".into(),
            },
            "mcp" => Tool::Mcp {
                server: "s".into(),
                tool: "t".into(),
                input: "{}".into(),
            },
            "agent" => Tool::Agent {
                description: "Look around".into(),
                agent_type: "general-purpose".into(),
                prompt: "Find it".into(),
            },
            _ => Tool::Other {
                name: "Todo".into(),
                input: "{\"a\":1}".into(),
            },
        };
        let run = |event| Entry::Run { event };
        let entries = vec![
            Entry::Prompt {
                prompt: Prompt {
                    text: "hi".into(),
                    images: vec!["C:\\a.png".into()],
                },
            },
            run(RunEvent::Started {
                agent: Agent::Claude,
                model: "claude-opus-5-5".into(),
                thread: "t".into(),
                cwd: "/w".into(),
            }),
            run(RunEvent::Text { text: "a".into() }),
            run(RunEvent::Thinking { text: "b".into() }),
            run(RunEvent::Tool {
                id: "1".into(),
                tool: tool("command"),
            }),
            run(RunEvent::ToolDone {
                id: "1".into(),
                ok: true,
                output: "ok".into(),
            }),
            run(RunEvent::SubagentTool {
                parent: "2".into(),
                id: "3".into(),
                tool: tool("read"),
            }),
            run(RunEvent::SubagentToolDone {
                parent: "2".into(),
                id: "3".into(),
                ok: false,
                output: "no".into(),
            }),
            run(RunEvent::SubagentText {
                parent: "2".into(),
                text: "c".into(),
            }),
            run(RunEvent::Woke),
            run(RunEvent::Approval {
                request: "r".into(),
                tool: tool("edit"),
                reason: Some("why".into()),
                always: true,
            }),
            Entry::Answer {
                request: "r".into(),
                answer: Answer::AllowAlways,
            },
            run(RunEvent::Steered),
            run(RunEvent::Limited {
                resets: Some(1_760_000_000_000),
            }),
            run(RunEvent::Usage {
                input: 1,
                output: 2,
            }),
            run(RunEvent::Context { used: 3, window: 4 }),
            run(RunEvent::Error {
                message: "e".into(),
            }),
            run(RunEvent::Finished {
                status: RunStatus::Interrupted,
                ms: 5,
                text: String::new(),
                error: None,
            }),
            run(RunEvent::Failed {
                message: "f".into(),
            }),
        ]
        .into_iter()
        .chain(
            ["search", "web", "mcp", "agent", "other"]
                .into_iter()
                .map(|k| {
                    run(RunEvent::Tool {
                        id: k.into(),
                        tool: tool(k),
                    })
                }),
        )
        .collect();
        let palette = Palette {
            bg: 0x161616ff,
            ansi: vec![0xff0000ff; 16],
            ..Default::default()
        };
        let down = [
            Down::Welcome {
                desktop: "Desk".into(),
                version: "0.25.0".into(),
                token: Some("tok".into()),
                proof: Some("aGk".into()),
            },
            Down::Denied {
                message: "no".into(),
                forget: true,
            },
            Down::Board {
                board: Box::new(Board {
                    spaces: vec![BoardSpace {
                        id: 1,
                        name: "w".into(),
                        path: "/w".into(),
                        tag: vec![1, 2, 3, 4],
                    }],
                    threads: vec![BoardThread {
                        id: 2,
                        space: 1,
                        title: "T".into(),
                        kind: BoardKind::Structured,
                        agent: Some(Agent::Codex),
                        model: Some("GPT-5.5".into()),
                        status: BoardStatus::Waiting,
                        doing: Some("Run tests".into()),
                        since: Some(9),
                        unseen: true,
                        place: "main".into(),
                        branch: true,
                        touched: 8,
                        shelf: Shelf::Snoozed,
                        rank: -3,
                        live: true,
                    }],
                    agents: vec![AgentInfo {
                        agent: Agent::Claude,
                        status: ProviderStatus {
                            id: "claude".into(),
                            installed: true,
                            ..Default::default()
                        },
                        catalog: AgentCatalog {
                            agent: Agent::Claude,
                            models: vec![ModelInfo {
                                id: "opus".into(),
                                label: "Opus".into(),
                                note: None,
                                efforts: vec!["high".into()],
                                default_effort: Some("high".into()),
                            }],
                            efforts: vec!["low".into()],
                        },
                    }],
                    start: StartPrefs {
                        agent: Some(Agent::Claude),
                        permission: Permission::Bypass,
                        picks: vec![(Agent::Claude, "opus".into(), "high".into())],
                        structured: true,
                    },
                    theme: BoardTheme {
                        scheme: crate::state::Scheme::Dark,
                        light: palette.clone(),
                        dark: palette,
                    },
                    present: true,
                }),
            },
            Down::Transcript {
                thread: 2,
                entries,
                reset: true,
            },
            Down::Term {
                frame: TermFrame {
                    thread: 3,
                    cols: 40,
                    rows: 20,
                    reset: true,
                    drop: 1,
                    len: 2,
                    lines: vec![(
                        1,
                        vec![
                            Span {
                                t: "ok".into(),
                                fg: Some(2),
                                bg: Some(Span::RGB | 0x102030),
                                s: Span::BOLD | Span::INVERSE,
                            },
                            Span {
                                t: " plain".into(),
                                ..Default::default()
                            },
                        ],
                    )],
                    cursor: Some((1, 4)),
                    fit: true,
                    paste: true,
                },
            },
            Down::Failed {
                thread: Some(2),
                message: "gone".into(),
            },
            Down::Pong,
            Down::Uploaded {
                id: 1,
                path: Some("C:\\tmp\\phone-1.jpg".into()),
                error: None,
            },
            Down::Folders {
                path: "C:\\Users\\ash".into(),
                parent: Some("C:\\Users".into()),
                dirs: vec!["C:\\Users\\ash\\code".into()],
            },
        ];
        let up = [
            Up::Hello {
                token: "tok".into(),
                device: "Pixel".into(),
                protocol: PROTOCOL,
                app: "0.24.4 (5321b19)".into(),
            },
            Up::Pair {
                proof: "aGk".into(),
                device: "Pixel".into(),
                protocol: PROTOCOL,
                app: "0.24.4 (5321b19)".into(),
            },
            Up::Watch { thread: 2 },
            Up::Unwatch { thread: 2 },
            Up::Fit {
                thread: 3,
                cols: 40,
                rows: 20,
            },
            Up::Unfit { thread: 3 },
            Up::Keys {
                thread: 3,
                text: "\u{1b}[A".into(),
            },
            Up::Paste {
                thread: 3,
                text: "a\nb".into(),
            },
            Up::Ask {
                ask: Ask::Send {
                    thread: 2,
                    text: "go".into(),
                    images: vec!["C:\\tmp\\phone-1.jpg".into()],
                },
            },
            Up::Ask {
                ask: Ask::Approve {
                    thread: 2,
                    request: "r".into(),
                    answer: Answer::Deny,
                },
            },
            Up::Ask {
                ask: Ask::Interrupt { thread: 2 },
            },
            Up::Ask {
                ask: Ask::Snooze {
                    thread: 2,
                    until: Snooze::Time {
                        at: 1_760_000_000_000,
                    },
                },
            },
            Up::Ask {
                ask: Ask::Snooze {
                    thread: 2,
                    until: Snooze::Done,
                },
            },
            Up::Ask {
                ask: Ask::New {
                    space: 0,
                    folder: Some("/home/ash/code".into()),
                    start: NewThread {
                        agent: None,
                        model: String::new(),
                        effort: String::new(),
                        permission: Permission::Plan,
                        terminal: true,
                        prompt: String::new(),
                    },
                },
            },
            Up::Ask {
                ask: Ask::Settle {
                    thread: 2,
                    on: false,
                },
            },
            Up::Leave,
            Up::Folders {
                path: String::new(),
            },
            Up::Upload {
                id: 1,
                data: "/9j/".into(),
            },
            Up::Ping,
        ];
        let lines = |all: Vec<String>| all.join("\n") + "\n";
        let down = lines(
            down.iter()
                .map(|d| serde_json::to_string(d).unwrap())
                .collect(),
        );
        let up_json = lines(
            up.iter()
                .map(|u| serde_json::to_string(u).unwrap())
                .collect(),
        );
        // every line reads back as what it was
        for (line, u) in up_json.lines().zip(&up) {
            assert_eq!(&serde_json::from_str::<Up>(line).unwrap(), u);
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../mobile/app/src/test/resources/wire");
        if std::env::var_os("HYPRSPACE_WRITE_FIXTURES").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("down.jsonl"), &down).unwrap();
            std::fs::write(dir.join("up.jsonl"), &up_json).unwrap();
        }
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name))
                .unwrap_or_default()
                .replace("\r\n", "\n")
        };
        assert_eq!(
            read("down.jsonl"),
            down,
            "the phone's fixtures are stale: run with HYPRSPACE_WRITE_FIXTURES=1 and update the app"
        );
        assert_eq!(read("up.jsonl"), up_json, "the phone's fixtures are stale");
    }
}
