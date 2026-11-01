// What an agent schedules with its `schedule` tool: a task tied to its own thread, so each run posts
// the task there as a new message. The agent writes times in words; they are read on the user's
// clock, and the answer tells it what they came to.

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use std::time::Duration;

use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*};
use hyprspace_proto::schedule::{ScheduleAsk, Scheduled, When};
use hyprspace_proto::state::{Pick, ThreadKind};
use hyprspace_proto::{Command, SessionId};

use super::{Root, Screen};
use crate::assets::icon;
use crate::composer::{minute_of, schedule_glyph, title_of};
use crate::dock::Automation;
use crate::time::{local, local_now, now_ms, wake_label};
use crate::zoom::px;
use crate::{colors, widgets};

/// How long the toast for a new automation stays.
const MADE_FOR: Duration = Duration::from_secs(6);

const DAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

fn weekday(name: &str) -> Option<u32> {
    let name = name.trim().to_lowercase();
    (name.len() >= 3)
        .then(|| DAYS.iter().position(|d| d.starts_with(&name)))
        .flatten()
        .map(|d| d as u32)
}

/// The next time the clock reads `minute`, on `date` when given, unix ms.
fn once(now: DateTime<Local>, date: Option<&str>, minute: u32) -> Result<u64, String> {
    let day = match date {
        Some(d) => NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d")
            .map_err(|_| "date has to be YYYY-MM-DD.".to_string())?,
        None => now.date_naive(),
    };
    let at = |day: NaiveDate| {
        day.and_hms_opt(minute / 60, minute % 60, 0)
            .and_then(|t| Local.from_local_datetime(&t).earliest())
    };
    let mut t = at(day).ok_or("That time doesn't exist on that day.")?;
    if t <= now && date.is_none() {
        t = at(day.succ_opt().ok_or("That date is out of range.")?)
            .ok_or("That time doesn't exist tomorrow.")?;
    }
    if t <= now {
        return Err("That time has already passed.".into());
    }
    Ok(t.timestamp_millis() as u64)
}

fn when_of(ask: &ScheduleAsk, now: DateTime<Local>) -> Result<When, String> {
    let minute = || {
        ask.time
            .as_deref()
            .and_then(minute_of)
            .ok_or_else(|| "time has to be a time of day, like 23:00 or 11pm.".to_string())
    };
    Ok(match ask.repeat.as_str() {
        "hourly" => When::Every { minutes: 60 },
        "daily" => When::Daily { minute: minute()? },
        "weekly" => When::Weekly {
            day: match &ask.day {
                Some(d) => weekday(d).ok_or("day has to be a day of the week, like wednesday.")?,
                None => now.weekday().num_days_from_monday(),
            },
            minute: minute()?,
        },
        _ => When::Once {
            at: once(now, ask.date.as_deref(), minute()?)?,
        },
    })
}

impl Root {
    /// Tells the dock what the thread on screen has scheduled.
    pub(crate) fn sync_automations(&mut self, cx: &mut Context<Self>) {
        let thread = match self.screen {
            Screen::Thread(id) => Some(id),
            _ => None,
        };
        let now = Local::now();
        let list: Vec<Automation> = self
            .state
            .scheduled
            .iter()
            .filter(|t| thread.is_some() && t.thread == thread)
            .map(|t| Automation {
                id: t.id,
                name: t.name.clone(),
                glyph: schedule_glyph(&t.when),
                when: match t.when {
                    When::Once { .. } => "Once".into(),
                    _ => t.when.label(),
                },
                next: if t.paused {
                    "Paused".into()
                } else {
                    match super::next_run(&t.when, t.last, now) {
                        Some(at) => format!("Next {}", wake_label(local(at), local_now())),
                        None => "Looks at GitHub every 5 minutes".into(),
                    }
                },
                paused: t.paused,
                ran: self
                    .ran
                    .as_ref()
                    .filter(|(task, _, _)| *task == t.id)
                    .map(|(_, n, ran)| (*n, *ran)),
            })
            .collect();
        self.work
            .dock
            .update(cx, |d, cx| d.set_automations(list, thread.is_some(), cx));
    }

