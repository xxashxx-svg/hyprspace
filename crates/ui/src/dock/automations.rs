// The Automations tab: what the thread on screen runs later, most often set up by its own agent,
// with run now, pause and delete on each.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*};

use super::{Dock, DockEvent};
use crate::assets::icon;
use crate::root::Fired;
use crate::zoom::px;
use crate::{colors, slide, widgets};

/// One automation as the tab shows it.
#[derive(Clone, PartialEq)]
pub struct Automation {
    pub id: u64,
    pub name: String,
    pub glyph: &'static str,
    /// When it runs: "Every day at 9:00 AM".
    pub when: String,
    /// "Next Wed 11:00 PM", "Paused" or "Looks at GitHub every 5 minutes".
    pub next: String,
    pub paused: bool,
    /// What came of the last Run now, and how many there have been.
    pub ran: Option<(usize, Fired)>,
}

fn ran_line(n: usize, ran: &Fired, cx: &mut Context<Dock>) -> AnyElement {
    let line = div().flex().items_center().gap(px(6.)).text_size(px(12.));
    let line = match *ran {
        Ok(Some(thread)) => line
            .text_color(colors::text2())
            .child(icon("check", 12., colors::ok()))
            .child("Sent to the thread")
            .child(
                widgets::button(("automation-open", n), "Open")
                    .h(px(22.))
                    .px(px(9.))
                    .text_size(px(11.5))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        cx.emit(DockEvent::GoTo(thread))
                    })),
            ),
        Ok(None) => line
            .text_color(colors::text2())
            .child(crate::spinner::ring(
                ("automation-wait", n),
                colors::text3(),
            ))
            .child("Making its worktree first"),
        Err(why) => line
            .text_color(colors::error())
            .child(icon("circle-alert", 12., colors::error()))
            .child(why),
    };
    slide::ease_in(line, ("automation-ran", n), 220, |d, t| {
        d.opacity(t).mt(px(6. * t))
    })
}

fn row(a: &Automation, cx: &mut Context<Dock>) -> AnyElement {
    let (id, paused) = (a.id, a.paused);
    div()
        .flex()
        .gap(px(10.))
        .px(px(10.))
        .py(px(10.))
        .rounded(px(10.))
        .bg(colors::ink(0.03))
        .border_1()
        .border_color(colors::ink(0.06))
        .child(
            div()
                .flex_none()
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .bg(colors::ink(0.05))
                .child(icon(
                    a.glyph,
                    14.,
                    if paused {
                        colors::text3()
                    } else {
                        colors::text2()
                    },
                )),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(a.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(format!("{} \u{b7} {}", a.when, a.next)),
                )
                .children(a.ran.as_ref().map(|(n, ran)| ran_line(*n, ran, cx))),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(2.))
                .child(
                    widgets::icon_button(("automation-edit", id), "pencil", 26.)
                        .tooltip(widgets::tip("Edit"))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(DockEvent::EditAutomation(id))
                        })),
                )
                .child(
                    widgets::icon_button(("automation-run", id), "play", 26.)
                        .tooltip(widgets::tip("Run now"))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(DockEvent::RunAutomation(id))
                        })),
                )
                .child(
                    widgets::icon_button(("automation-delete", id), "trash-2", 26.)
                        .tooltip(widgets::tip("Delete"))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(DockEvent::DeleteAutomation(id))
                        })),
                )
                .child(div().w(px(4.)))
                .child(
                    widgets::switch(("automation-on", id), !paused)
                        .tooltip(widgets::tip(if paused { "Resume" } else { "Pause" }))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.emit(DockEvent::PauseAutomation(id, !paused))
                        })),
                ),
        )
        .into_any_element()
}

impl Dock {
    pub fn set_automations(
        &mut self,
        automations: Vec<Automation>,
        on_thread: bool,
        cx: &mut Context<Self>,
    ) {
        if self.automations != automations || self.on_thread != on_thread {
            self.automations = automations;
            self.on_thread = on_thread;
            cx.notify();
        }
    }

    pub(super) fn automations_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.automations.is_empty() {
            return super::empty(
                "Nothing scheduled in this thread. Ask its agent to do something later, like \"check this again at 9pm\".",
            );
        }
        let rows: Vec<AnyElement> = self.automations.iter().map(|a| row(a, cx)).collect();
        div()
            .id("dock-automations")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(10.))
            .children(rows)
            .into_any_element()
    }
}
