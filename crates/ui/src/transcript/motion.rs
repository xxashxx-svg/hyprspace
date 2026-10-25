use std::time::{Duration, Instant};

use gpui::{App, ListOffset, ListState, Pixels, Window};

use crate::slide::{animations, ease_out};
use crate::zoom::px;

enum Aim {
    Bottom,
    Item(usize),
}

struct Trip {
    from: Pixels,
    aim: Aim,
    start: Instant,
    length: Duration,
}

pub struct Follow {
    pub stick: bool,
    /// How fast the view is gliding to the end, in px a second.
    velocity: f32,
    trip: Option<Trip>,
    last: Option<Instant>,
    wheel: Pixels,
    wheeled: Option<Instant>,
    /// Until then the view keeps to the end without gliding, while a thread just put on screen
    /// measures its rows.
    calm: Option<Instant>,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            stick: true,
            velocity: 0.,
            trip: None,
            last: None,
            wheel: px(0.),
            wheeled: None,
            calm: None,
        }
    }
}

const EASE: f32 = 0.06;
/// The glide to the end: its spring, in radians a second, settles in about a quarter second, and
/// it never moves faster than this many px a second.
const SPRING: f32 = 18.;
const GLIDE_MAX: f32 = 1400.;
const SETTLE: Duration = Duration::from_millis(300);

fn y(list: &ListState) -> Pixels {
    -list.scroll_px_offset_for_scrollbar().y
}

fn max(list: &ListState) -> Pixels {
    list.max_offset_for_scrollbar().y
}

fn set(list: &ListState, to: Pixels) {
    list.scroll_by(to - y(list));
}

fn target(list: &ListState, aim: &Aim) -> Pixels {
    match aim {
        Aim::Bottom => max(list),
        Aim::Item(ix) => {
            // bounds_for_item only knows rows on screen, so read the row's offset by scrolling to it
            let at = list.logical_scroll_top();
            list.scroll_to(ListOffset {
                item_ix: *ix,
                offset_in_item: px(0.),
            });
            let to = y(list);
            list.scroll_to(at);
            to.clamp(px(0.), max(list))
        }
    }
}

impl Follow {
    pub fn down(&mut self, list: &ListState) {
        self.go(list, Aim::Bottom);
    }

    pub fn seek(&mut self, list: &ListState, ix: usize) {
        self.stick = false;
        self.go(list, Aim::Item(ix));
    }

    /// The thread was just put on screen, or its rows laid out anew.
    pub fn settle(&mut self) {
        self.velocity = 0.;
        self.calm = Some(Instant::now() + SETTLE);
    }

    pub fn snap(&mut self, list: &ListState) {
        self.trip = None;
        self.velocity = 0.;
        self.wheel = px(0.);
        self.stick = true;
        list.scroll_to_end();
    }

    pub fn wheel(&mut self, up: bool) {
        self.trip = None;
        self.velocity = 0.;
        if up {
            self.stick = false;
        }
    }

    /// A turn of the mouse wheel, eased out over the next frames. Positive scrolls down.
    pub fn spin(&mut self, by: Pixels) {
        self.trip = None;
        self.velocity = 0.;
        if by < px(0.) {
            self.stick = false;
        }
        self.wheel += by;
    }

    pub fn ease(&mut self, list: &ListState, window: &mut Window) {
        if self.wheel == px(0.) {
            self.wheeled = None;
            return;
        }
        let now = Instant::now();
        let dt = self
            .wheeled
            .replace(now)
            .map_or(1. / 120., |t| (now - t).as_secs_f32().min(0.05));
        let left = self.wheel;
        let step = if !animations() || left.abs() < px(0.5) {
            left
        } else {
            left * (1. - (-dt / EASE).exp())
        };
        let end = max(list);
        let to = (y(list) + step).clamp(px(0.), end);
        set(list, to);
        // an end reached drops what is left, so the next turn answers at once
        self.wheel = if to == px(0.) || to == end {
            px(0.)
        } else {
            left - step
        };
        window.request_animation_frame();
    }

