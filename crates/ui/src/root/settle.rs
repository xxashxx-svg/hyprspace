// Settling and snoozing threads, after T3 Code's. Settled threads leave their space for the
// Settled shelf at the bottom of the sidebar; untouched ones settle by themselves after a while. A snoozed thread hides
// until a time, or until its agent finishes, and comes back marked new. Either can be undone for
// a few seconds from the toast.
//
// A settled thread known to be idle gives up its terminal and agent process; the conversation
// resumes when it is opened again. That is what keeps dozens of threads cheap. A thread at work
// keeps going, and a snoozed one always keeps its session. A plain shell has nothing to resume,
// and Codex in a terminal never says whether it's working, so both keep their terminal.

use std::time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window,
    div, prelude::*, px,
};
use hyprspace_proto::{Agent, Snooze, Thread, ThreadKind};

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
    /// At work: in a turn, waiting on you, or with subagents still running after its turn ended.
    pub(crate) fn busy(&self, thread: u64) -> bool {
        matches!(
            self.status.get(&thread),
            Some(Status::Working | Status::Waiting)
        ) || self
            .activity
            .get(&thread)
            .is_some_and(|a| !a.subs.is_empty())
    }

    /// On screen, which auto-settle leaves alone.
    fn on_screen(&self, thread: u64) -> bool {
        self.screen == Screen::Thread(thread)
    }

    /// Gives up a thread's session when it is known to be idle and its conversation can resume.
    /// Only a structured thread and Claude in a terminal (through its hooks) say whether they're
    /// working, so anything else keeps running.
    pub(crate) fn free(&mut self, thread: u64) {
        let knowable = self
            .state
            .thread(thread)
            .is_some_and(|(_, t)| match &t.kind {
                ThreadKind::Structured { .. } => true,
                ThreadKind::Terminal { run, .. } => run
                    .as_ref()
                    .is_some_and(|l| l.agent == Agent::Claude && l.resume.is_some()),
            });
        if knowable && !self.busy(thread) {
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
            self.unpeek(thread);
        }
        if let Some(t) = self.state.thread_mut(thread) {
            t.settled = on;
            if on {
                t.snooze = None;
                t.pinned = None;
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

    pub(crate) fn toggle_pin(&mut self, thread: u64, cx: &mut Context<Self>) {
        let pinned = self
            .state
            .thread(thread)
            .is_some_and(|(_, t)| t.pinned.is_some());
        self.pin(thread, !pinned, cx);
    }

    pub(crate) fn pin(&mut self, thread: u64, on: bool, cx: &mut Context<Self>) {
        if let Some(t) = self.state.thread_mut(thread) {
            t.pinned = on.then(now_ms);
        }
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
            self.unpeek(thread);
        }
        if let Some(t) = self.state.thread_mut(thread) {
            match until {
                Some(_) => t.snooze = until,
                None => t.wake(now_ms()),
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

    /// Settling or snoozing the shelved thread on screen sends it away like any other.
    fn unpeek(&mut self, thread: u64) {
        if self.peek == Some(thread) {
            self.peek = None;
        }
    }

    /// A settled or snoozed thread that starts a turn, because a message was sent to it, comes
    /// back to the top of the list.
    pub(crate) fn bring_back(&mut self, thread: u64) {
        if let Some(t) = self.state.thread_mut(thread)
            && !t.active()
        {
            t.settled = false;
            t.wake(now_ms());
            self.unpeek(thread);
            self.reveal = Some(thread);
            self.save();
        }
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
                t.wake(now_ms());
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

    /// A thread's session ended, its agent quit or its terminal closed: a snooze waiting for its
    /// turn to finish will never see that, so it wakes now.
    pub(crate) fn session_ended(&mut self, thread: u64) {
        if let Some(t) = self.state.thread_mut(thread)
            && t.snooze == Some(Snooze::Done)
        {
            t.wake(now_ms());
            self.unseen.insert(thread);
            self.save();
        }
    }

    fn resume_due(&mut self, now: u64, cx: &mut Context<Self>) {
        let due: Vec<Thread> = self
            .state
            .spaces
            .iter()
            .flat_map(|s| &s.threads)
            .filter(|t| t.resume_at.is_some_and(|at| at <= now))
            .cloned()
            .collect();
        for t in due {
            if !self.views.contains_key(&t.id) {
                self.make_view(&t, None, true, cx);
            }
            if let Some(super::View::Structured(v)) = self.views.get(&t.id) {
                v.update(cx, |v, cx| v.continue_now(cx));
            } else if let Some(t) = self.state.thread_mut(t.id) {
                t.resume_at = None;
            }
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
            .keys()
            .chain(self.activity.keys())
            .copied()
            .filter(|t| self.busy(*t))
            .collect();
        let shown = match self.screen {
            Screen::Thread(id) => Some(id),
            _ => None,
        };
        // a settled thread opened for a look gives its session back once it is off screen; a
        // snoozed one keeps it
        let looked: Vec<u64> = self
            .views
            .keys()
            .copied()
            .filter(|id| {
                !self.on_screen(*id) && self.state.thread(*id).is_some_and(|(_, t)| t.settled)
            })
            .collect();
        for id in looked {
            self.free(id);
        }
        self.resume_due(now, cx);
        woke.extend(self.state.wake_due(now, false));
        for s in &mut self.state.spaces {
            for t in &mut s.threads {
                if t.settles(now, after) && !busy.contains(&t.id) && shown != Some(t.id) {
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
        let (title, snooze) = self
            .state
            .thread(u.thread)
            .map(|(_, t)| (t.title.clone(), t.snooze))
            .unwrap_or_default();
        let (glyph, what) = match snooze.filter(|_| u.what == "Snoozed") {
            Some(s) => ("clock", format!("Snoozed until {}", self.wake_text(s))),
            None => ("archive", u.what.to_string()),
        };
        let tint = colors::accent();
        let bar = div()
            .absolute()
            .left_0()
            .bottom_0()
            .h(px(2.))
            .bg(tint.opacity(0.7))
            .with_animation(
                ("undo-bar", u.serial),
                gpui::Animation::new(UNDO_FOR),
                move |d, t| d.w(gpui::relative(1. - t)),
            );
        let toast = div()
            .id("undo-toast")
            .occlude()
            .relative()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(10.))
            .min_w(px(300.))
            .max_w(px(460.))
            .pl(px(10.))
            .pr(px(6.))
            .py(px(8.))
            .rounded(px(12.))
            .bg(colors::surface2())
            .border_1()
            .border_color(colors::border2())
            .shadow(colors::shadow())
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded_full()
                    .bg(tint.opacity(0.14))
                    .child(icon(glyph, 13., tint)),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_baseline()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text1())
                            .child(what),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(colors::text3())
                            .child(title),
                    ),
            )
            .child(
                div()
                    .id("undo")
                    .flex_none()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(7.))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(tint)
                    .cursor_pointer()
                    .hover(|s| s.bg(tint.opacity(0.12)))
                    .child("Undo")
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.undo(window, cx))),
            )
            .child(
                widgets::icon_button("undo-close", "x", 24.).on_click(cx.listener(
                    |r, _: &ClickEvent, _, cx| {
                        r.undo = None;
                        cx.notify();
                    },
                )),
            )
            .child(bar);
        let toast = crate::slide::ease_in(toast, ("undo-toast", u.serial), 180, |d, t| {
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
