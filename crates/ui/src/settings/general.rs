// Settings, General: how a new thread starts (which agent, as a transcript or a terminal, and how
// much it may do on its own) and which app the Open button opens a folder in. The first three are
// the composer's saved picks, so changing one here or in the composer changes both.

use gpui::{AnyElement, ClickEvent, Context, div, prelude::*, px};
use hyprspace_proto::{Agent, Permission};

use super::controls::{group, row, row_with, text};
use super::{Picker, Root};
use crate::{colors, widgets};

/// Each mode, most careful first: its name and what picking it means.
pub(super) const MODES: [(Permission, &str, &str); 4] = [
    (
        Permission::Plan,
        "Plan only",
        "Reads and plans. It changes nothing.",
    ),
    (
        Permission::Ask,
        "Ask first",
        "Asks before every edit and command.",
    ),
    (
        Permission::Auto,
        "Auto edit",
        "Edits files in the folder on its own. Asks before anything else.",
    ),
    (
        Permission::Bypass,
        "Full access",
        "Never asks. Use it only in folders you trust.",
    ),
];

impl Root {
    /// The agent the composer starts the next thread with: the saved pick if it is installed,
    /// else the first one that is (the composer's own rule).
    pub(super) fn new_thread_agent(&self) -> Option<Agent> {
        let mut installed = self.agents.iter().filter(|a| a.status.installed);
        let first = installed.clone().next().map(|a| a.agent);
        installed
            .find(|a| Some(a.agent) == self.state.composer.agent)
            .map(|a| a.agent)
            .or(first)
    }

    pub(super) fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        let prefs = &self.state.composer;
        let agent_control = match self.new_thread_agent() {
            Some(a) => self.dropdown(Picker::Agent, a.name(), cx),
            None => text(if self.agents.is_empty() {
                "Checking"
            } else {
                "None installed"
            })
            .into_any_element(),
        };

        let terminal = prefs.terminal;
        let structured = prefs.structured;
        let switch = widgets::segments().children(
            [(false, "Off"), (true, "On")]
                .into_iter()
                .enumerate()
                .map(|(i, (value, name))| {
                    widgets::segment(("structured", i), None, name, structured == value, false)
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.update_prefs(|p| p.structured = value, cx)
                        }))
                }),
        );
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

        let mode = MODES
            .iter()
            .find(|m| m.0 == prefs.permission)
            .unwrap_or(&MODES[1]);
        let risky = mode.0 == Permission::Bypass;
        let permission = row_with(
            None,
            "Permission",
            div()
                .text_size(px(12.))
                .text_color(if risky {
                    colors::error()
                } else {
                    colors::text3()
                })
                .child(mode.2),
            self.dropdown(Picker::Permission, mode.1, cx),
        );

        let after = self.state.settle_after;
        let settle = widgets::segments().children(
            hyprspace_proto::SettleAfter::ALL
                .into_iter()
                .enumerate()
                .map(|(i, value)| {
                    let name = match value {
                        hyprspace_proto::SettleAfter::Never => "Never",
                        hyprspace_proto::SettleAfter::Day => "1 day",
                        hyprspace_proto::SettleAfter::ThreeDays => "3 days",
                        hyprspace_proto::SettleAfter::Week => "1 week",
                    };
                    widgets::segment(("settle-after", i), None, name, after == value, false)
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.state.settle_after = value;
                            r.save();
                            r.tidy_threads(cx);
                            cx.notify();
                        }))
                }),
        );

        let opener = self.state.open_with;
        let open_control = if self.work.openers.is_empty() {
            text("Looking for apps").into_any_element()
        } else {
            self.dropdown(Picker::Opener, opener.name(), cx)
        };

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group(
                "New threads",
                [
                    Some(row(
                        "Agent",
                        "New threads start with it. The composer changes it too.",
                        agent_control,
                    )),
                    structured.then(|| {
                        row(
                            "Start as",
                            if terminal {
                                "The agent's own CLI runs in a terminal."
                            } else {
                                "The agent's work shows as a transcript you can read and steer."
                            },
                            start,
                        )
                    }),
                    Some(permission),
                ]
                .into_iter()
                .flatten()
                .collect(),
            ))
            .child(group(
                "Threads",
                vec![row(
                    "Settle untouched threads",
                    "A thread nobody touched for this long moves to the Settled shelf. One on screen or at work stays.",
                    settle,
                )],
            ))
            .child(group(
                "Experimental",
                vec![row(
                    "Structured sessions",
                    "Lets Claude and Codex run as a transcript you read and steer, instead of in a terminal. Still being built.",
                    switch,
                )],
            ))
            .child(group(
                "Folders",
                vec![row(
                    "Open folders in",
                    "The Open button in a thread's bar opens its folder here.",
                    open_control,
                )],
            ))
            .into_any_element()
    }
}
