// Scheduling the message in the box instead of sending it, from the clock beside Send. The task
// keeps the box's project, agent, model, permission and worktree choice.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, MouseButton, Pixels, Point, Window, div,
    prelude::*,
};
use hyprspace_proto::schedule::When;
use hyprspace_theme::MONO;

use super::{Composer, ComposerEvent, title_of};
use crate::assets::icon;
use crate::input::TextInput;
use crate::zoom::px;
use crate::{colors, widgets};

const TIME: &str = "Write the time as 9:00 or 21:30.";
const LABEL: &str = "Write the label to watch for.";

impl Composer {
    pub(super) fn can_schedule(&self) -> bool {
        !self.remote() && self.target.as_ref().is_some_and(|t| t.cwd.is_some())
    }

    pub(super) fn open_schedule(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.schedule_menu = Some(e.position());
        cx.notify();
    }

    fn at(&self, cx: &Context<Self>) -> Option<u32> {
        minute_of(self.schedule_at.read(cx).text())
    }

    pub(super) fn schedule_daily(&mut self, cx: &mut Context<Self>) {
        let when = self.at(cx).map(|minute| When::Daily { minute });
        self.schedule(when.ok_or(TIME), cx)
    }

    pub(super) fn schedule_labeled(&mut self, cx: &mut Context<Self>) {
        let label = self.schedule_label.read(cx).text().trim().to_string();
        self.schedule(
            (!label.is_empty())
                .then_some(When::Labeled { label })
                .ok_or(LABEL),
            cx,
        )
    }

    fn schedule(&mut self, when: Result<When, &'static str>, cx: &mut Context<Self>) {
        let when = match when {
            Ok(when) => when,
            Err(why) => {
                self.error = Some(why.into());
                cx.notify();
                return;
            }
        };
        let text = self.input.read(cx).text().trim().to_string();
        let (Some(space), Some(agent)) = (
            self.target.as_ref().map(|t| t.space),
            self.agent().map(|a| a.agent),
        ) else {
            return;
        };
        self.schedule_menu = None;
        if text.is_empty() {
            self.error = Some("Write what each run should do first.".into());
            cx.notify();
            return;
        }
        cx.emit(ComposerEvent::Schedule {
            space,
            name: title_of(&text),
            prompt: text,
            pick: self.pick().unwrap_or_else(|| self.prefs.pick(agent)),
            permission: self.prefs.permission,
            worktree: self.worktree_on(),
            when,
        });
        self.reset(cx);
        cx.notify();
    }
}

pub(crate) fn glyph(when: &When) -> &'static str {
    match when {
        When::Every { .. } => "rotate-cw",
        When::Daily { .. } => "sun",
        When::Weekly { .. } => "calendar",
        When::CiFails => "circle-alert",
        When::PrOpens => "git-pull-request",
        When::Labeled { .. } => "tag",
    }
}

/// "9:00", "21:30" or "9" as minutes past midnight.
fn minute_of(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':').unwrap_or((text.trim(), "0"));
    let (h, m) = (h.trim().parse::<u32>().ok()?, m.trim().parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

fn field(input: &gpui::Entity<TextInput>, width: f32) -> AnyElement {
    div()
        .id("schedule-field")
        .flex_none()
        .w(px(width))
        .h(px(22.))
        .flex()
        .items_center()
        .px(px(6.))
        .rounded(px(5.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.03))
        .font_family(MONO)
        .text_size(px(11.5))
        .text_color(colors::text1())
        .cursor_text()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(input.clone())
        .into_any_element()
}

fn section(label: &'static str) -> AnyElement {
    div()
        .px(px(10.))
        .pt(px(8.))
        .pb(px(3.))
        .text_size(px(11.))
        .text_color(colors::text3())
        .child(label)
        .into_any_element()
}

pub(super) fn menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    type Pick = fn(&Composer, &Context<Composer>) -> Result<When, &'static str>;
    let time = c.schedule_at.read(cx).text().trim().to_string();
    let time = if time.is_empty() {
        "9:00".to_string()
    } else {
        time
    };
    let row =
        |id: usize, glyph: &'static str, label: String, extra: Option<AnyElement>, pick: Pick| {
            div()
                .id(("schedule", id))
                .flex()
                .items_center()
                .gap(px(9.))
                .h(px(30.))
                .px(px(10.))
                .rounded(px(7.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.06)))
                .child(icon(glyph, 13., colors::text3()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(colors::text1())
                        .child(label),
                )
                .children(extra)
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                    let when = pick(c, cx);
                    c.schedule(when, cx)
                }))
                .into_any_element()
        };
    let clock = [
        row(0, "rotate-cw", "Every hour".into(), None, |_, _| {
            Ok(When::Every { minutes: 60 })
        }),
        row(1, "rotate-cw", "Every 6 hours".into(), None, |_, _| {
            Ok(When::Every { minutes: 360 })
        }),
        row(
            2,
            "sun",
            "Every day at".into(),
            Some(field(&c.schedule_at, 54.)),
            |c, cx| c.at(cx).map(|minute| When::Daily { minute }).ok_or(TIME),
        ),
        row(
            3,
            "calendar",
            format!("Mondays at {time}"),
            None,
            |c, cx| {
                c.at(cx)
                    .map(|minute| When::Weekly { day: 0, minute })
                    .ok_or(TIME)
            },
        ),
    ];
    let github = [
        row(4, "circle-alert", "When CI fails".into(), None, |_, _| {
            Ok(When::CiFails)
        }),
        row(
            5,
            "git-pull-request",
            "When a pull request opens".into(),
            None,
            |_, _| Ok(When::PrOpens),
        ),
        row(
            6,
            "tag",
            "When an issue is labeled".into(),
            Some(field(&c.schedule_label, 70.)),
            |c, cx| {
                let label = c.schedule_label.read(cx).text().trim().to_string();
                (!label.is_empty())
                    .then_some(When::Labeled { label })
                    .ok_or(LABEL)
            },
        ),
    ];
    let body = div()
        .w(px(272.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(
            div()
                .px(px(10.))
                .pt(px(4.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child("Run on a schedule"),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("Each run starts a new thread."),
                ),
        )
        .child(section("On the clock"))
        .children(clock)
        .child(section("On GitHub"))
        .children(github)
        .into_any_element();
    let close = cx.listener(|c, _: &(), _, cx| {
        c.schedule_menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_with_or_without_minutes() {
        assert_eq!(minute_of("9:00"), Some(540));
        assert_eq!(minute_of(" 21:30 "), Some(1290));
        assert_eq!(minute_of("7"), Some(420));
        assert_eq!(minute_of("24:00"), None);
        assert_eq!(minute_of("noon"), None);
    }
}
