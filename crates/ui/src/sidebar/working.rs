// The Working shelf, a beta switched on in Settings, General, after T3 Code's: a thread folds
// into it while its agent works, or its subagents do after the run ended, and comes back to the
// top of the list once it finishes or needs you. A terminal that never says it works (Codex, a
// plain shell) stays where it is.

use std::time::{Duration, Instant};

use gpui::{App, Context};
use hyprspace_proto::Thread;

use crate::root::Root;
use crate::time::now_ms;
use crate::transcript::Status;

/// A thread that just finished stays this long, so a run that ends a moment before the next
/// one starts doesn't jump out and back.
const HOLD: Duration = Duration::from_secs(2);

fn on_shelf(t: &Thread, busy: bool, left: Option<Instant>, now: Instant) -> bool {
    let held = left.is_some_and(|at| now.duration_since(at) < HOLD);
    t.active() && t.pinned.is_none() && (busy || held)
}

impl Root {
    /// Working, or done with subagents still running. One waiting on you isn't.
    fn shelf_busy(&self, thread: u64, cx: &App) -> bool {
        match self.status.get(&thread) {
            Some(Status::Working) => true,
            Some(Status::Waiting | Status::Failed) => false,
            _ => !self.subagents(thread, cx).is_empty(),
        }
    }

    pub(crate) fn on_working_shelf(&self, t: &Thread, cx: &App) -> bool {
        self.state.working_shelf
            && on_shelf(
                t,
                self.shelf_busy(t.id, cx),
                self.shelf.get(&t.id).copied().flatten(),
                Instant::now(),
            )
    }

    /// Looks again at whether `thread` is busy, after its status, a run event or its subagents
    /// changed: subagents ending change no status. One that stops comes back on top of the
    /// list, at once if it needs you and after `HOLD` if it finished.
    pub(crate) fn shelf_look(&mut self, thread: u64, cx: &mut Context<Self>) {
        if !self.state.working_shelf {
            return;
        }
        if self.shelf_busy(thread, cx) {
            if self.shelf.insert(thread, None) != Some(None) {
                cx.notify();
            }
            return;
        }
        if self.shelf.get(&thread) != Some(&None) {
            return;
        }
        if matches!(
            self.status.get(&thread),
            Some(Status::Waiting | Status::Failed)
        ) {
            self.shelf.remove(&thread);
            self.back_on_top(thread);
            return cx.notify();
        }
        let at = Instant::now();
        self.shelf.insert(thread, Some(at));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOLD).await;
            let _ = this.update(cx, |r, cx| {
                if r.shelf.get(&thread) == Some(&Some(at)) {
                    r.shelf.remove(&thread);
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
    fn a_busy_thread_is_on_the_shelf_until_its_hold_runs_out() {
        let t = Thread::default();
        let now = Instant::now();
        assert!(on_shelf(&t, true, None, now));
        assert!(!on_shelf(&t, false, None, now));
        let left = Some(now);
        assert!(on_shelf(&t, false, left, now + HOLD / 2));
        assert!(!on_shelf(&t, false, left, now + HOLD));
    }

    #[test]
    fn pinned_snoozed_and_settled_threads_stay_where_they_are() {
        let now = Instant::now();
        let busy = |t: Thread| on_shelf(&t, true, None, now);
        assert!(!busy(Thread {
            pinned: Some(1),
            ..Thread::default()
        }));
        assert!(!busy(Thread {
            snooze: Some(Snooze::Done),
            ..Thread::default()
        }));
        assert!(!busy(Thread {
            settled: true,
            ..Thread::default()
        }));
    }
}
