// The card that shows beside a thread's row when the pointer rests on it, after T3 Code's: the
// title, the project and its folder, the agent with its model and effort, and the subagents it has
// running. The row itself only has room for the agent's mark. It floats just past the sidebar's edge, level with the row, the
// way T3 Code's does, so the list under the pointer stays in view.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, FontWeight, IntoElement, SharedString, Window, anchored, deferred,
    div, point, prelude::*, px,
};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

use crate::assets::{icon, mark};
use crate::colors;
use crate::root::Root;

/// How long the pointer rests on a row before its card shows.
const AFTER: Duration = Duration::from_millis(450);

#[derive(Clone)]
pub struct RowCard {
    pub title: SharedString,
    pub space: SharedString,
    pub path: SharedString,
    pub agent: Option<Agent>,
    /// The agent with its model and effort, "Claude Opus 5.5 · High", or "Terminal".
    pub model: SharedString,
    /// Each subagent running: what it was asked, and how long it has run.
    pub subagents: Vec<(SharedString, SharedString)>,
}

/// Subagents listed before the rest fold into a count.
const SUBS_SHOWN: usize = 6;

/// One line of the card: a glyph in a fixed column, then its text.
fn line(glyph: AnyElement, text: SharedString) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_none()
                .justify_center()
                .w(px(16.))
                .child(glyph),
        )
        .child(div().min_w_0().truncate().child(text))
}

impl RowCard {
    fn render(self) -> AnyElement {
        let agent = match self.agent {
            Some(a) => mark(a, 12., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 12., colors::text3()).into_any_element(),
        };
        div()
            .max_w(px(320.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .text_size(px(12.))
            .text_color(colors::text2())
            .child(
                div()
                    .truncate()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(self.title.clone()),
            )
            .child(line(
                super::row::tag(&self.space, false),
                self.space.clone(),
            ))
            .child(
                div()
                    .pl(px(23.))
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .child(self.path.clone()),
            )
            .child(line(agent, self.model.clone()))
            .when(!self.subagents.is_empty(), |d| {
                let more = self.subagents.len().saturating_sub(SUBS_SHOWN);
                d.child(div().my(px(2.)).h(px(1.)).bg(colors::border1()))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text3())
                            .child(match self.subagents.len() {
                                1 => "1 subagent".to_string(),
                                n => format!("{n} subagents"),
                            }),
                    )
                    .children(self.subagents.iter().take(SUBS_SHOWN).enumerate().map(
                        |(i, (label, took))| {
                            // the agent's mark with the ring the rows turn while they work
                            let badge = match self.agent {
                                Some(a) => mark(a, 10., colors::brand(a).0).into_any_element(),
                                None => icon("bot", 10., colors::text3()).into_any_element(),
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .min_w_0()
                                .child(
                                    div()
                                        .relative()
                                        .flex()
                                        .flex_none()
                                        .items_center()
                                        .justify_center()
                                        .size(px(16.))
                                        .child(badge)
                                        .child(super::row::ring(("card-sub-ring", i), 16., 2.)),
                                )
                                .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                                .child(
                                    div()
                                        .flex_none()
                                        .font_family(MONO)
                                        .text_size(px(10.5))
                                        .text_color(colors::text3())
                                        .child(took.clone()),
                                )
                        },
                    ))
                    .when(more > 0, |d| {
                        d.child(
                            div()
                                .pl(px(23.))
                                .text_size(px(11.))
                                .text_color(colors::text3())
                                .child(format!("{more} more")),
                        )
                    })
            })
            .into_any_element()
    }
}

impl Root {
    /// The pointer came onto a thread's row, or left it. The card waits a moment, so sweeping
    /// across the list doesn't flash one card after another.
    pub(crate) fn row_hovered(&mut self, thread: u64, on: bool, cx: &mut Context<Self>) {
        if on {
            self.hover_row = Some((thread, Instant::now()));
            self._hover_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(AFTER).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
        } else if self.hover_row.is_some_and(|(t, _)| t == thread) {
            self.hover_row = None;
        }
        cx.notify();
    }

    /// The card for the row under the pointer, past the sidebar's edge and level with the row.
    /// Not while a menu is open or something is being dragged, and only while the pointer is on
    /// that row. On Windows the pointer can leave for another monitor without any element hearing
    /// of it; GPUI only marks the window unhovered and redraws, so that is checked here.
    pub(crate) fn hover_card(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let (thread, since) = self.hover_row?;
        if since.elapsed() < AFTER || self.menu.is_some() || cx.has_active_drag() {
            return None;
        }
        let bounds = *self.row_bounds.borrow().get(&thread)?;
        if !window.is_window_hovered() || !bounds.contains(&window.mouse_position()) {
            return None;
        }
        let card = self.thread_card(thread, cx)?;
        Some(
            deferred(
                anchored()
                    .position(point(bounds.right() + px(16.), bounds.top()))
                    .snap_to_window_with_margin(px(8.))
                    .child(card.render()),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}
