// The Settings screen, opened from the sidebar's foot. A placeholder for now: a heading and a way
// back. The theme logic it will need is `Root::apply_theme` and `colors::dark`.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*, px};

use crate::colors;
use crate::root::Root;
use crate::widgets;

impl Root {
    pub(crate) fn settings(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .px(px(32.))
            .py(px(28.))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child("Settings"),
            )
            .child(
                div()
                    .flex()
                    .child(widgets::button("settings-back", "Back").on_click(
                        cx.listener(|r, _: &ClickEvent, window, cx| r.close_settings(window, cx)),
                    )),
            )
            .into_any_element()
    }
}
