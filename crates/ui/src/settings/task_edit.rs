// Editing a scheduled task in place: its name, the message each run sends, and when it runs. The
// agent, model and permission stay as they were set.

use gpui::{AnyElement, ClickEvent, Context, Entity, FontWeight, div, prelude::*};
use hyprspace_proto::schedule::{When, clock};

use super::Root;
use crate::composer::{minute_of, title_of};
use crate::input::TextInput;
use crate::time::now_ms;
use crate::zoom::px;
use crate::{colors, widgets};

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub(crate) struct TaskEdit {
    pub(crate) id: u64,
    name: Entity<TextInput>,
    prompt: Entity<TextInput>,
    at: Entity<TextInput>,
    label: Entity<TextInput>,
    /// What starts it; its time and label are read from their fields on Save.
    when: When,
    error: Option<&'static str>,
}

fn same_kind(a: &When, b: &When) -> bool {
    match (a, b) {
        (When::Every { minutes: x }, When::Every { minutes: y }) => x == y,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

fn input_box(input: &Entity<TextInput>, tall: bool) -> AnyElement {
    div()
        .w_full()
        .px(px(10.))
        .py(px(7.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.03))
        .text_size(px(13.))
        .text_color(colors::text1())
        .when(tall, |d| d.min_h(px(64.)))
        .cursor_text()
        .child(input.clone())
        .into_any_element()
}

fn caption(text: &'static str) -> AnyElement {
    div()
        .text_size(px(11.5))
        .text_color(colors::text3())
        .child(text)
        .into_any_element()
}

impl Root {
    pub(crate) fn edit_task(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(t) = self.state.scheduled.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        let field = |text: String, multiline: bool, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut i = TextInput::new("", multiline, cx);
                i.set_text(text, cx);
                i
            })
        };
        let at = match t.when {
            When::Daily { minute } | When::Weekly { minute, .. } => clock(minute),
            _ => "9:00 AM".into(),
        };
        let label = match &t.when {
            When::Labeled { label } => label.clone(),
            _ => String::new(),
        };
        self.task_edit = Some(TaskEdit {
            id,
            name: field(t.name.clone(), false, cx),
            prompt: field(t.prompt.clone(), true, cx),
            at: field(at, false, cx),
            label: field(label, false, cx),
            when: t.when.clone(),
            error: None,
        });
        cx.notify();
    }

    fn save_task(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.task_edit.as_mut() else {
            return;
        };
        let minute = minute_of(edit.at.read(cx).text());
        let label = edit.label.read(cx).text().trim().to_string();
        let prompt = edit.prompt.read(cx).text().trim().to_string();
        let name = edit.name.read(cx).text().trim().to_string();
        let when = match edit.when.clone() {
            When::Daily { .. } => minute.map(|minute| When::Daily { minute }),
            When::Weekly { day, .. } => minute.map(|minute| When::Weekly { day, minute }),
            When::Labeled { .. } => (!label.is_empty()).then_some(When::Labeled { label }),
            when => Some(when),
        };
        let checked = match (&when, prompt.is_empty()) {
            (_, true) => Err("Write what each run should do."),
            (None, _) if matches!(edit.when, When::Labeled { .. }) => {
                Err("Write the label to watch for.")
            }
            (None, _) => Err("Write the time as 9:00 AM or 9:30 PM."),
            (Some(when), false) => Ok(when.clone()),
        };
        let when = match checked {
            Ok(when) => when,
            Err(why) => {
                edit.error = Some(why);
                cx.notify();
                return;
            }
        };
        let id = edit.id;
        if let Some(t) = self.state.scheduled.iter_mut().find(|t| t.id == id) {
            if t.when != when {
                t.last = now_ms();
                t.seen = None;
            }
            t.name = if name.is_empty() {
                title_of(&prompt)
            } else {
                name
            };
            t.prompt = prompt;
            t.when = when;
        }
        self.task_edit = None;
        self.save();
        cx.notify();
    }

    pub(super) fn task_editor(&self, edit: &TaskEdit, cx: &mut Context<Self>) -> AnyElement {
        let choices = [
            ("Every hour", When::Every { minutes: 60 }),
            ("Every 6 hours", When::Every { minutes: 360 }),
            ("Every day", When::Daily { minute: 0 }),
            (
                "Every week",
                When::Weekly {
                    day: match edit.when {
                        When::Weekly { day, .. } => day,
                        _ => 0,
                    },
                    minute: 0,
                },
            ),
            ("When CI fails", When::CiFails),
            ("When a PR opens", When::PrOpens),
            (
                "When an issue is labeled",
                When::Labeled {
                    label: String::new(),
                },
            ),
        ];
        let kinds =
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .children(choices.into_iter().enumerate().map(|(i, (label, when))| {
                    let on = same_kind(&edit.when, &when);
                    widgets::segment(("task-kind", i), None, label, on, false)
                        .rounded(px(7.))
                        .bg(if on {
                            colors::ink(0.1)
                        } else {
                            colors::ink(0.04)
                        })
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            if let Some(e) = r.task_edit.as_mut() {
                                e.when = when.clone();
                                e.error = None;
                            }
                            cx.notify();
                        }))
                }));
        let days = match edit.when {
            When::Weekly { day, .. } => Some(div().flex().gap(px(4.)).children(
                DAYS.iter().enumerate().map(|(i, d)| {
                    let i = i as u32;
                    widgets::segment(("task-day", i as usize), None, *d, day == i, false)
                        .rounded(px(7.))
                        .bg(if day == i {
                            colors::ink(0.1)
                        } else {
                            colors::ink(0.04)
                        })
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            if let Some(e) = r.task_edit.as_mut() {
                                e.when = When::Weekly { day: i, minute: 0 };
                            }
                            cx.notify();
                        }))
                }),
            )),
            _ => None,
        };
        let at = matches!(edit.when, When::Daily { .. } | When::Weekly { .. });
        let detail = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(10.))
            .children(days)
            .when(at, |d| {
                d.child(caption("at"))
                    .child(div().w(px(110.)).child(input_box(&edit.at, false)))
            })
            .when(matches!(edit.when, When::Labeled { .. }), |d| {
                d.child(caption("label"))
                    .child(div().w(px(140.)).child(input_box(&edit.label, false)))
            })
            .when(matches!(edit.when, When::Once { .. }), |d| {
                d.child(caption("Runs once at the time it was set for."))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(14.))
            .rounded(px(10.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::ink(0.02))
            .child(caption("Name"))
            .child(input_box(&edit.name, false))
            .child(caption("Each run sends"))
            .child(input_box(&edit.prompt, true))
            .child(caption("Runs"))
            .child(kinds)
            .child(detail)
            .children(edit.error.map(|why| {
                div()
                    .text_size(px(12.))
                    .text_color(colors::error())
                    .child(why)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1())
                    .child(
                        widgets::button("task-cancel", "Cancel").on_click(cx.listener(
                            |r, _: &ClickEvent, _, cx| {
                                r.task_edit = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        widgets::primary("task-save", "Save")
                            .h(px(28.))
                            .px(px(12.))
                            .rounded(px(8.))
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.save_task(cx))),
                    ),
            )
            .into_any_element()
    }
}
