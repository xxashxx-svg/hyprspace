// Settings, Shortcuts: the keys and clicks the app answers to, read only. Each row matches a real
// binding: the palette's and the dock's in `palette/` and `workbench/`, the prompt box's in `input/`,
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
        ""
    } else {
        "With nothing selected, Ctrl+C interrupts."
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
                    "Works in a terminal too.",
                    &[chord(&[MOD, "K"])],
                ),
                line("Command palette, again", "", &[chord(&[MOD, "Shift", "P"])]),
                line("Sidebar", "", &[chord(&[MOD, "Shift", "B"])]),
                line("Files and git", "", &[chord(&[MOD, "Shift", "G"])]),
                line("Close Settings", "", &["Esc".into()]),
            ],
        ))
        .child(group(
            "Files and images",
            vec![
                line("Open a path from a terminal", "", &[chord(&[MOD, "Click"])]),
                line(
                    "Zoom an image",
                    "Drag to move. Double-click to fit.",
                    &["Wheel".into()],
                ),
                line("Close a file or an image", "", &["Esc".into()]),
            ],
        ))
        .child(group(
            "Prompt box",
            vec![
                line("Send", "Queues it while a run works.", &["Enter".into()]),
                line("New line", "", &[chord(&["Shift", "Enter"])]),
                line("Interrupt the run", "", &["Esc".into()]),
                line("Paste", "Images attach.", &[chord(&[MOD, "V"])]),
            ],
        ))
        .child(group(
            "Terminal",
            vec![
                line("Copy", copy_desc, &copy),
                line("Paste", "", &[chord(&[MOD, "V"])]),
                line(
                    "Paste an image",
                    "Types the image's path.",
                    &[chord(&[ALT, "V"])],
                ),
                line(
                    "Find",
                    "Enter steps through matches.",
                    &[chord(&[MOD, "F"])],
                ),
                line("Open a link or file", "", &[chord(&[MOD, "Click"])]),
            ],
        ))
        .into_any_element()
}
