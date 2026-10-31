use gpui::{ClickEvent, Context, div, prelude::*};
use hyprspace_proto::NotifyPrefs;

use super::Root;
use super::controls::{group, row};
use crate::widgets;

impl Root {
    pub(super) fn notify_group(&self, cx: &mut Context<Self>) -> gpui::Div {
        let prefs = self.state.notify;
        let switch = |id: &'static str, on: bool, flip: fn(&mut NotifyPrefs)| {
            widgets::switch(id, on).on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                flip(&mut r.state.notify);
                r.save();
                cx.notify();
            }))
        };
        group(
            "Notifications",
            vec![
                row(
                    "Waiting on you",
                    "A thread asks for an approval or has a question.",
                    switch("notify-waiting", prefs.waiting, |p| p.waiting = !p.waiting),
                ),
                row(
                    "Failed",
                    "A run ends with an error.",
                    switch("notify-failed", prefs.failed, |p| p.failed = !p.failed),
                ),
                row(
                    "Finished",
                    "A run ends.",
                    switch("notify-finished", prefs.finished, |p| {
                        p.finished = !p.finished
                    }),
                ),
                row(
                    "Where they show",
                    "A system notification while HyprSpace is in the background, and a toast at the top while it is in front. They stay on this computer. Nothing is sent anywhere.",
                    div(),
                ),
            ],
        )
    }
}
