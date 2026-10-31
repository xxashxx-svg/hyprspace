// Scheduled tasks running: each one due starts a thread with its prompt, on the clock or when its
// watch finds something new on the repo's GitHub. Watches look every five minutes.

use std::path::PathBuf;

use chrono::{DateTime, Datelike, Local, TimeZone};
use gpui::Context;
use hyprspace_proto::git::Worktree;
use hyprspace_proto::schedule::{Happening, When};
use hyprspace_proto::{Command, FolderCommand, Launch, Prompt};

use super::{Root, Start};
use crate::time::now_ms;

const WEEK: u64 = 7 * 24 * 60 * 60 * 1000;
/// At most this many threads from one look, so a burst of failures doesn't start dozens.
const BURST: usize = 3;

impl Root {
    pub(super) fn run_scheduled(&mut self, now: DateTime<Local>, cx: &mut Context<Self>) {
        let due: Vec<u64> = self
            .state
            .scheduled
            .iter()
            .filter(|t| !t.paused && due(&t.when, t.last, now))
            .map(|t| t.id)
            .collect();
        for id in due {
            self.fire(id, None, cx);
        }
    }

    pub(super) fn poll_watches(&mut self) {
        let mut asked: Vec<(PathBuf, When)> = Vec::new();
        for t in self
            .state
            .scheduled
            .iter()
            .filter(|t| !t.paused && t.when.watch())
        {
            let Some(cwd) = self.state.space(t.space).and_then(|s| s.cwd.clone()) else {
                continue;
            };
            if !asked.iter().any(|(c, w)| *c == cwd && *w == t.when) {
                asked.push((cwd, t.when.clone()));
            }
        }
        for (cwd, when) in asked {
            self.client
                .send(Command::Folder(FolderCommand::Happened { cwd, when }));
        }
    }

    pub(crate) fn happened(
        &mut self,
        cwd: &PathBuf,
        when: &When,
        items: &[Happening],
        cx: &mut Context<Self>,
    ) {
        let newest = items.iter().map(|h| h.id).max();
        let ids: Vec<u64> = self
            .state
            .scheduled
            .iter()
            .filter(|t| {
                t.when == *when
                    && self.state.space(t.space).and_then(|s| s.cwd.as_ref()) == Some(cwd)
            })
            .map(|t| t.id)
            .collect();
        for id in ids {
            let Some(t) = self.state.scheduled.iter_mut().find(|t| t.id == id) else {
                continue;
            };
            let seen = t.seen;
            t.seen = newest.max(seen);
            let paused = t.paused;
            for h in news(items, seen).into_iter().filter(|_| !paused) {
                self.fire(id, Some(format!("{}\n{}", h.line, h.url)), cx);
            }
        }
        self.save();
    }

    /// Starts the task's thread now, with what a watch found after its prompt.
    pub(crate) fn fire(&mut self, id: u64, found: Option<String>, cx: &mut Context<Self>) {
        let Some(t) = self.state.scheduled.iter_mut().find(|t| t.id == id) else {
            return;
        };
        t.last = now_ms();
        let t = t.clone();
        let Some(space) = self.state.space(t.space) else {
            return;
        };
        let (Some(cwd), setup) = (space.cwd.clone(), space.setup.clone()) else {
            return;
        };
        let launch = Launch {
            agent: t.pick.agent,
            cwd: cwd.clone(),
            model: Some(t.pick.model).filter(|m| !m.is_empty()),
            effort: Some(t.pick.effort).filter(|e| !e.is_empty()),
            permission: t.permission,
            resume: None,
            fast: t.pick.fast,
            rewind: None,
        };
        let text = match found {
            Some(found) => format!("{}\n\n{found}", t.prompt),
            None => t.prompt.clone(),
        };
        let start = Start {
            launch,
            prompt: Some(Prompt::text(text)),
            title: t.name.clone(),
            terminal: false,
        };
        if t.worktree {
            let request = self.next_request;
            self.next_request += 1;
            self.client
                .send(Command::Folder(FolderCommand::AddWorktree {
                    request,
                    cwd,
                    name: t.name,
                    base: None,
                    setup: Some(setup).filter(|s| !s.trim().is_empty()),
                }));
            self.scheduled_starts.insert(request, (t.space, start));
        } else {
            self.add_thread(t.space, start, cx);
        }
        self.save();
    }

    pub(crate) fn scheduled_worktree(
        &mut self,
        request: u64,
        result: &Result<Worktree, String>,
        cx: &mut Context<Self>,
    ) {
        let Some((space, mut start)) = self.scheduled_starts.remove(&request) else {
            return;
        };
        match result {
            Ok(wt) => {
                start.launch.cwd = wt.path.clone();
                if let Some(id) = self.add_thread(space, start, cx)
                    && let Some(t) = self.state.thread_mut(id)
                {
                    t.worktree = Some(wt.clone());
                    self.save();
                }
            }
            Err(e) => self.notify(
                format!("A scheduled task couldn't make its worktree: {e}"),
                cx,
            ),
        }
    }
}

