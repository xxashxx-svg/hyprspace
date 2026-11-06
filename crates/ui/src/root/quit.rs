use gpui::{AnyElement, ClickEvent, Context, FontWeight, MouseButton, Window, div, prelude::*};

use super::Root;
use crate::transcript::Status;
use crate::zoom::px;
use crate::{colors, slide, spinner, widgets};

const LISTED: usize = 4;

impl Root {
    pub(super) fn guard_close(window: &mut Window, cx: &mut Context<Self>) {
        let root = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            root.update(cx, |r, cx| {
                if r.close_ok || r.closing || r.working() == 0 {
                    return true;
                }
                r.closing = true;
                cx.notify();
                false
            })
            .unwrap_or(true)
        });
    }

    /// The threads at work on this computer, as (id, title, space name).
    fn at_work(&self) -> Vec<(u64, String, String)> {
        self.state
            .spaces
            .iter()
            .flat_map(|s| {
                s.threads
                    .iter()
                    .filter(|t| t.id < crate::machines::FIRST && self.busy(t.id))
                    .map(|t| (t.id, t.title.clone(), s.name.clone()))
            })
            .collect()
    }

    fn work_row(&self, id: u64, title: String, space: String) -> AnyElement {
        let waiting = self.status.get(&id) == Some(&Status::Waiting);
        let mark = if waiting {
            div()
                .size(px(6.))
                .rounded_full()
                .bg(colors::waiting())
                .into_any_element()
        } else {
            spinner::ring(("close-spin", id), colors::text2())
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(32.))
            .px(px(12.))
            .child(
                div()
                    .flex_none()
                    .size(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(mark),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .text_color(colors::text1())
                    .child(title),
            )
            .child(
                div()
                    .flex_none()
                    .max_w(px(120.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(if waiting {
                        colors::waiting()
                    } else {
                        colors::text3()
                    })
                    .child(if waiting {
                        "Needs you".to_string()
                    } else {
                        space
                    }),
            )
            .into_any_element()
    }

    pub(super) fn close_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.closing {
            return None;
        }
        let work = self.at_work();
        let n = self.working();
        if n == 0 {
            self.closing = false;
            return None;
        }
        let line = match n {
            1 => "1 thread is still working. Closing stops it, and you can resume it when \
                  HyprSpace opens again."
                .to_string(),
            n => format!(
                "{n} threads are still working. Closing stops them, and you can resume them \
                 when HyprSpace opens again."
            ),
        };
        let more = work.len().saturating_sub(LISTED);
        let rows: Vec<AnyElement> = work
            .into_iter()
            .take(LISTED)
            .map(|(id, title, space)| self.work_row(id, title, space))
            .collect();
        let list = (!rows.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .py(px(4.))
                .rounded(px(10.))
                .border_1()
                .border_color(colors::ink(0.06))
                .bg(colors::ink(0.03))
                .children(rows)
                .when(more > 0, |d| {
                    d.child(
                        div()
                            .h(px(26.))
                            .px(px(12.))
                            .pl(px(36.))
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child(format!("and {more} more")),
                    )
                })
        });
        let card = div()
            .w(px(420.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(20.))
            .rounded(px(14.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child("Close HyprSpace?"),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .line_height(px(18.))
                            .text_color(colors::text2())
                            .child(line),
                    ),
            )
            .children(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        widgets::button("close-anyway", "Close anyway")
                            .text_color(colors::error())
                            .on_click(cx.listener(|r, _: &ClickEvent, window, _| {
                                r.close_ok = true;
                                window.remove_window();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        widgets::primary("close-cancel", "Keep working")
                            .h(px(28.))
                            .px(px(12.))
                            .rounded(px(8.))
                            .text_size(px(12.5))
                            .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                                r.closing = false;
                                cx.notify();
                            })),
                    ),
            );
        let card = slide::ease_in(card, "close-card", 200, |d, t| {
            d.opacity(t).mt(px(12. * (1. - t)))
        });
        let scrim = colors::bg();
        Some(slide::ease_in(
            div()
                .id("close-dialog")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|r, _, _, cx| {
                        r.closing = false;
                        cx.notify();
                    }),
                )
                .child(card),
            "close-scrim",
            200,
            move |d, t| d.bg(scrim.opacity(0.64 * t)),
        ))
    }
}
