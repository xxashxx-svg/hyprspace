// Settings, General: which build this is, how new sessions start, and where the app keeps its
// data.

use std::path::PathBuf;

use gpui::{AnyElement, ClickEvent, Context, div, prelude::*, px};
use hyprspace_theme::MONO;

use super::{VERSION, group, row};
use crate::root::Root;
use crate::{colors, widgets};

/// The engine's `persist::state_dir`, worked out the same way. The channel has no request for it
/// yet, and the rule is three lines, so the UI repeats it rather than guess.
fn state_dir() -> PathBuf {
    if let Ok(d) = std::env::var("HYPRSPACE_STATE_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".hyprspace").join("native")
}

impl Root {
    pub(super) fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        let terminal = self.state.composer.terminal;
        let kinds = [
            (false, "Structured", "sparkles"),
            (true, "Terminal", "terminal"),
        ];
        let start = widgets::segments().children(kinds.into_iter().enumerate().map(
            |(i, (value, name, glyph))| {
                widgets::segment(("start-as", i), Some(glyph), name, terminal == value, false)
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.update_prefs(|p| p.terminal = value, cx)
                    }))
            },
        ));
        let mono = |text: String| {
            div()
                .max_w(px(360.))
                .truncate()
                .px(px(9.))
                .py(px(5.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(0.04))
                .font_family(MONO)
                .text_size(px(12.))
                .text_color(colors::text2())
                .child(text)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(group(
                "App",
                vec![row(
                    "Version",
                    "The build of HyprSpace you are running.",
                    mono(format!("v{VERSION}")),
                )],
            ))
            .child(group(
                "Sessions",
                vec![row(
                    "New sessions start as",
                    if terminal {
                        "The agent's own CLI runs in a terminal."
                    } else {
                        "The agent's work shows as a transcript you can read and steer."
                    },
                    start,
                )],
            ))
            .child(group(
                "Data",
                vec![row(
                    "State folder",
                    "Your spaces, threads and settings are saved here.",
                    mono(state_dir().display().to_string()),
                )],
            ))
            .into_any_element()
    }
}
