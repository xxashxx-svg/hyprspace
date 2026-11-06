// Settling and snoozing threads, after T3 Code's. Settled threads leave their space for the
// Settled shelf at the bottom of the sidebar; untouched ones settle by themselves after a while. A snoozed thread hides
// until a time, or until its agent finishes, and comes back marked new. Either can be undone for
// a few seconds from the line in the sidebar's foot.
//
// A settled thread known to be idle gives up its terminal and agent process; the conversation
// resumes when it is opened again. That is what keeps dozens of threads cheap. A thread at work
// keeps going, and a snoozed one always keeps its session. A plain shell has nothing to resume,
// and Codex in a terminal never says whether it's working, so both keep their terminal.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    prelude::*,
};
use hyprspace_proto::phone::Ask;
use hyprspace_proto::{Agent, Draft, Snooze, Thread, ThreadKind};

use super::{Root, Screen};
use crate::assets::icon;
use crate::colors;
use crate::time::{local, local_now, now_ms, presets, to_ms, wake_label, wake_short};
use crate::transcript::Status;
use crate::widgets;
use crate::zoom::px;

/// How long the toast offers Undo.
const UNDO_FOR: Duration = Duration::from_secs(5);
const FADE: Duration = Duration::from_millis(200);

/// What the toast can put back.
#[derive(Clone)]
pub(crate) struct Undo {
    threads: Vec<(u64, bool, Option<Snooze>)>,
    what: &'static str,
    draft: Option<Draft>,
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

    pub(crate) fn working(&self) -> usize {
        let threads: std::collections::HashSet<u64> = self
            .status
            .keys()
            .chain(self.activity.keys())
            .copied()
            .filter(|&t| t < crate::machines::FIRST && self.busy(t))
            .collect();
        threads.len()
    }

    /// On screen, which auto-settle leaves alone.
    pub(super) fn on_screen(&self, thread: u64) -> bool {
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

    fn remember(&mut self, threads: &[u64], what: &'static str, cx: &mut Context<Self>) {
        let threads: Vec<_> = threads
            .iter()
            .filter_map(|&id| {
                self.state
                    .thread(id)
                    .map(|(_, t)| (id, t.settled, t.snooze))
            })
            .collect();
        if threads.is_empty() {
            return;
        }
        let undo = Undo {
            threads,
            what,
            draft: None,
            at: Instant::now(),
            serial: 0,
        };
        self.offer_undo(undo, cx);
    }

    pub(crate) fn remember_draft(&mut self, draft: Draft, cx: &mut Context<Self>) {
        let undo = Undo {
            threads: Vec::new(),
            what: "Discarded",
            draft: Some(draft),
            at: Instant::now(),
            serial: 0,
        };
        self.offer_undo(undo, cx);
    }

    fn offer_undo(&mut self, mut undo: Undo, cx: &mut Context<Self>) {
        undo.serial = self.undo.as_ref().map_or(0, |u| u.serial + 1);
        self.undo = Some(undo);
        // the toast leaves by itself
        self._undo_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNDO_FOR - FADE).await;
            let _ = this.update(cx, |_, cx| cx.notify());
            cx.background_executor().timer(FADE).await;
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
        self.settle_many(&[thread], on, window, cx);
    }

