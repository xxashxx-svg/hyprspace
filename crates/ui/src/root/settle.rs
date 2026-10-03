// Settling and snoozing threads, after T3 Code's. Settled threads leave their space for the
// Settled shelf at the bottom of the sidebar; untouched ones settle by themselves after a while. A snoozed thread hides
// until a time, or until its agent finishes, and comes back marked new. Either can be undone for
// a few seconds from the toast.
//
// A settled thread that is idle gives up its terminal and agent process; the conversation
// resumes when it is opened again. That is what keeps dozens of threads cheap. A plain shell has
// nothing to resume, and may be running a dev server or a watcher, so it keeps its terminal.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    prelude::*, px,
};
use hyprspace_proto::{Pane, Snooze, ThreadKind};

use super::{Root, Screen};
use crate::assets::icon;
use crate::colors;
use crate::time::{local, local_now, now_ms, presets, to_ms, wake_label, wake_short};
use crate::transcript::Status;
use crate::widgets;

/// How long the toast offers Undo.
const UNDO_FOR: Duration = Duration::from_secs(5);

/// What the toast can put back.
#[derive(Clone)]
pub(crate) struct Undo {
    thread: u64,
    what: &'static str,
    settled: bool,
    snooze: Option<Snooze>,
    at: Instant,
    /// Tells this toast from the last, so each one eases in.
    serial: usize,
}

impl Root {
    fn busy(&self, thread: u64) -> bool {
        matches!(
            self.status.get(&thread),
            Some(Status::Working | Status::Waiting)
        )
    }

    /// On screen in any space's grid, which auto-settle leaves alone.
    fn on_screen(&self, thread: u64) -> bool {
        self.screen == Screen::Thread(thread)
            || self
                .state
                .spaces
                .iter()
                .any(|s| s.grid.panes.contains(&Pane::Thread { id: thread }))
    }

    /// Gives up an idle thread's session when its conversation can resume; a busy one keeps it
    /// until its turn ends.
    pub(crate) fn free(&mut self, thread: u64) {
        let resumes = self
            .state
            .thread(thread)
            .is_some_and(|(_, t)| match &t.kind {
                ThreadKind::Structured { .. } => true,
                ThreadKind::Terminal { run, .. } => {
                    run.as_ref().is_some_and(|l| l.resume.is_some())
                }
            });
        if resumes && !self.busy(thread) {
            self.drop_view(thread);
        }
    }

