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
        if let Some(edit) = self.task_edit.as_ref().filter(|e| e.id == t.id) {
            return self.task_editor(edit, cx);
        }
        let place = self
            .state
            .space(t.space)
            .map_or("a removed project".to_string(), |s| s.name.clone());
        let mut what = format!("{} \u{b7} {place}", t.when.label());
        if let Some((_, thread)) = t.thread.and_then(|id| self.state.thread(id)) {
            what.push_str(&format!(" \u{b7} in {}", thread.title));
        }
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
        let ran = self
            .ran
            .as_ref()
            .filter(|(task, _, _)| *task == t.id)
            .map(|(_, n, result)| ran_line(*n, result, cx));
        let under = div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(text(what))
            .child(text(next))
            .children(ran);
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
                widgets::icon_button(("scheduled-edit", id as usize), "pencil", 26.)
                    .tooltip(widgets::tip("Edit"))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.edit_task(id, cx))),
            )
            .child(
                widgets::icon_button(("scheduled-run", id as usize), "play", 26.)
                    .tooltip(widgets::tip("Run now"))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        let n = r.ran.as_ref().map_or(0, |(_, n, _)| n + 1);
                        r.ran = Some((id, n, r.fire(id, None, cx)));
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

/// What came of a run started by hand: the thread it started and a way to it, or why it didn't.
fn ran_line(n: usize, result: &crate::root::Fired, cx: &mut Context<Root>) -> AnyElement {
    let line = div().flex().items_center().gap(px(6.)).text_size(px(12.));
    let line = match *result {
        Ok(Some(thread)) => line
            .text_color(colors::text2())
            .child(icon("check", 12., colors::ok()))
            .child("Running now")
            .child(
                widgets::button(("scheduled-open", n), "Open")
                    .h(px(22.))
                    .px(px(9.))
                    .text_size(px(11.5))
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.open_thread(thread, window, cx)
                    })),
            ),
        Ok(None) => line
            .text_color(colors::text2())
            .child(crate::spinner::ring(("scheduled-wait", n), colors::text3()))
            .child("Making its worktree, then the thread starts"),
        Err(why) => line
            .text_color(colors::error())
            .child(icon("circle-alert", 12., colors::error()))
            .child(why),
    };
    crate::slide::ease_in(line, ("scheduled-ran", n), 220, |d, t| {
        d.opacity(t).mt(px(4. * t))
    })
}
