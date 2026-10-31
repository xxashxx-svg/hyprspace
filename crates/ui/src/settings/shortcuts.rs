// Settings, Shortcuts: the keys and clicks the app answers to, read only. Each row matches a real
// binding: the palette's and the dock's in `palette/` and `workbench/`, the prompt box's in `input/`,
// Settings' own in `settings/`, and the terminal's in `terminal/` (its keys and its link click).
// Change one there, change it here. Settings search reads the same lines.

use gpui::{AnyElement, div, prelude::*};

use super::controls::{group, keys, row};
use super::{Root, index};
use crate::zoom::px;

/// Cmd on macOS, Ctrl elsewhere: GPUI's `secondary`, and the terminal's copy and link keys.
const MOD: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};
const ALT: &str = if cfg!(target_os = "macos") {
    "Option"
} else {
    "Alt"
};

pub(super) struct Line {
    pub(super) section: &'static str,
    pub(super) name: &'static str,
    pub(super) desc: &'static str,
    pub(super) keys: &'static [&'static [&'static str]],
}

const fn line(
    section: &'static str,
    name: &'static str,
    desc: &'static str,
    keys: &'static [&'static [&'static str]],
) -> Line {
    Line {
        section,
        name,
        desc,
        keys,
    }
}

const COPY: &[&[&str]] = if cfg!(target_os = "macos") {
    &[&[MOD, "C"]]
} else {
    &[&[MOD, "C"], &[MOD, "Shift", "C"]]
};
const COPY_DESC: &str = if cfg!(target_os = "macos") {
    ""
} else {
    "With nothing selected, Ctrl+C interrupts."
};

pub(super) const LINES: &[Line] = &[
    line(
        "App",
        "Command palette",
        "Works in a terminal too.",
        &[&[MOD, "K"]],
    ),
    line("App", "Command palette, again", "", &[&[MOD, "Shift", "P"]]),
    line("App", "Sidebar", "", &[&[MOD, "Shift", "B"]]),
    line(
        "App",
        "Open a thread by its place in the sidebar",
        if cfg!(target_os = "macos") {
            "Hold Cmd to see the numbers."
        } else {
            "Hold Ctrl to see the numbers."
        },
        &[&[MOD, "1-9"]],
    ),
    line(
        "App",
        "Next thread waiting on you",
        "",
        &[&[MOD, "Shift", "J"]],
    ),
    line("App", "Files and git", "", &[&[MOD, "Shift", "G"]]),
    line(
        "App",
        "Terminal under a structured thread",
        "",
        &[&["Ctrl", "`"]],
    ),
    line(
        "App",
        "Find in a structured thread",
        "Enter steps through matches.",
        &[&[MOD, "F"]],
    ),
    line(
        "App",
        "Your previous or next message",
        "In a structured thread.",
        &[&[ALT, "\u{2191}"], &[ALT, "\u{2193}"]],
    ),
    line(
        "App",
        "Edit the last queued message",
        "With the message box empty. It goes back in its place.",
        &[&["\u{2191}"]],
    ),
    line("App", "Zoom in", "", &[&[MOD, "+"]]),
    line("App", "Zoom out", "", &[&[MOD, "-"]]),
    line("App", "Actual size", "", &[&[MOD, "0"]]),
    line(
        "Settings",
        "Search settings",
        "/ works too.",
        &[&[MOD, "F"]],
    ),
    line("Settings", "Close Settings", "", &[&["Esc"]]),
    line(
        "Files and images",
        "Open a path from a terminal",
        "",
        &[&[MOD, "Click"]],
    ),
    line(
        "Files and images",
        "Zoom an image",
        "Drag to move. Double-click to fit.",
        &[&["Wheel"]],
    ),
    line(
        "Files and images",
        "Close a file or an image",
        "",
        &[&["Esc"]],
    ),
    line(
        "Prompt box",
        "Send",
        "Queues it while a run works.",
        &[&["Enter"]],
    ),
    line(
        "Prompt box",
        "Start a thread in the background",
        "In a new thread's box. It runs while you write the next one.",
        &[&[MOD, "Enter"]],
    ),
    line(
        "Prompt box",
        "Steer the run",
        "Sends it into the run now instead of queueing it.",
        &[&[MOD, "Enter"]],
    ),
    line(
        "Prompt box",
        "Send and start a new thread",
        "",
        &[&[MOD, ALT, "Enter"]],
    ),
    line("Prompt box", "New line", "", &[&["Shift", "Enter"]]),
    line("Prompt box", "Interrupt the run", "", &[&["Esc"]]),
    line("Prompt box", "Paste", "Images attach.", &[&[MOD, "V"]]),
    line(
        "Prompt box",
        "Screenshot the window you're in",
        "From any app, once switched on in General. Windows only.",
        &[&["Ctrl", "Alt", "S"]],
    ),
    line("Approvals and questions", "Allow", "", &[&[ALT, "Y"]]),
    line(
        "Approvals and questions",
        "Always allow",
        "When the agent offers it.",
        &[&[ALT, "A"]],
    ),
    line(
        "Approvals and questions",
        "Deny",
        "Sending a message denies too.",
        &[&[ALT, "N"]],
    ),
    line(
        "Approvals and questions",
        "Pick a question's answer",
        "",
        &[&[ALT, "1-9"]],
    ),
    line("Terminal", "Copy", COPY_DESC, COPY),
    line("Terminal", "Paste", "", &[&[MOD, "V"]]),
    line(
        "Terminal",
        "Paste an image",
        "Types the image's path.",
        &[&[ALT, "V"]],
    ),
    line(
        "Terminal",
        "Find",
        "Enter steps through matches.",
        &[&[MOD, "F"]],
    ),
    line("Terminal", "Open a link or file", "", &[&[MOD, "Click"]]),
];

impl Root {
    pub(super) fn shortcuts_page(&self) -> AnyElement {
        let groups = LINES.chunk_by(|a, b| a.section == b.section).map(|lines| {
            let rows = lines
                .iter()
                .map(|l| {
                    let caps = div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(l.keys.iter().map(|k| keys(k)));
                    self.spot(index::line_spot(l), row(l.name, l.desc, caps), false)
                })
                .collect();
            group(lines[0].section, rows)
        });
        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .children(groups)
            .into_any_element()
    }
}