    /// A toast for an automation just made, with a way to it and a way to take it back.
    pub(crate) fn show_made(&mut self, task: u64, cx: &mut Context<Self>) {
        let serial = self.made.map_or(0, |(_, n)| n + 1);
        self.made = Some((task, serial));
        self._made_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MADE_FOR).await;
            let _ = this.update(cx, |r, cx| {
                if r.made.is_some_and(|(_, n)| n == serial) {
                    r.made = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    pub(crate) fn made_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (task, serial) = self.made?;
        let t = self.state.scheduled.iter().find(|t| t.id == task)?;
        let next = match super::next_run(&t.when, t.last, Local::now()) {
            Some(at) => format!("Next {}", wake_label(local(at), local_now())),
            None => "Checks GitHub every 5 minutes".into(),
        };
        let action = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex_none()
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.08)))
                .child(label)
        };
        let toast = widgets::toast("made-toast")
            .child(icon(
                crate::composer::schedule_glyph(&t.when),
                14.,
                colors::text2(),
            ))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text1())
                            .child(format!("Scheduled \u{b7} {}", t.when.label())),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(colors::text3())
                            .child(next),
                    ),
            )
            .child(div().flex_none().w(px(1.)).h(px(16.)).bg(colors::border1()))
            .child(
                action("made-view", "View").on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                    r.made = None;
                    r.work.settings = Some(crate::settings::Tab::Scheduled);
                    cx.notify();
                })),
            )
            .child(action("made-undo", "Undo").on_click(cx.listener(
                move |r, _: &ClickEvent, _, cx| {
                    r.made = None;
                    r.state.scheduled.retain(|t| t.id != task);
                    r.save();
                    cx.notify();
                },
            )))
            .child(
                widgets::icon_button("made-close", "x", widgets::SMALL).on_click(cx.listener(
                    |r, _: &ClickEvent, _, cx| {
                        r.made = None;
                        cx.notify();
                    },
                )),
            );
        let toast = crate::slide::ease_in(toast, ("made-toast", serial), 180, |d, t| {
            d.opacity(t).mt(px(-8. * (1. - t)))
        });
        Some(
            div()
                .absolute()
                .top(px(52.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(toast)
                .into_any_element(),
        )
    }

    pub(crate) fn schedule_ask(
        &mut self,
        request: u64,
        parent: SessionId,
        ask: ScheduleAsk,
        cx: &mut Context<Self>,
    ) {
        let (ok, text) = match self.add_automation(parent, ask) {
            Ok(text) => (true, text),
            Err(why) => (false, why),
        };
        self.client.send(Command::Delegated { request, ok, text });
        cx.notify();
    }

    fn add_automation(&mut self, parent: SessionId, ask: ScheduleAsk) -> Result<String, String> {
        let gone = || "This thread is gone.".to_string();
        let thread = *self.sessions.get(&parent).ok_or_else(gone)?;
        let (space, t) = self.state.thread(thread).ok_or_else(gone)?;
        let ThreadKind::Structured { launch } = &t.kind else {
            return Err(gone());
        };
        let (space, launch) = (space.id, launch.clone());
        let now = Local::now();
        let when = when_of(&ask, now)?;
        let name = ask.name.clone().unwrap_or_else(|| title_of(&ask.task));
        let task = Scheduled {
            id: self.state.take_id(),
            name: name.clone(),
            prompt: ask.task,
            space,
            pick: Pick {
                agent: launch.agent,
                model: launch.model.clone().unwrap_or_default(),
                effort: launch.effort.clone().unwrap_or_default(),
                fast: launch.fast,
            },
            permission: launch.permission,
            worktree: false,
            when: when.clone(),
            paused: false,
            last: now_ms(),
            seen: None,
            thread: Some(thread),
        };
        self.state.scheduled.push(task);
        self.save();
        let next = super::next_run(&when, now_ms(), now)
            .map(|at| wake_label(local(at), local_now()))
            .unwrap_or_default();
        let how = match when {
            When::Once { .. } => format!("once, {next}"),
            _ => format!("{}, next {next}", when.label().to_lowercase()),
        };
        Ok(format!(
            "Scheduled \"{name}\" {how}, in the user's local time. It sends the task to this thread as a new message. The user sees it in this thread's Automations, where they can run, pause or delete it."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, h: u32, m: u32) -> DateTime<Local> {
        // 2 November 2026 is a Monday
        Local.with_ymd_and_hms(2026, 11, day, h, m, 0).unwrap()
    }

    fn ask(repeat: &str, time: &str, date: Option<&str>, day: Option<&str>) -> ScheduleAsk {
        ScheduleAsk {
            name: None,
            task: "check memory".into(),
            repeat: repeat.into(),
            time: Some(time.into()),
            date: date.map(Into::into),
            day: day.map(Into::into),
        }
    }

    #[test]
    fn an_agents_times_read_on_the_users_clock() {
        let now = at(2, 20, 0);
        let ms = |t: DateTime<Local>| t.timestamp_millis() as u64;
        assert_eq!(
            when_of(&ask("once", "11pm", None, None), now),
            Ok(When::Once {
                at: ms(at(2, 23, 0))
            })
        );
        assert_eq!(
            when_of(&ask("once", "9:00", None, None), now),
            Ok(When::Once {
                at: ms(at(3, 9, 0))
            })
        );
        assert_eq!(
            when_of(&ask("once", "9:00", Some("2026-11-01"), None), now),
            Err("That time has already passed.".into())
        );
        assert_eq!(
            when_of(&ask("weekly", "23:00", None, Some("Wed")), now),
            Ok(When::Weekly {
                day: 2,
                minute: 23 * 60
            })
        );
        assert!(when_of(&ask("daily", "noon", None, None), now).is_err());
    }
}
