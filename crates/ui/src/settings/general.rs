// Settings, General: how a new thread starts (which agent, and how much it may do on its own),
// when threads settle, every snoozed thread and when it wakes, which app the Open button opens a
// folder in, and on macOS what agents may do outside their terminal. The first two are the
// composer's saved picks, so changing one here or in the composer changes both.

use gpui::{AnyElement, ClickEvent, Context, div, prelude::*};
use hyprspace_proto::{Agent, Permission, SettleAfter, Snooze};

use super::controls::{group, row, row_with, text};
use super::index::{
    AGENT, AGENT_CHECKS, CLEANUP, OPENER, PERMISSION, SETTLE, SNAPSHOTS, SNOOZED, STRUCTURED,
    TITLES, WORKING,
};
use super::{Picker, Root, Tab};
use crate::time::{left, now_ms};
use crate::zoom::px;
use crate::{colors, widgets};

/// Each mode, most careful first: its name and what picking it means.
pub(super) const MODES: [(Permission, &str, &str); 4] = [
    (
        Permission::Plan,
        "Plan only",
        "Reads and plans. Changes nothing.",
    ),
    (
        Permission::Ask,
        "Ask first",
        "Asks before edits and commands.",
    ),
    (
        Permission::Auto,
        "Auto edit",
        "Edits on its own, asks for the rest.",
    ),
    (
        Permission::Bypass,
        "Full access",
        "Edits files and runs commands without asking.",
    ),
];

pub(super) fn settle_name(after: SettleAfter) -> &'static str {
    match after {
        SettleAfter::Never => "Never",
        SettleAfter::Day => "1 day",
        SettleAfter::ThreeDays => "3 days",
        SettleAfter::Week => "1 week",
    }
}

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

    /// Every snoozed thread, soonest to wake first: its project, when it wakes and how long that
    /// is, and a button to wake it now. One snoozed until its agent finishes comes last.
    fn snoozed_group(&self, cx: &mut Context<Self>) -> gpui::Div {
        let now = now_ms();
        let mut snoozed: Vec<(&hyprspace_proto::Space, &hyprspace_proto::Thread, Snooze)> = self
            .state
            .spaces
            .iter()
            .flat_map(|s| {
                s.threads
                    .iter()
                    .filter_map(move |t| Some((s, t, t.snooze?)))
            })
            .collect();
        snoozed.sort_by_key(|(_, _, snooze)| match snooze {
            Snooze::Time { at } => *at,
            Snooze::Done => u64::MAX,
        });
        let title = match snoozed.len() {
            0 => "Snoozed".to_string(),
            n => format!("Snoozed ({n})"),
        };
        let rows: Vec<AnyElement> = if snoozed.is_empty() {
            vec![row(
                "Nothing is snoozed",
                "Use the clock on a thread's row.",
                div(),
            )]
        } else {
            snoozed
                .into_iter()
                .map(|(space, t, snooze)| {
                    let id = t.id;
                    let when = match snooze {
                        Snooze::Time { at } => format!(
                            "{}, wakes {} ({})",
                            space.name,
                            self.wake_text(snooze),
                            left(at, now)
                        ),
                        Snooze::Done => format!("{}, wakes when its agent finishes", space.name),
                    };
                    row(
                        t.title.clone(),
                        when,
                        widgets::button(("wake-now", id), "Wake now").on_click(cx.listener(
                            move |r, _: &ClickEvent, window, cx| r.snooze(id, None, window, cx),
                        )),
                    )
                })
                .collect()
        };
        group(&title, rows)
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

        let structured = prefs.structured;
        let switch = widgets::switch("structured", structured).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| r.update_prefs(|p| p.structured = !structured, cx),
        ));
        let mode = MODES
            .iter()
            .find(|m| m.0 == prefs.permission)
            .unwrap_or(&MODES[1]);
        let permission = self.spot(
            PERMISSION.spot(),
            row_with(
                None,
                PERMISSION.title,
                div()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(mode.2),
                self.with_reset(
                    &PERMISSION,
                    self.dropdown(Picker::Permission, mode.1, cx),
                    cx,
                ),
            ),
            false,
        );

        let after = self.state.settle_after;
        let settle = widgets::segments().children(SettleAfter::ALL.into_iter().enumerate().map(
            |(i, value)| {
                let name = settle_name(value);
                widgets::segment(("settle-after", i), None, name, after == value, false).on_click(
                    cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.state.settle_after = value;
                        r.save();
                        r.tidy_threads(cx);
                        cx.notify();
                    }),
                )
            },
        ));

        let clean = self.state.clean_worktrees;
        let cleanup = widgets::segments().children(
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
                    widgets::segment(("clean-worktrees", i), None, name, clean == value, false)
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.state.clean_worktrees = value;
                            r.save();
                            cx.notify();
                        }))
                }),
        );

        let named = self.state.titles;
        let titles = widgets::switch("titles", named).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| {
                r.state.titles = !named;
                r.save();
                cx.notify();
            },
        ));

        let shelf = self.state.working_shelf;
        let working = widgets::switch("working-shelf", shelf).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| {
                r.state.working_shelf = !shelf;
                r.save();
                cx.notify();
            },
        ));

        let snapping = self.state.snapshots;
        let snapshots = widgets::switch("snapshots", snapping).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| {
                r.state.snapshots = !snapping;
                r.client
                    .send(hyprspace_proto::Command::Snapshots { on: !snapping });
                r.save();
                cx.notify();
            },
        ));

        let checking = self.state.agent_checks;
        let agent_checks = widgets::switch("agent-checks", checking).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| {
                r.state.agent_checks = !checking;
                r.updater.update(cx, |u, _| u.set_agent_checks(!checking));
                r.save();
                cx.notify();
            },
        ));

        let opener = self.state.open_with;
        let open_control = if self.work.openers.is_empty() {
            text("Looking for apps").into_any_element()
        } else {
            self.dropdown(Picker::Opener, opener.name(), cx)
        };

        let page = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group(
                "New threads",
                vec![self.item(&AGENT, agent_control, cx), permission],
            ))
            .child(group(
                "Threads",
                vec![
                    self.item(&SETTLE, settle, cx),
                    self.item(&SNAPSHOTS, snapshots, cx),
                    self.item(&TITLES, titles, cx),
                ],
            ))
            .child(group(
                "Updates",
                vec![self.item(&AGENT_CHECKS, agent_checks, cx)],
            ))
            .child(self.spot(
                SNOOZED.spot(),
                self.snoozed_group(cx).into_any_element(),
                true,
            ))
            .child(group(
                "Experimental",
                vec![
                    self.item(&STRUCTURED, switch, cx),
                    self.item(&WORKING, working, cx),
                ],
            ))
            .child(group(
                "Folders",
                vec![
                    self.item(&OPENER, open_control, cx),
                    self.item(&CLEANUP, cleanup, cx),
                ],
            ));
        #[cfg(target_os = "macos")]
        let page = page.child(self.mac_group(cx));
        page.child(self.restore_group(Tab::General, cx))
            .into_any_element()
    }
}
