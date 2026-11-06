use gpui::{AnyElement, ClickEvent, Context, prelude::*};

use crate::root::Root;
use crate::widgets;
use crate::zoom::px;

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
            .child(widgets::toast_text(thread.title.clone(), t.kind.toast()))
            .child(widgets::button("notice-open", "Open").h(px(26.)).on_click(
                cx.listener(move |r, _: &ClickEvent, window, cx| r.open_thread(id, window, cx)),
            ));
        Some(widgets::toast_at(
            toast,
            ("notice-toast", t.serial),
            below as usize,
        ))
    }
}
