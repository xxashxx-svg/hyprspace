use gpui::{
    AnyElement, Context, Entity, Focusable, IntoElement, Subscription, Window, div, prelude::*,
};
use hyprspace_proto::Snooze;

use super::Root;
use crate::colors;
use crate::input::{InputEvent, TextInput};
use crate::time::{local_now, parse_when, to_ms, wake_label};
use crate::zoom::px;

pub(crate) struct CustomSnooze {
    input: Entity<TextInput>,
    _keys: Subscription,
}

impl Root {
    pub(crate) fn open_custom_snooze(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, thread)) = self.snooze_menu else {
            return;
        };
        let input = cx.new(|cx| TextInput::new("2h, 3pm, tomorrow 10am", false, cx));
        let keys = cx.subscribe_in(
            &input,
            window,
            move |r, input, e: &InputEvent, window, cx| match e {
                InputEvent::Submit => {
                    if let Some(at) = parse_when(input.read(cx).text(), local_now()) {
                        r.snooze_custom = None;
                        r.snooze(thread, Some(Snooze::Time { at: to_ms(at) }), window, cx);
                    }
                }
                InputEvent::Cancel => {
                    r.snooze_custom = None;
                    r.snooze_menu = None;
                    cx.notify();
                }
                _ => cx.notify(),
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.snooze_custom = Some(CustomSnooze { input, _keys: keys });
        cx.notify();
    }

    pub(crate) fn custom_snooze_body(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.snooze_custom.as_ref()?;
        let now = local_now();
        let typed = c.input.read(cx).text().trim().to_string();
        let (note, tint) = match parse_when(&typed, now) {
            Some(at) => (format!("Wakes {}", wake_label(at, now)), colors::text2()),
            None if typed.is_empty() => ("Enter to snooze".to_string(), colors::text3()),
            None => ("Not a time I can read".to_string(), colors::text3()),
        };
        Some(
            div()
                .w(px(250.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(5.))
                .child(
                    div()
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .px(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors::border2())
                        .text_size(px(12.5))
                        .child(c.input.clone()),
                )
                .child(
                    div()
                        .px(px(4.))
                        .text_size(px(11.5))
                        .text_color(tint)
                        .child(note),
                )
                .into_any_element(),
        )
    }
}
