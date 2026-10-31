// Settings, Scheduled: the tasks that start threads on a schedule or on GitHub, each with run now,
// pause and delete. Tasks are made from a new thread's box, with the clock beside Send.

use gpui::{AnyElement, ClickEvent, Context, div, prelude::*};
use hyprspace_proto::schedule::Scheduled;

use super::Root;
use super::controls::{group, row, row_with, text};
use crate::assets::icon;
use crate::time::{local, local_now, wake_label};
use crate::zoom::px;
use crate::{colors, widgets};

impl Root {
    pub(super) fn scheduled_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.scheduled.is_empty() {
            return group(
                "Tasks",
                vec![row(
                    "Nothing is scheduled",
                    "Write a message in a new thread and press the clock beside Send. It can run on the clock, or each time CI fails, a pull request opens or an issue gets a label.",
                    div(),
                )],
            )
            .into_any_element();
        }
        let rows = self
            .state
            .scheduled
            .iter()
            .map(|t| self.scheduled_row(t, cx))
            .collect();
        group("Tasks", rows).into_any_element()
    }

    fn scheduled_row(&self, t: &Scheduled, cx: &mut Context<Self>) -> AnyElement {
        let place = self
            .state
            .space(t.space)
            .map_or("a removed project".to_string(), |s| s.name.clone());
        let mut what = format!("{} \u{b7} {place}", t.when.label());
        if t.worktree {
            what.push_str(" \u{b7} new worktree");
        }
        let next = if t.paused {
            "Paused".to_string()
        } else {
            match crate::root::next_run(&t.when, t.last, chrono::Local::now()) {
                Some(at) => format!("Next {}", wake_label(local(at), local_now())),
                None => "Looks at GitHub every 5 minutes".to_string(),
            }
        };
        let under = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(text(what))
            .child(text(next));
        let lead = div()
            .flex_none()
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .bg(colors::ink(0.05))
            .child(icon(
                crate::composer::schedule_glyph(&t.when),
                14.,
                if t.paused {
                    colors::text3()
                } else {
                    colors::text2()
                },
            ))
            .into_any_element();
        let (id, paused) = (t.id, t.paused);
        let controls = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                widgets::icon_button(("scheduled-run", id as usize), "play", 26.)
                    .tooltip(widgets::tip("Run now"))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.fire(id, None, cx);
                        cx.notify();
                    })),
            )
            .child(
                widgets::icon_button(("scheduled-delete", id as usize), "trash-2", 26.)
                    .tooltip(widgets::tip("Delete"))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.state.scheduled.retain(|t| t.id != id);
                        r.save();
                        cx.notify();
                    })),
            )
            .child(div().w(px(6.)))
            .child(
                widgets::switch(("scheduled-on", id as usize), !paused)
                    .tooltip(widgets::tip(if paused { "Resume" } else { "Pause" }))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        if let Some(t) = r.state.scheduled.iter_mut().find(|t| t.id == id) {
                            t.paused = !paused;
                        }
                        r.save();
                        cx.notify();
                    })),
            );
        row_with(Some(lead), t.name.clone(), under, controls)
    }
}
