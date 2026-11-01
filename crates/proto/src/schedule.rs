// Tasks that start a thread with the same prompt on a schedule, or when something happens on the
// repo's GitHub, kept in Settings, Scheduled. They run while the app is open; a time missed while
// it was closed runs once when it opens again.

use serde::{Deserialize, Serialize};

use crate::run::Permission;
use crate::state::Pick;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scheduled {
    pub id: u64,
    pub name: String,
    pub prompt: String,
    pub space: u64,
    pub pick: Pick,
    pub permission: Permission,
    /// Each run in a worktree of its own.
    #[serde(default)]
    pub worktree: bool,
    pub when: When,
    #[serde(default)]
    pub paused: bool,
    /// The last run, or when it was made, unix ms.
    pub last: u64,
    /// For a watch, the newest run, pull request or issue number seen, so only newer ones start a
    /// thread. None until the first look, which only takes note.
    #[serde(default)]
    pub seen: Option<u64>,
    /// The thread each run posts its prompt into, instead of starting one: the thread whose agent
    /// set it up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<u64>,
}

/// What an agent asked for with its `schedule` tool, as it wrote it. The UI reads the times in the
/// user's own clock and answers with what it made of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleAsk {
    pub name: Option<String>,
    pub task: String,
    /// once, hourly, daily or weekly.
    pub repeat: String,
    /// The local time of day, like 23:00 or 11pm.
    pub time: Option<String>,
    /// For once: the local date, YYYY-MM-DD.
    pub date: Option<String>,
    /// For weekly: the day, like wednesday.
    pub day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum When {
    /// Once, at `at`, unix ms. The task goes once it has run.
    Once {
        at: u64,
    },
    Every {
        minutes: u32,
    },
    /// At `minute` past local midnight every day.
    Daily {
        minute: u32,
    },
    /// At `minute` past local midnight on `day` each week, 0 being Monday.
    Weekly {
        day: u32,
        minute: u32,
    },
    CiFails,
    PrOpens,
    Labeled {
        label: String,
    },
}

const DAYS: [&str; 7] = [
    "Mondays",
    "Tuesdays",
    "Wednesdays",
    "Thursdays",
    "Fridays",
    "Saturdays",
    "Sundays",
];

/// A minute of the day on a 12-hour clock: "9:00 AM".
pub fn clock(minute: u32) -> String {
    let (h, m) = (minute / 60 % 24, minute % 60);
    let half = if h < 12 { "AM" } else { "PM" };
    let h = match h % 12 {
        0 => 12,
        h => h,
    };
    format!("{h}:{m:02} {half}")
}

impl When {
    /// Starts on something happening on GitHub rather than on the clock.
    pub fn watch(&self) -> bool {
        matches!(self, When::CiFails | When::PrOpens | When::Labeled { .. })
    }

    pub fn label(&self) -> String {
        let at = |m: &u32| clock(*m);
        match self {
            When::Once { .. } => "Once".into(),
            When::Every { minutes: 60 } => "Every hour".into(),
            When::Every { minutes } if minutes % 60 == 0 => format!("Every {} hours", minutes / 60),
            When::Every { minutes } => format!("Every {minutes} minutes"),
            When::Daily { minute } => format!("Every day at {}", at(minute)),
            When::Weekly { day, minute } => {
                format!("{} at {}", DAYS[*day as usize % 7], at(minute))
            }
            When::CiFails => "When CI fails".into(),
            When::PrOpens => "When a pull request opens".into(),
            When::Labeled { label } => format!("When an issue is labeled {label}"),
        }
    }
}

/// A run, pull request or issue a watch found: its number, a line about it and its link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Happening {
    pub id: u64,
    pub line: String,
    pub url: String,
}