/// When a clock task runs next after `now`, unix ms. Watches have no time.
pub(crate) fn next(when: &When, last: u64, now: DateTime<Local>) -> Option<u64> {
    let at = now.timestamp_millis() as u64;
    let (day, minute) = match when {
        When::Every { minutes } => return Some((last + *minutes as u64 * 60_000).max(at)),
        When::Daily { minute } => (None, *minute),
        When::Weekly { day, minute } => (Some(*day), *minute),
        _ => return None,
    };
    (0..8).find_map(|ahead| {
        let date = now
            .date_naive()
            .checked_add_days(chrono::Days::new(ahead))?;
        if day.is_some_and(|d| date.weekday().num_days_from_monday() != d) {
            return None;
        }
        let t = date.and_hms_opt(minute / 60, minute % 60, 0)?;
        let t = Local.from_local_datetime(&t).earliest()?;
        (t > now).then(|| t.timestamp_millis() as u64)
    })
}

/// What a watch found that it hadn't seen, oldest first and at most a few. Nothing on the first
/// look, which only takes note of what's already there.
fn news(items: &[Happening], seen: Option<u64>) -> Vec<&Happening> {
    let Some(seen) = seen else {
        return Vec::new();
    };
    let mut fresh: Vec<&Happening> = items.iter().filter(|h| h.id > seen).collect();
    fresh.sort_by_key(|h| h.id);
    fresh.truncate(BURST);
    fresh
}

/// Whether a clock task's time came since `last`. A time missed while the app was closed still
/// counts once, up to a week back.
fn due(when: &When, last: u64, now: DateTime<Local>) -> bool {
    let at = now.timestamp_millis() as u64;
    let latest = match when {
        When::Every { minutes } => return at.saturating_sub(last) >= *minutes as u64 * 60_000,
        When::Daily { minute } => latest(now, None, *minute),
        When::Weekly { day, minute } => latest(now, Some(*day), *minute),
        _ => return false,
    };
    latest.is_some_and(|t| t > last && at - t < WEEK)
}

/// The latest time at `minute` past midnight, on `day` when given, at or before `now`, unix ms.
fn latest(now: DateTime<Local>, day: Option<u32>, minute: u32) -> Option<u64> {
    (0..8).find_map(|back| {
        let date = now.date_naive().checked_sub_days(chrono::Days::new(back))?;
        if day.is_some_and(|d| date.weekday().num_days_from_monday() != d) {
            return None;
        }
        let at = date.and_hms_opt(minute / 60, minute % 60, 0)?;
        let at = Local.from_local_datetime(&at).earliest()?;
        (at <= now).then(|| at.timestamp_millis() as u64)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, h: u32, m: u32) -> DateTime<Local> {
        // 2 November 2026 is a Monday
        Local.with_ymd_and_hms(2026, 11, day, h, m, 0).unwrap()
    }

    fn ms(t: DateTime<Local>) -> u64 {
        t.timestamp_millis() as u64
    }

    #[test]
    fn a_watch_starts_on_what_is_new_after_its_first_look() {
        let h = |id| Happening {
            id,
            line: format!("#{id}"),
            url: String::new(),
        };
        let items: Vec<_> = (1..=10).rev().map(h).collect();
        assert!(news(&items, None).is_empty());
        let ids = |seen| {
            news(&items, Some(seen))
                .iter()
                .map(|h| h.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(8), [9, 10]);
        assert_eq!(ids(2), [3, 4, 5]);
        assert!(ids(10).is_empty());
    }

    #[test]
    fn a_clock_task_runs_once_each_time_it_comes_round() {
        let daily = When::Daily { minute: 9 * 60 };
        let yesterday = ms(at(1, 10, 0));
        assert!(!due(&daily, yesterday, at(2, 8, 59)));
        assert!(due(&daily, yesterday, at(2, 9, 30)));
        assert!(!due(&daily, ms(at(2, 9, 30)), at(2, 15, 0)));
        // closed over the time, it runs on opening
        assert!(due(&daily, ms(at(1, 8, 0)), at(2, 7, 0)));

        let mondays = When::Weekly {
            day: 0,
            minute: 9 * 60,
        };
        assert!(due(&mondays, ms(at(1, 12, 0)), at(2, 9, 1)));
        assert!(!due(&mondays, ms(at(2, 9, 1)), at(3, 9, 1)));

        assert_eq!(next(&daily, 0, at(2, 8, 0)), Some(ms(at(2, 9, 0))));
        assert_eq!(next(&daily, 0, at(2, 9, 0)), Some(ms(at(3, 9, 0))));
        assert_eq!(next(&mondays, 0, at(3, 9, 0)), Some(ms(at(9, 9, 0))));
        assert_eq!(next(&When::CiFails, 0, at(2, 9, 0)), None);

        let hourly = When::Every { minutes: 60 };
        assert_eq!(
            next(&hourly, ms(at(2, 9, 0)), at(2, 9, 30)),
            Some(ms(at(2, 10, 0)))
        );
        assert!(!due(&hourly, ms(at(2, 9, 0)), at(2, 9, 59)));
        assert!(due(&hourly, ms(at(2, 9, 0)), at(2, 10, 0)));
        assert!(!due(&When::CiFails, 0, at(2, 9, 0)));
    }
}
