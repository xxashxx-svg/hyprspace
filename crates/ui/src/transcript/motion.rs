use std::time::{Duration, Instant};

use gpui::{App, ListOffset, ListState, Pixels, Window, px};

use crate::slide::{animations, ease_out};

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
    trip: Option<Trip>,
    last: Option<Instant>,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            stick: true,
            trip: None,
            last: None,
        }
    }
}

fn y(list: &ListState) -> Pixels {
    -list.scroll_px_offset_for_scrollbar().y
}

fn max(list: &ListState) -> Pixels {
    list.max_offset_for_scrollbar().y
}

fn set(list: &ListState, to: Pixels) {
    list.scroll_by(to - y(list));
}

fn target(list: &ListState, aim: &Aim) -> Option<Pixels> {
    match aim {
        Aim::Bottom => Some(max(list)),
        Aim::Item(ix) => list
            .bounds_for_item(*ix)
            .map(|b| (y(list) + b.top() - list.viewport_bounds().top()).clamp(px(0.), max(list))),
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

    pub fn snap(&mut self, list: &ListState) {
        self.trip = None;
        self.stick = true;
        list.scroll_to_end();
    }

    pub fn wheel(&mut self, up: bool) {
        self.trip = None;
        if up {
            self.stick = false;
        }
    }

    fn go(&mut self, list: &ListState, aim: Aim) {
        let to = target(list, &aim);
        let (true, Some(to)) = (animations(), to) else {
            self.trip = None;
            match aim {
                Aim::Bottom => self.snap(list),
                Aim::Item(ix) => list.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px(0.),
                }),
            }
            return;
        };
        let from = y(list);
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

    pub fn step(&mut self, list: &ListState, window: &mut Window, cx: &mut App) {
        let now = Instant::now();
        let dt = self
            .last
            .map_or(0.016, |l| (now - l).as_secs_f32())
            .min(0.05);
        self.last = Some(now);
        if let Some(trip) = &self.trip {
            let Some(to) = target(list, &trip.aim) else {
                self.trip = None;
                return;
            };
            let t = (now - trip.start).as_secs_f32() / trip.length.as_secs_f32();
            if t >= 1. {
                if matches!(trip.aim, Aim::Bottom) {
                    self.snap(list);
                } else {
                    set(list, to);
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
        let gap = max - y;
        if gap <= px(0.5) {
            return;
        }
        if !animations() || gap > list.viewport_bounds().size.height * 2. {
            list.scroll_to_end();
        } else {
            let k = 1. - (-dt / 0.06).exp();
            set(
                list,
                (y + (gap * k).max(px(0.5))).max(max - px(24.)).min(max),
            );
        }
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

    pub fn step(&mut self, text: &str, window: &mut Window, cx: &mut App) -> Option<usize> {
        if !animations() {
            return None;
        }
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.05);
        self.last = now;
        let left = text.len() as f32 - self.shown;
        if left <= 0. {
            return None;
        }
        // text that piles up runs out faster, so the reveal never trails the stream for long
        let speed = (left / 0.2).max(120.);
        self.shown = (self.shown + speed * dt).min(text.len() as f32);
        let mut end = self.shown as usize;
        while !text.is_char_boundary(end) {
            end += 1;
        }
        crate::pace::next(window, cx);
        Some(end)
    }
}
