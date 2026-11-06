// Scheduling the message in the box instead of sending it. The clock beside Send turns the box
// into an automation: a line over the message reads as a sentence ("Run every day at 9:00 AM in
// HyprSpace"), each part changed where it stands, and Send becomes Schedule. The task keeps the
// box's project, agent, model, permission and worktree choice.

use chrono::{Datelike, Local};
use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, MouseButton, Pixels, Point, Window, div,
    prelude::*,
};
use hyprspace_proto::schedule::When;
use hyprspace_theme::MONO;

use super::{Composer, ComposerEvent, title_of};
use crate::assets::icon;
use crate::input::TextInput;
use crate::time::{local, local_now, wake_label};
use crate::zoom::px;
use crate::{colors, widgets};

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

const TIME: &str = "Write the time as 9:00 AM or 9:30 PM.";
const LABEL: &str = "Write the label to watch for.";

impl Composer {
    pub(super) fn can_schedule(&self) -> bool {
        !self.remote() && self.target.as_ref().is_some_and(|t| t.cwd.is_some())
    }

    pub(super) fn planning(&self) -> bool {
        self.plan.is_some()
    }

    /// The clock: into an automation, every day to start with, or back to a thread that starts now.
    pub(super) fn toggle_plan(&mut self, cx: &mut Context<Self>) {
        self.plan = match self.plan {
            Some(_) => None,
            None => Some(When::Daily { minute: 9 * 60 }),
        };
        self.schedule_menu = None;
        self.error = None;
        cx.notify();
    }

    fn open_schedule(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.schedule_menu = Some(e.position());
        cx.notify();
    }

    fn pick_plan(&mut self, when: When, cx: &mut Context<Self>) {
        self.plan = Some(when);
        self.schedule_menu = None;
        self.error = None;
        cx.notify();
    }

    fn at(&self, cx: &Context<Self>) -> Option<u32> {
        minute_of(self.schedule_at.read(cx).text())
    }

    /// The plan with its time and label read from their fields.
    fn plan_when(&self, cx: &Context<Self>) -> Result<When, &'static str> {
        match self.plan.clone().ok_or(TIME)? {
            When::Daily { .. } => self.at(cx).map(|minute| When::Daily { minute }).ok_or(TIME),
            When::Weekly { day, .. } => self
                .at(cx)
                .map(|minute| When::Weekly { day, minute })
                .ok_or(TIME),
            When::Labeled { .. } => {
                let label = self.schedule_label.read(cx).text().trim().to_string();
                (!label.is_empty())
                    .then_some(When::Labeled { label })
                    .ok_or(LABEL)
            }
            when => Ok(when),
        }
    }

    pub(super) fn schedule_plan(&mut self, cx: &mut Context<Self>) {
        let when = self.plan_when(cx);
        self.schedule(when, cx)
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
        self.plan = None;
        self.reset(cx);
        cx.notify();
    }
}

pub(crate) fn glyph(when: &When) -> &'static str {
    match when {
        When::Once { .. } => "clock",
        When::Every { .. } => "rotate-cw",
        When::Daily { .. } => "sun",
        When::Weekly { .. } => "calendar",
        When::CiFails => "circle-alert",
        When::PrOpens => "git-pull-request",
        When::Labeled { .. } => "tag",
    }
}

/// "9:00 AM", "9pm", "21:30" or "9" as minutes past midnight.
pub(crate) fn minute_of(text: &str) -> Option<u32> {
    let text = text.trim().to_lowercase();
    let (text, half) = match text.strip_suffix('m').unwrap_or(&text) {
        t if t.ends_with('a') => (t.trim_end_matches('a'), Some(false)),
        t if t.ends_with('p') => (t.trim_end_matches('p'), Some(true)),
        _ => (text.as_str(), None),
    };
    let text = text.trim();
    let (h, m) = text.split_once(':').unwrap_or((text, "0"));
    let (h, m) = (h.trim().parse::<u32>().ok()?, m.trim().parse::<u32>().ok()?);
    let h = match half {
        None if h < 24 => h,
        Some(pm) if (1..=12).contains(&h) => h % 12 + if pm { 12 } else { 0 },
        _ => return None,
    };
    (m < 60).then_some(h * 60 + m)
}

