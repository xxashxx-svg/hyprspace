use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*};

use crate::root::Root;
use crate::zoom::px;
use crate::{colors, widgets};

impl Root {
    pub(crate) fn notice_toast(&self, below: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = self
            .notices
            .toast
            .as_ref()
            .filter(|_| self.state.notify.on)?;
        let (_, thread) = self.state.thread(t.thread)?;
        let id = t.thread;
        let toast = widgets::toast("notice-toast")
            .child(widgets::status_dot(t.kind.status()))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text1())
                            .child(thread.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(colors::text2())
                            .child(t.kind.toast()),
                    ),
            )
            .child(widgets::button("notice-open", "Open").h(px(26.)).on_click(
                cx.listener(move |r, _: &ClickEvent, window, cx| r.open_thread(id, window, cx)),
            ));
        let toast = crate::slide::ease_in(toast, ("notice-toast", t.serial), 180, |d, t| {
            d.opacity(t).mt(px(-8. * (1. - t)))
        });
        Some(
            div()
                .absolute()
                .top(px(if below { 98. } else { 52. }))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(toast)
                .into_any_element(),
        )
    }
}