    fn remember(&mut self, thread: u64, what: &'static str, cx: &mut Context<Self>) {
        let Some((_, t)) = self.state.thread(thread) else {
            return;
        };
        let serial = self.undo.as_ref().map_or(0, |u| u.serial + 1);
        self.undo = Some(Undo {
            thread,
            what,
            settled: t.settled,
            snooze: t.snooze,
            at: Instant::now(),
            serial,
        });
        // the toast leaves by itself
        self._undo_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNDO_FOR).await;
            let _ = this.update(cx, |r, cx| {
                if r.undo.as_ref().is_some_and(|u| u.at.elapsed() >= UNDO_FOR) {
                    r.undo = None;
                    cx.notify();
                }
            });
        }));
    }

    /// Settles a thread, or brings it back.
    pub(crate) fn settle(
        &mut self,
        thread: u64,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if on {
            self.remember(thread, "Settled", cx);
        }
        if let Some(t) = self.state.thread_mut(thread) {
            t.settled = on;
            if on {
                t.snooze = None;
            } else {
                // back in the active list for a full window before it can settle again
                t.touched = now_ms();
            }
        }
        if on {
            self.free(thread);
        }
        self.leave(window, cx);
        self.save();
        cx.notify();
    }

    /// Snoozes a thread, or wakes it with `None`.
    pub(crate) fn snooze(
        &mut self,
        thread: u64,
        until: Option<Snooze>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.snooze_menu = None;
        if until.is_some() {
            self.remember(thread, "Snoozed", cx);
        }
        if let Some(t) = self.state.thread_mut(thread) {
            t.snooze = until;
            if until.is_none() {
                t.touched = now_ms();
            }
        }
        if until.is_none() {
            self.unseen.insert(thread);
        }
        self.leave(window, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(u) = self.undo.take() else {
            return;
        };
        if let Some(t) = self.state.thread_mut(u.thread) {
            t.settled = u.settled;
            t.snooze = u.snooze;
        }
        self.leave(window, cx);
        self.save();
        cx.notify();
    }

    /// A thread did something: a turn started, ended, or stopped to ask. Called with each status.
    pub(crate) fn touched(&mut self, thread: u64, status: Status) {
        let ended = matches!(status, Status::Done | Status::Waiting | Status::Failed);
        let mut changed = false;
        if let Some(t) = self.state.thread_mut(thread) {
            if matches!(status, Status::Working) || ended {
                t.touched = now_ms();
                changed = true;
            }
            // snoozed until it finishes: it has
            if ended && t.snooze == Some(Snooze::Done) {
                t.snooze = None;
            }
        }
        if ended
            && self
                .state
                .thread(thread)
                .is_some_and(|(_, t)| t.snooze.is_none() && !t.settled)
        {
            // a woken thread comes back marked new
            if self.screen != Screen::Thread(thread) {
                self.unseen.insert(thread);
            }
        }
        // a settled thread kept its session only to finish its turn; one that stopped to ask
        // still counts as busy, so its question isn't lost
        if ended
            && self
                .state
                .thread(thread)
                .is_some_and(|(_, t)| t.settled && !self.on_screen(thread))
        {
            self.free(thread);
        }
        if changed {
            self.save();
        }
    }

    /// Wakes snoozes that are due and settles threads that sat untouched long enough. The pump
    /// calls it every few seconds.
    pub(crate) fn tidy_threads(&mut self, cx: &mut Context<Self>) {
        if !self.loaded {
            return;
        }
        let now = now_ms();
        let after = self.state.settle_after;
        let mut woke = Vec::new();
        let mut settled = Vec::new();
        let busy: Vec<u64> = self
            .status
            .iter()
            .filter(|(_, s)| matches!(s, Status::Working | Status::Waiting))
            .map(|(t, _)| *t)
            .collect();
        let shown: Vec<u64> = self
            .state
            .spaces
            .iter()
            .flat_map(|s| s.grid.panes.iter())
            .filter_map(|p| match p {
                Pane::Thread { id } => Some(*id),
                _ => None,
            })
            .chain(match self.screen {
                Screen::Thread(id) => Some(id),
                _ => None,
            })
            .collect();
        for s in &mut self.state.spaces {
            for t in &mut s.threads {
                if let Some(Snooze::Time { at }) = t.snooze
                    && at <= now
                {
                    t.snooze = None;
                    t.touched = now;
                    woke.push(t.id);
                } else if t.settles(now, after) && !busy.contains(&t.id) && !shown.contains(&t.id) {
                    t.settled = true;
                    settled.push(t.id);
                }
            }
        }
        if woke.is_empty() && settled.is_empty() {
            return;
        }
        self.unseen.extend(woke);
        for t in settled {
            self.free(t);
        }
        self.save();
        cx.notify();
    }

    /// The snooze menu, after T3 Code's: each choice with when it wakes, and Until it finishes
    /// for a thread at work.
    pub(crate) fn snooze_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (at, thread) = self.snooze_menu?;
        let now = local_now();
        let mut rows: Vec<AnyElement> = presets(now)
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let wake = to_ms(p.at);
                choice(("snooze", i), p.label, wake_short(p.at, now))
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.snooze(thread, Some(Snooze::Time { at: wake }), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        if self.busy(thread) {
            rows.push(
                choice(
                    "snooze-done",
                    "Until it finishes",
                    "When the turn ends".into(),
                )
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.snooze(thread, Some(Snooze::Done), window, cx)
                }))
                .into_any_element(),
            );
        }
        let close = cx.listener(|r, _: &(), _, cx| {
            r.snooze_menu = None;
            cx.notify();
        });
        Some(widgets::popup(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            div()
                .w(px(250.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .px(px(10.))
                        .pt(px(4.))
                        .pb(px(4.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text3())
                        .child("Snooze until"),
                )
                .children(rows),
        ))
    }

    pub(crate) fn open_snooze_menu(
        &mut self,
        at: Point<Pixels>,
        thread: u64,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.snooze_menu = Some((at, thread));
        cx.notify();
    }

    /// The toast after a settle or a snooze, with Undo, for a few seconds.
    pub(crate) fn undo_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let u = self.undo.as_ref().filter(|u| u.at.elapsed() < UNDO_FOR)?;
        let title = self
            .state
            .thread(u.thread)
            .map(|(_, t)| t.title.clone())
            .unwrap_or_default();
        let toast = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pl(px(14.))
            .pr(px(6.))
            .py(px(6.))
            .rounded(px(10.))
            .bg(colors::surface3())
            .border_1()
            .border_color(colors::border2())
            .shadow(colors::shadow())
            .text_size(px(12.5))
            .child(
                div()
                    .max_w(px(320.))
                    .truncate()
                    .text_color(colors::text2())
                    .child(format!("{} \"{title}\"", u.what)),
            )
            .child(
                div()
                    .id("undo")
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)))
                    .child("Undo")
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.undo(window, cx))),
            );
        let toast = crate::slide::ease_in(toast, ("undo-toast", u.serial), 160, |d, t| {
            d.opacity(t).mb(px(8. * (1. - t)))
        });
        Some(
            div()
                .absolute()
                .bottom(px(18.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(toast)
                .into_any_element(),
        )
    }

    /// When a snoozed thread wakes, for its row.
    pub(crate) fn wake_text(&self, snooze: Snooze) -> String {
        match snooze {
            Snooze::Done => "when it finishes".into(),
            Snooze::Time { at } => wake_label(local(at), local_now()),
        }
    }
}

/// One snooze choice: its name, and when it wakes on the right.
fn choice(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    when: String,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(30.))
        .px(px(10.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.06)))
        .child(icon("clock", 13., colors::text3()))
        .child(
            div()
                .flex_1()
                .text_size(px(12.5))
                .text_color(colors::text1())
                .child(label),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(colors::text3())
                .child(when),
        )
}