    fn go(&mut self, list: &ListState, aim: Aim) {
        self.wheel = px(0.);
        if !animations() {
            self.trip = None;
            match aim {
                Aim::Bottom => self.snap(list),
                Aim::Item(ix) => list.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px(0.),
                }),
            }
            return;
        }
        let (from, to) = (y(list), target(list, &aim));
        let screen = list.viewport_bounds().size.height.max(px(1.));
        let length =
            Duration::from_millis((260. + 80. * ((to - from).abs() / screen).min(2.)) as u64);
        // a trip of many screens starts a screen and a half out, so it doesn't blur past them
        let reach = screen * 1.5;
        let from = if (to - from).abs() > reach {
            if to > from { to - reach } else { to + reach }
        } else {
            from
        };
        self.trip = Some(Trip {
            from,
            aim,
            start: Instant::now(),
            length,
        });
    }

    /// Glides to what arrives only while the agent works; anything else keeps to the end at once.
    pub fn step(&mut self, list: &ListState, live: bool, window: &mut Window, cx: &mut App) {
        let now = Instant::now();
        let dt = self
            .last
            .map_or(0.016, |l| (now - l).as_secs_f32())
            .min(0.05);
        self.last = Some(now);
        if let Some(trip) = &self.trip {
            let to = target(list, &trip.aim);
            let t = (now - trip.start).as_secs_f32() / trip.length.as_secs_f32();
            if t >= 1. {
                match trip.aim {
                    Aim::Bottom => self.snap(list),
                    // by the row, not the pixel: rows not drawn yet can count as no height, and a
                    // pixel lands past them
                    Aim::Item(ix) if to < max(list) => list.scroll_to(ListOffset {
                        item_ix: ix,
                        offset_in_item: px(0.),
                    }),
                    Aim::Item(_) => set(list, to),
                }
                self.trip = None;
            } else {
                set(list, trip.from + (to - trip.from) * ease_out(t));
            }
            window.request_animation_frame();
            return;
        }
        let (y, max) = (y(list), max(list));
        if !self.stick {
            if max > px(0.) && y >= max - px(1.) {
                self.stick = true;
            }
            return;
        }
        let gap = f32::from(max - y);
        if gap <= 0.5 {
            self.velocity = 0.;
            return;
        }
        let calm = self.calm.is_some_and(|t| now < t);
        if !animations()
            || !live
            || calm
            || gap > f32::from(list.viewport_bounds().size.height) * 2.
        {
            self.velocity = 0.;
            list.scroll_to_end();
            crate::pace::next(window, cx);
            return;
        }
        // a spring that settles without overshoot carries the view to the end, so what arrives
        // glides up the whole way; the cap keeps a tall arrival from racing
        let accel = SPRING * SPRING * gap - 2. * SPRING * self.velocity;
        self.velocity = (self.velocity + accel * dt).clamp(0., GLIDE_MAX);
        let step = (self.velocity * dt).clamp(0.5, gap);
        set(list, y + px(step));
        crate::pace::next(window, cx);
    }

    pub fn behind(&self, list: &ListState) -> bool {
        self.stick && self.trip.is_none() && max(list) - y(list) > px(0.5)
    }

    pub fn away(&self, list: &ListState) -> bool {
        !self.stick && self.trip.is_none() && y(list) < max(list) - px(48.)
    }
}

pub struct Reveal {
    pub ix: usize,
    shown: f32,
    last: Instant,
}

impl Reveal {
    pub fn new(ix: usize, from: usize) -> Self {
        Self {
            ix,
            shown: from as f32,
            last: Instant::now(),
        }
    }

    pub fn behind(&self, len: usize) -> bool {
        animations() && self.shown < len as f32
    }

    pub fn step(&mut self, text: &str, window: &mut Window, cx: &mut App) -> Option<usize> {
        if !animations() {
            return None;
        }
        let left = text.len() as f32 - self.shown;
        if left <= 0. {
            return None;
        }
        let now = Instant::now();
        // only on the beat, which can land up to half a refresh early; a redraw between beats,
        // for a glide, keeps the text as it was
        if now - self.last >= crate::pace::BEAT * 3 / 4 {
            let dt = (now - self.last).as_secs_f32().min(0.1);
            self.last = now;
            // text that piles up runs out faster, so the reveal never trails the stream for long
            let speed = (left / 0.2).max(120.);
            self.shown = (self.shown + speed * dt).min(text.len() as f32);
        }
        let mut end = self.shown as usize;
        while !text.is_char_boundary(end) {
            end += 1;
        }
        crate::pace::beat(window, cx);
        Some(end)
    }
}