fn field(id: &'static str, input: &gpui::Entity<TextInput>, width: f32) -> AnyElement {
    div()
        .id(id)
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

/// The sentence over the message while the box sets up an automation.
pub(super) fn when_line(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    let plan = c.plan.clone()?;
    let tint = colors::accent();
    let token = |id: &'static str, text: String| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(24.))
            .px(px(9.))
            .rounded(px(7.))
            .bg(tint.opacity(0.12))
            .text_color(tint)
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .hover(|s| s.bg(tint.opacity(0.2)))
            .child(text)
            .child(icon("chevron-down", 10., tint))
    };
    let trigger = match &plan {
        When::Every { minutes: 60 } => "every hour".to_string(),
        When::Every { minutes } => format!("every {} hours", minutes / 60),
        When::Daily { .. } => "every day".into(),
        When::Weekly { .. } => "every week on".into(),
        When::CiFails => "when CI fails".into(),
        When::PrOpens => "when a pull request opens".into(),
        When::Labeled { .. } => "when an issue is labeled".into(),
        When::Once { .. } => "once".into(),
    };
    let trigger = token("plan-trigger", trigger)
        .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_schedule(e, cx)));
    let day = match plan {
        When::Weekly { day, .. } => Some(
            token("plan-day", DAYS[day as usize % 7].to_string()).on_click(cx.listener(
                move |c, _: &ClickEvent, _, cx| {
                    c.pick_plan(
                        When::Weekly {
                            day: (day + 1) % 7,
                            minute: 0,
                        },
                        cx,
                    )
                },
            )),
        ),
        _ => None,
    };
    let at = matches!(plan, When::Daily { .. } | When::Weekly { .. });
    let next = match c.plan_when(cx) {
        Ok(when) if when.watch() => "Checks GitHub every 5 minutes".to_string(),
        Ok(when) => crate::root::next_run(&when, crate::time::now_ms(), Local::now())
            .map(|t| format!("Next {}", wake_label(local(t), local_now())))
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    let place = c
        .target
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_default();
    Some(
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(7.))
            .px(px(14.))
            .py(px(10.))
            .text_size(px(13.))
            .text_color(colors::text2())
            .child(icon("clock", 14., tint))
            .child("Run")
            .child(trigger)
            .children(day)
            .when(at, |d| {
                d.child("at").child(field("plan-at", &c.schedule_at, 80.))
            })
            .when(matches!(plan, When::Labeled { .. }), |d| {
                d.child(field("plan-label", &c.schedule_label, 90.))
            })
            .when(!place.is_empty(), |d| d.child(format!("in {place}")))
            .when(c.worktree_on(), |d| d.child("on a new worktree"))
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(next),
            )
            .child(
                widgets::icon_button("plan-off", "x", 22.)
                    .tooltip(widgets::tip("Start it now instead"))
                    .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.toggle_plan(cx))),
            )
            .into_any_element(),
    )
}

/// What starts the automation, picked from the trigger on the line.
pub(super) fn menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let minute = c.at(cx).unwrap_or(9 * 60);
    let today = Local::now().weekday().num_days_from_monday();
    let row = |id: usize, glyph: &'static str, label: &'static str, when: When| {
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
            .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| c.pick_plan(when.clone(), cx)))
            .into_any_element()
    };
    let body = div()
        .w(px(240.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(section("On the clock"))
        .child(row(
            0,
            "rotate-cw",
            "Every hour",
            When::Every { minutes: 60 },
        ))
        .child(row(
            1,
            "rotate-cw",
            "Every 6 hours",
            When::Every { minutes: 360 },
        ))
        .child(row(2, "sun", "Every day", When::Daily { minute }))
        .child(row(
            3,
            "calendar",
            "Every week",
            When::Weekly { day: today, minute },
        ))
        .child(section("On GitHub"))
        .child(row(4, "circle-alert", "When CI fails", When::CiFails))
        .child(row(
            5,
            "git-pull-request",
            "When a pull request opens",
            When::PrOpens,
        ))
        .child(row(
            6,
            "tag",
            "When an issue is labeled",
            When::Labeled {
                label: String::new(),
            },
        ))
        .into_any_element();
    let close = cx.listener(|c, _: &(), _, cx| {
        c.schedule_menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Down,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

#[cfg(test)]
mod tests {
    use hyprspace_proto::schedule::clock;

    use super::*;

    #[test]
    fn times_read_with_or_without_minutes() {
        assert_eq!(minute_of("9:00"), Some(540));
        assert_eq!(minute_of(" 21:30 "), Some(1290));
        assert_eq!(minute_of("7"), Some(420));
        assert_eq!(minute_of("24:00"), None);
        assert_eq!(minute_of("noon"), None);
    }

    #[test]
    fn times_read_on_a_12_hour_clock() {
        assert_eq!(minute_of("9:00 AM"), Some(540));
        assert_eq!(minute_of("10pm"), Some(1320));
        assert_eq!(minute_of("9:30 p"), Some(1290));
        assert_eq!(minute_of("12 am"), Some(0));
        assert_eq!(minute_of("12:15 PM"), Some(735));
        assert_eq!(minute_of("13pm"), None);
        assert_eq!(minute_of("0am"), None);
        assert_eq!(clock(0), "12:00 AM");
        assert_eq!(clock(1290), "9:30 PM");
    }
}
