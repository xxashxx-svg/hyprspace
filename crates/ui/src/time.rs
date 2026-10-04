// Times the way the sidebar and the resume list show them, and the snooze menu's wake times.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Duration, Local, NaiveDateTime, NaiveTime, TimeZone, Timelike, Weekday};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// "now", "5m", "3h", "2d", "6w" since `then` (unix ms).
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then) / 1000;
    match secs {
        0..60 => "now".into(),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        86_400..1_209_600 => format!("{}d", secs / 86_400),
        _ => format!("{}w", secs / 604_800),
    }
}

/// A snooze choice: what the menu calls it and when it wakes, in local time.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub label: &'static str,
    pub at: NaiveDateTime,
}

/// The snooze menu's choices at `now`, after T3 Code's: in 1 hour, in 3 hours, this evening
/// (while it is still ahead), tomorrow morning and next Monday morning.
pub fn presets(now: NaiveDateTime) -> Vec<Preset> {
    let morning = NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default();
    let evening = NaiveTime::from_hms_opt(18, 0, 0).unwrap_or_default();
    let today = now.date();
    let mut out = vec![
        Preset {
            label: "In 1 hour",
            at: now + Duration::hours(1),
        },
        Preset {
            label: "In 3 hours",
            at: now + Duration::hours(3),
        },
    ];
    // a snooze that wakes in under an hour is no evening
    if now.time() < NaiveTime::from_hms_opt(17, 0, 0).unwrap_or_default() {
        out.push(Preset {
            label: "This evening",
            at: today.and_time(evening),
        });
    }
    out.push(Preset {
        label: "Tomorrow",
        at: (today + Duration::days(1)).and_time(morning),
    });
    let to_monday = 7 - today.weekday().num_days_from_monday() as i64;
    out.push(Preset {
        label: "Next week",
        at: (today + Duration::days(to_monday)).and_time(morning),
    });
    out
}

/// A local time as unix ms.
pub fn to_ms(at: NaiveDateTime) -> u64 {
    Local
        .from_local_datetime(&at)
        .earliest()
        .map_or(0, |t| t.timestamp_millis().max(0) as u64)
}

/// Unix ms as local time.
pub fn local(ms: u64) -> NaiveDateTime {
    Local
        .timestamp_millis_opt(ms as i64)
        .earliest()
        .map(|t| t.naive_local())
        .unwrap_or_default()
}

pub fn local_now() -> NaiveDateTime {
    Local::now().naive_local()
}

/// A clock time the way Windows and macOS show it by default: "9:00 AM".
fn clock(t: NaiveDateTime) -> String {
    let (pm, h) = t.hour12();
    format!("{h}:{:02} {}", t.minute(), if pm { "PM" } else { "AM" })
}

/// When a snooze wakes, from `now`: "5:30 PM" today, "tomorrow 9:00 AM", "Mon 9:00 AM" this
/// week, "Oct 12, 9:00 AM" past that.
pub fn wake_label(at: NaiveDateTime, now: NaiveDateTime) -> String {
    let days = (at.date() - now.date()).num_days();
    match days {
        ..=0 => clock(at),
        1 => format!("tomorrow {}", clock(at)),
        2..=6 => format!("{} {}", weekday(at.weekday()), clock(at)),
        _ => format!("{}, {}", at.format("%b %-d"), clock(at)),
    }
}

/// The menu's short form beside a choice: the clock for today and tomorrow, the day too further
/// out.
/// How long until `at`, from `now`, both unix ms: "in 45m", "in 3h", "in 2d 4h".
pub fn left(at: u64, now: u64) -> String {
    let mins = at.saturating_sub(now) / 60_000;
    match mins {
        0 => "in under a minute".into(),
        1..60 => format!("in {mins}m"),
        60..1440 => match mins % 60 {
            0 => format!("in {}h", mins / 60),
            m => format!("in {}h {m}m", mins / 60),
        },
        _ => match mins / 60 % 24 {
            0 => format!("in {}d", mins / 1440),
            h => format!("in {}d {h}h", mins / 1440),
        },
    }
}

pub fn wake_short(at: NaiveDateTime, now: NaiveDateTime) -> String {
    match (at.date() - now.date()).num_days() {
        ..=1 => clock(at),
        _ => format!("{} {}", weekday(at.weekday()), clock(at)),
    }
}

fn weekday(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn rounds_down_to_the_largest_unit() {
        let now = 10_000_000_000;
        assert_eq!(ago(now - 5_000, now), "now");
        assert_eq!(ago(now - 125_000, now), "2m");
        assert_eq!(ago(now - 3 * 3_600_000, now), "3h");
        assert_eq!(ago(now - 2 * 86_400_000, now), "2d");
        assert_eq!(ago(now - 21 * 86_400_000, now), "3w");
        assert_eq!(ago(now + 9, now), "now");
    }

    #[test]
    fn snooze_choices_follow_the_clock() {
        // a Saturday afternoon
        let now = at(2026, 10, 3, 14, 30);
        let p = presets(now);
        let labels: Vec<_> = p.iter().map(|p| p.label).collect();
        assert_eq!(
            labels,
            [
                "In 1 hour",
                "In 3 hours",
                "This evening",
                "Tomorrow",
                "Next week"
            ]
        );
        assert_eq!(p[2].at, at(2026, 10, 3, 18, 0));
        assert_eq!(p[3].at, at(2026, 10, 4, 9, 0));
        // next week is the coming Monday morning
        assert_eq!(p[4].at, at(2026, 10, 5, 9, 0));
        // late in the day there is no evening left
        assert!(
            !presets(at(2026, 10, 3, 17, 30))
                .iter()
                .any(|p| p.label == "This evening")
        );
        // on a Monday, next week is a week away
        assert_eq!(
            presets(at(2026, 10, 5, 8, 0)).last().unwrap().at,
            at(2026, 10, 12, 9, 0)
        );
    }

    #[test]
    fn wake_times_read_like_a_clock() {
        let now = at(2026, 10, 3, 14, 30);
        assert_eq!(wake_label(at(2026, 10, 3, 17, 5), now), "5:05 PM");
        assert_eq!(wake_label(at(2026, 10, 4, 9, 0), now), "tomorrow 9:00 AM");
        assert_eq!(wake_label(at(2026, 10, 5, 9, 0), now), "Mon 9:00 AM");
        assert_eq!(wake_label(at(2026, 10, 12, 0, 15), now), "Oct 12, 12:15 AM");
        assert_eq!(wake_short(at(2026, 10, 5, 9, 0), now), "Mon 9:00 AM");
    }

    #[test]
    fn time_left_reads_short() {
        let m = 60_000;
        assert_eq!(left(0, 0), "in under a minute");
        assert_eq!(left(45 * m, 0), "in 45m");
        assert_eq!(left(180 * m, 0), "in 3h");
        assert_eq!(left(135 * m, 0), "in 2h 15m");
        assert_eq!(left((2 * 1440 + 240) * m, 0), "in 2d 4h");
        assert_eq!(left(0, 5 * m), "in under a minute");
    }
}
