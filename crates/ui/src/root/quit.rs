use gpui::{AnyElement, ClickEvent, Context, FontWeight, MouseButton, Window, div, prelude::*};

use super::Root;
use crate::assets::icon;
use crate::zoom::px;
use crate::{colors, widgets};

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

    pub(super) fn close_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.closing {
            return None;
        }
        let n = self.working();
        if n == 0 {
            return None;
        }
        let (title, line) = match n {
            1 => (
                "A thread is still working".to_string(),
                "Closing HyprSpace stops it. You can resume it when HyprSpace opens again.",
            ),
            n => (
                format!("{n} threads are still working"),
                "Closing HyprSpace stops them. You can resume them when HyprSpace opens again.",
            ),
        };
        let card =
            div()
                .w(px(400.))
                .flex()
                .flex_col()
                .gap(px(18.))
                .p(px(20.))
                .rounded(px(12.))
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface2())
                .shadow(colors::shadow())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .gap(px(14.))
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(36.))
                                .rounded(px(9.))
                                .bg(colors::ink(0.06))
                                .child(icon("circle-alert", 18., colors::waiting())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(colors::text1())
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .line_height(px(18.))
                                        .text_color(colors::text2())
                                        .child(line),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1())
                        .child(widgets::button("close-cancel", "Keep working").on_click(
                            cx.listener(|r, _: &ClickEvent, _, cx| {
                                r.closing = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            widgets::button("close-anyway", "Close anyway")
                                .text_color(colors::error())
                                .on_click(cx.listener(|r, _: &ClickEvent, window, _| {
                                    r.close_ok = true;
                                    window.remove_window();
                                })),
                        ),
                );
        let scrim = colors::hsla(colors::theme().shadow);
        Some(
            div()
                .id("close-dialog")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(scrim.opacity(0.6))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|r, _, _, cx| {
                        r.closing = false;
                        cx.notify();
                    }),
                )
                .child(card)
                .into_any_element(),
        )
    }
}
