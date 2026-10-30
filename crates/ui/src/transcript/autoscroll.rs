// Scrolling with the middle mouse button, as browsers on Windows do: a press drops a marker that
// follows the pointer, and the further it goes above or below the press the faster the transcript
// moves. A click anywhere or Esc ends it, and so does letting go of the wheel after dragging with it
// held. The pointer hides while it runs, so the marker takes its place.

use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, Pixels, Point, Task, Window, div, point, prelude::*};

use super::TranscriptView;
use crate::assets::icon;
use crate::colors;
use crate::zoom::px;

/// Around the marker the pointer can rest without anything moving.
const STILL: f32 = 12.;
/// Pixels a second for each pixel the pointer is past that, plus more the further it goes.
const PACE: f32 = 14.;
const GROW: f32 = 0.06;
const SIZE: f32 = 30.;
/// A view that stops drawing for this long, gone behind another thread or a minimized window,
/// ends it, so the pointer never stays hidden.
const STALE: Duration = Duration::from_millis(250);

pub(super) struct Autoscroll {
    origin: Point<Pixels>,
    dragged: bool,
    last: Instant,
    _hidden: Hidden,
    _watch: Task<()>,
}

impl Autoscroll {
    pub(super) fn new(at: Point<Pixels>, cx: &mut Context<TranscriptView>) -> Self {
        let watch = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(STALE).await;
                let ended = this.update(cx, |v, cx| {
                    let stale = v
                        .autoscroll
                        .as_ref()
                        .is_none_or(|a| a.last.elapsed() > STALE);
                    if stale {
                        v.autoscroll = None;
                        cx.notify();
                    }
                    stale
                });
                if ended.unwrap_or(true) {
                    break;
                }
            }
        });
        Self {
            origin: at,
            dragged: false,
            last: Instant::now(),
            _hidden: Hidden::new(),
            _watch: watch,
        }
    }

    pub(super) fn dragged(&self) -> bool {
        self.dragged
    }
}

/// One frame of it: the transcript moves by how long the frame took at the pointer's speed.
pub(super) fn step(v: &mut TranscriptView, window: &mut Window) {
    if !window.is_window_active() {
        v.autoscroll = None;
    }
    let Some(a) = v.autoscroll.as_mut() else {
        return;
    };
    let now = Instant::now();
    let dt = (now - a.last).as_secs_f32().min(0.05);
    a.last = now;
    // the window's own pointer, since move listeners miss it over the rows that block the mouse
    let off = f32::from(window.mouse_position().y - a.origin.y);
    if off.abs() > STILL {
        a.dragged = true;
    }
    let past = (off.abs() - STILL).max(0.);
    let speed = (past * PACE + past * past * GROW) * off.signum();
    if speed != 0. {
        v.follow.drift(&v.list, gpui::px(speed * dt));
    }
    // every refresh, not the 60 Hz pace, or the marker trails the pointer
    window.request_animation_frame();
}

/// The marker under the pointer, with the ways it can go.
pub(super) fn marker(v: &TranscriptView, window: &Window) -> Option<AnyElement> {
    v.autoscroll.as_ref()?;
    let at = window.mouse_position();
    let half = gpui::px(SIZE / 2.);
    Some(
        gpui::deferred(
            gpui::anchored()
                .position(point(at.x - half, at.y - half))
                .child(
                    div()
                        .size(px(SIZE))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(colors::border2())
                        .bg(colors::surface2())
                        .shadow(colors::shadow())
                        .child(icon("autoscroll", SIZE * 0.62, colors::text1())),
                ),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

/// The OS pointer, hidden for as long as this lives. GPUI only hides it until the mouse moves.
struct Hidden;

impl Hidden {
    fn new() -> Self {
        pointer(false);
        Self
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        pointer(true);
    }
}

#[cfg(windows)]
fn pointer(show: bool) {
    unsafe extern "system" {
        fn ShowCursor(show: i32) -> i32;
    }
    // SAFETY: a flag in, the display count out; each hide here has one show from the drop
    unsafe { ShowCursor(show as i32) };
}

#[cfg(target_os = "macos")]
fn pointer(show: bool) {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayHideCursor(display: u32) -> i32;
        fn CGDisplayShowCursor(display: u32) -> i32;
    }
    // SAFETY: 0 is the null display, which means every display; hides and shows pair up
    unsafe {
        if show {
            CGDisplayShowCursor(0);
        } else {
            CGDisplayHideCursor(0);
        }
    }
}
