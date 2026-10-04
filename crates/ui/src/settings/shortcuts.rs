// Settings, Shortcuts: the keys and clicks the app answers to, read only. Each row matches a real
// binding: the palette's and the dock's in `palette/` and `panes/`, the prompt box's in `input/`,
// and the terminal's in `terminal/` (its keys and its link click). Change one there, change it
// here.

use gpui::{AnyElement, div, prelude::*, px};

use super::controls::{group, keys, row};

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

fn chord(parts: &[&str]) -> String {
    parts.join("+")
}

fn line(name: &str, desc: &str, chords: &[String]) -> AnyElement {
    let caps = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .children(chords.iter().map(|c| keys(c)));
    row(name.to_string(), desc.to_string(), caps)
}

pub(super) fn page() -> AnyElement {
    let copy = if cfg!(target_os = "macos") {
        vec![chord(&[MOD, "C"])]
    } else {
        vec![chord(&[MOD, "C"]), chord(&[MOD, "Shift", "C"])]
    };
    let copy_desc = if cfg!(target_os = "macos") {
        "Copies the selected text."
    } else {
        "With nothing selected, Ctrl+C interrupts the program as usual."
    };
    div()
        .flex()
        .flex_col()
        .gap(px(28.))
        .child(group(
            "App",
            vec![
                line(
                    "Command palette",
                    "Threads, layouts, themes and settings. Works inside a terminal too.",
                    &[chord(&[MOD, "K"])],
                ),
                line(
                    "Command palette, the other way",
                    "Opens the same palette.",
                    &[chord(&[MOD, "Shift", "P"])],
                ),
                line(
                    "Sidebar",
                    "Shows or hides the list of folders and threads.",
                    &[chord(&[MOD, "Shift", "B"])],
                ),
                line(
                    "Files and git",
                    "Shows or hides the dock on the right.",
                    &[chord(&[MOD, "Shift", "G"])],
                ),
                line(
                    "Close Settings",
                    "Goes back to where you were.",
                    &["Esc".into()],
                ),
            ],
        ))
        .child(group(
            "Threads and panes",
            vec![
                line(
                    "Open a thread beside the others",
                    "On a thread in the sidebar. A plain click replaces the focused pane.",
                    &[chord(&[MOD, "Click"])],
                ),
                line(
                    "Maximize or restore a pane",
                    "On the pane's header.",
                    &["Double-click".into()],
                ),
                line(
                    "Swap two panes",
                    "Drag a pane's header onto another pane.",
                    &["Drag".into()],
                ),
            ],
        ))
        .child(group(
            "Prompt box",
            vec![
                line(
                    "Send",
                    "While a run is going, the prompt joins it.",
                    &["Enter".into()],
                ),
                line(
                    "New line",
                    "Adds a line instead of sending.",
                    &[chord(&["Shift", "Enter"])],
                ),
                line(
                    "Interrupt the run",
                    "In a structured thread. The agent stops where it is.",
                    &["Esc".into()],
                ),
                line(
                    "Paste",
                    "Text, or an image to attach.",
                    &[chord(&[MOD, "V"])],
                ),
            ],
        ))
        .child(group(
            "Terminal",
            vec![
                line("Copy", copy_desc, &copy),
                line(
                    "Paste",
                    "Pastes the clipboard's text.",
                    &[chord(&[MOD, "V"])],
                ),
                line(
                    "Paste an image",
                    "Saves the clipboard's image and types its path, for agents that read images.",
                    &[chord(&[ALT, "V"])],
                ),
                line(
                    "Find",
                    "Enter and Shift+Enter step through the matches.",
                    &[chord(&[MOD, "F"])],
                ),
                line(
                    "Open a link or file",
                    "Files open in the viewer, links in your browser.",
                    &[chord(&[MOD, "Click"])],
                ),
            ],
        ))
        .into_any_element()
}
