// The Working shelf, a beta switched on in Settings, General, after T3 Code's: a thread folds
// into it while its agent works, and comes back to the top of the list once it finishes or
// needs you. Only a status change moves it, so a terminal that never says it works (Codex, a
// plain shell) stays where it is.

use std::time::{Duration, Instant};

use gpui::Context;
use hyprspace_proto::Thread;

use crate::root::Root;
use crate::time::now_ms;
use crate::transcript::Status;

/// A thread that just finished stays this long, so a run that ends a moment before the next
/// one starts doesn't jump out and back.
const HOLD: Duration = Duration::from_secs(2);

fn on_shelf(t: &Thread, status: Option<Status>, left: Option<Instant>, now: Instant) -> bool {
    let held = left.is_some_and(|at| now.duration_since(at) < HOLD);
    t.active() && t.pinned.is_none() && (status == Some(Status::Working) || held)
}

impl Root {
    pub(crate) fn on_working_shelf(&self, t: &Thread) -> bool {
        self.state.working_shelf
            && on_shelf(
                t,
                self.status.get(&t.id).copied(),
                self.left_work.get(&t.id).copied(),
                Instant::now(),
            )
    }

    /// A thread's status changed from `before`. One that stops working comes back on top of the
    /// list: at once if it needs you, after `HOLD` if it finished.
    pub(crate) fn shelf_status(
        &mut self,
        thread: u64,
        before: Option<Status>,
        status: Status,
        cx: &mut Context<Self>,
    ) {
        if status == Status::Working {
            self.left_work.remove(&thread);
            return;
        }
        if !self.state.working_shelf || before != Some(Status::Working) {
            return;
        }
        if !matches!(status, Status::Done | Status::Idle) {
            return self.back_on_top(thread);
        }
        let at = Instant::now();
        self.left_work.insert(thread, at);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOLD).await;
            let _ = this.update(cx, |r, cx| {
                if r.left_work.get(&thread) == Some(&at) {
                    r.left_work.remove(&thread);
                    r.back_on_top(thread);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// A host keeps its own threads' order, so only this computer's move.
    fn back_on_top(&mut self, thread: u64) {
        let local = self
            .state
            .thread(thread)
            .is_some_and(|(s, t)| s.machine.is_none() && t.active() && t.pinned.is_none());
        if let Some(t) = self.state.thread_mut(thread).filter(|_| local) {
            t.order = Some(now_ms() as i64);
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use hyprspace_proto::Snooze;

    use super::*;

    #[test]
    fn a_working_thread_is_on_the_shelf_until_its_hold_runs_out() {
        let t = Thread::default();
        let now = Instant::now();
        assert!(on_shelf(&t, Some(Status::Working), None, now));
        assert!(!on_shelf(&t, Some(Status::Waiting), None, now));
        assert!(!on_shelf(&t, Some(Status::Done), None, now));
        assert!(!on_shelf(&t, None, None, now));
        let left = Some(now);
        assert!(on_shelf(&t, Some(Status::Done), left, now + HOLD / 2));
        assert!(!on_shelf(&t, Some(Status::Done), left, now + HOLD));
    }

    #[test]
    fn pinned_snoozed_and_settled_threads_stay_where_they_are() {
        let now = Instant::now();
        let working = |t: Thread| on_shelf(&t, Some(Status::Working), None, now);
        assert!(!working(Thread {
            pinned: Some(1),
            ..Thread::default()
        }));
        assert!(!working(Thread {
            snooze: Some(Snooze::Done),
            ..Thread::default()
        }));
        assert!(!working(Thread {
            settled: true,
            ..Thread::default()
        }));
    }
}