    pub(crate) fn settle_many(
        &mut self,
        threads: &[u64],
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = if on {
            self.next_after(threads, cx)
        } else {
            None
        };
        if on {
            self.remember(threads, "Settled", cx);
        }
        for &thread in threads {
            self.ask_host(thread, |thread| Ask::Settle { thread, on });
            if on {
                self.unpeek(thread);
            }
            if let Some(t) = self.state.thread_mut(thread) {
                t.set_settled(on, now_ms());
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
        }
        self.move_on(next, window, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn pin(&mut self, thread: u64, on: bool, cx: &mut Context<Self>) {
        self.ask_host(thread, |thread| Ask::Pin { thread, on });
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
        self.snooze_many(&[thread], until, window, cx);
    }

    pub(crate) fn snooze_many(
        &mut self,
        threads: &[u64],
        until: Option<Snooze>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.snooze_menu = None;
        self.snooze_custom = None;
        let next = if until.is_some() {
            self.next_after(threads, cx)
        } else {
            None
        };
        if until.is_some() {
            self.remember(threads, "Snoozed", cx);
        }
        for &thread in threads {
            self.ask_host(thread, |thread| match until {
                Some(until) => Ask::Snooze { thread, until },
                None => Ask::Settle { thread, on: false },
            });
            if until.is_some() {
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
        }
        self.move_on(next, window, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.undo
            .as_ref()
            .is_some_and(|u| u.at.elapsed() < UNDO_FOR)
    }

    fn next_after(&self, threads: &[u64], cx: &mut Context<Self>) -> Option<u64> {
        let Screen::Thread(on) = self.screen else {
            return None;
        };
        if !threads.contains(&on) {
            return None;
        }
        let list: Vec<u64> = self
            .jump_list(cx)
            .into_iter()
            .filter(|&t| t == on || self.state.thread(t).is_some_and(|(_, t)| t.active()))
            .collect();
        let at = list.iter().position(|&t| t == on)?;
        let free = |t: &&u64| !threads.contains(t);
        list[at + 1..]
            .iter()
            .find(free)
            .or_else(|| list[..at].iter().rev().find(free))
            .copied()
    }

    fn move_on(&mut self, next: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        match next {
            Some(next) => self.open_thread(next, window, cx),
            None => self.leave(window, cx),
        }
    }

    pub(crate) fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(u) = self.undo.take() else {
            return;
        };
        if let Some(draft) = u.draft {
            self.state.drafts.push(draft);
            self.save();
            cx.notify();
            return;
        }
        for (thread, settled, snooze) in u.threads {
            self.ask_host(thread, |thread| match snooze {
                Some(until) => Ask::Snooze { thread, until },
                None => Ask::Settle {
                    thread,
                    on: settled,
                },
            });
            if let Some(t) = self.state.thread_mut(thread) {
                t.set_settled(settled, now_ms());
                t.snooze = snooze;
            }
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
            t.set_settled(false, now_ms());
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

    /// An update started installing: every thread mid-run is marked, with its background work, so
    /// the new version can pick it back up. A failed install leaves the version as it was, and
    /// marks on an unchanged version are cleared at the next launch without acting.
    pub(crate) fn mark_updating(&mut self, cx: &mut Context<Self>) {
        let marks: Vec<(u64, Option<Vec<String>>)> = self
            .views
            .iter()
            .filter_map(|(id, view)| match view {
                super::View::Structured(v) => {
                    let v = v.read(cx);
                    Some((*id, v.elapsed().map(|_| v.background_work())))
                }
                _ => None,
            })
            .collect();
        for (id, mark) in marks {
            if let Some(t) = self.state.thread_mut(id) {
                t.updating = mark;
            }
        }
        self.save();
    }

    /// The first launch after an update picks up the runs it cut off. A dev or test copy loads
    /// copies of real threads, and picking those up would start real agents on them.
    pub(crate) fn pick_up_updates(&mut self, updated: bool, cx: &mut Context<Self>) {
        let copy = std::env::var_os("HYPRSPACE_DEV").is_some()
            || std::env::var_os("HYPRSPACE_STATE_DIR").is_some();
        let marked: Vec<(Thread, Vec<String>)> = self
            .state
            .spaces
            .iter_mut()
            .flat_map(|s| &mut s.threads)
            .filter_map(|t| t.updating.take().map(|work| (t.clone(), work)))
            .collect();
        if marked.is_empty() {
            return;
        }
        self.save();
        if !updated || copy {
            return;
        }
        for (t, work) in marked {
            if !self.views.contains_key(&t.id) {
                self.make_view(&t, None, true, cx);
            }
            if let Some(super::View::Structured(v)) = self.views.get(&t.id) {
                v.update(cx, |v, cx| v.pick_up_after_update(work, cx));
            }
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
        self.clean_worktrees(now);
        self.run_scheduled(chrono::Local::now(), cx);
        woke.extend(self.state.wake_due(now, false));
        for s in self.state.spaces.iter_mut().filter(|s| s.machine.is_none()) {
            for t in &mut s.threads {
                if t.settles(now, after) && !busy.contains(&t.id) && shown != Some(t.id) {
                    t.set_settled(true, now);
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
        rows.push(
            choice("snooze-custom", "Custom...", String::new())
                .on_click(
                    cx.listener(|r, _: &ClickEvent, window, cx| r.open_custom_snooze(window, cx)),
                )
                .into_any_element(),
        );
        let close = cx.listener(|r, _: &(), _, cx| {
            r.snooze_menu = None;
            r.snooze_custom = None;
            cx.notify();
        });
        if let Some(body) = self.custom_snooze_body(cx) {
            return Some(widgets::popup(
                at,
                widgets::Open::Down,
                window,
                move |w, cx| close(&(), w, cx),
                body,
            ));
        }
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

    /// The line in the sidebar's foot after a settle, a snooze or a discard, for a few seconds.
    pub(crate) fn undo_note(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let u = self.undo.as_ref()?;
        let left = UNDO_FOR.saturating_sub(u.at.elapsed());
        if left.is_zero() {
            return None;
        }
        let what = match u.threads.len() {
            n if n > 1 => format!("{} {n},", u.what),
            _ => format!("{},", u.what),
        };
        let key = if cfg!(target_os = "macos") {
            "Cmd+Z"
        } else {
            "Ctrl+Z"
        };
        let note = div()
            .flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .justify_end()
            .items_center()
            .gap(px(4.))
            .px(px(6.))
            .text_size(px(11.5))
            .text_color(colors::text3())
            .child(div().flex_none().child(what))
            .child(
                div()
                    .id("undo")
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors::text1()))
                    .child(format!("{key} to undo"))
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.undo(window, cx))),
            );
        let fade = FADE.as_millis() as u64;
        Some(if left <= FADE {
            crate::slide::ease_in(note, ("undo-out", u.serial), fade, |d, t| d.opacity(1. - t))
        } else {
            crate::slide::ease_in(note, ("undo-in", u.serial), fade, |d, t| d.opacity(t))
        })
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
