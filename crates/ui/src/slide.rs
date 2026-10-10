// Side panels slide open and shut along their width instead of popping, after T3 Code's: the
// panel keeps its own width inside a clipping frame and rides the frame's moving edge, so it
// slides in and out past the window's edge rather than being wiped. `Glide` eases anything that
// moves between spots, like a menu's highlight or a slider's knob.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, ElementId, IntoElement, div, ease_in_out, prelude::*, px,
};

const LENGTH: Duration = Duration::from_millis(180);

thread_local! {
    /// Settings, Appearance, Animations. Off, panels, menus and rows snap into place.
    static ANIMATIONS: Cell<bool> = const { Cell::new(true) };
    /// When the last panel slide ends.
    static SLIDE_END: Cell<Option<Instant>> = const { Cell::new(None) };
    static FLIPS: RefCell<Turns> = RefCell::default();
}

#[derive(Default)]
struct Turns {
    count: usize,
    at: HashMap<String, (usize, Instant)>,
}

pub fn flip(key: String) {
    FLIPS.with(|f| {
        let Turns { count, at: flips } = &mut *f.borrow_mut();
        *count += 1;
        flips.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(2));
        flips.insert(key, (*count, Instant::now()));
    });
}

pub fn flipped(key: &str, within: Duration) -> Option<usize> {
    if !animations() {
        return None;
    }
    FLIPS.with(|f| {
        f.borrow()
            .at
            .get(key)
            .filter(|(_, at)| at.elapsed() < within)
            .map(|(n, _)| *n)
    })
}

/// How much is left of a panel sliding open or shut, if one is. A terminal holds its size until
/// then and reflows once, instead of rewrapping its text on every frame of the slide.
pub fn sliding() -> Option<Duration> {
    let end = SLIDE_END.with(Cell::get)?;
    end.checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
}

pub fn set_animations(on: bool) {
    ANIMATIONS.with(|a| a.set(on));
}

pub fn animations() -> bool {
    ANIMATIONS.with(Cell::get)
}

/// `el` eased in by `f` over `ms` the first time it shows under `id`, or drawn as it ends when
/// animations are off.
pub fn ease_in<E: IntoElement + 'static>(
    el: E,
    id: impl Into<ElementId>,
    ms: u64,
    f: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if !animations() {
        return f(el, 1.).into_any_element();
    }
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(ms)).with_easing(ease_out),
        f,
    )
    .into_any_element()
}
/// A dialog's card rising 8px into place as it fades in, the first time it shows under `id`.
/// The frame spans its parent so a card sized against the window keeps its width.
pub fn rise_in(card: impl IntoElement, id: impl Into<ElementId>) -> AnyElement {
    ease_in(
        div()
            .relative()
            .w_full()
            .flex()
            .justify_center()
            .child(card),
        id,
        180,
        |d, t| d.opacity(t).top(px(8. * (1. - t))),
    )
}

/// Short enough to keep up with the arrow keys and the mouse.
const GLIDE: Duration = Duration::from_millis(140);

/// Fast at first, settling at the end.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// A panel's open state as last drawn and how often it changed. Each change plays the slide once,
/// whichever code flipped the state.
#[derive(Default)]
pub struct Flips {
    open: Option<bool>,
    count: usize,
    /// When the last change happened.
    at: Option<Instant>,
}

impl Flips {
    /// Notes the state this draw shows and returns the slide to play. The first look counts as
    /// no change, so a panel saved shut stays shut at launch instead of sliding away.
    pub fn see(&mut self, open: bool) -> usize {
        if self.open.is_some_and(|o| o != open) {
            let now = Instant::now();
            self.count += 1;
            self.at = Some(now);
            if animations() {
                SLIDE_END.with(|e| e.set(Some(now + LENGTH)));
            }
        }
        self.open = Some(open);
        self.count()
    }

    /// The change count while its slide is still going, else 0. A panel drawn again later, after
    /// Settings or another screen took its place, must not replay the last slide: that is what
    /// slid the sidebar in again and shoved the terminal right after closing Settings.
    pub fn count(&self) -> usize {
        // a few frames of slack, so the last frame of the slide still counts as part of it
        let live = self
            .at
            .is_some_and(|t| t.elapsed() < LENGTH + Duration::from_millis(50));
        if live { self.count } else { 0 }
    }
}

/// `body` in a frame that is `full` wide when open and `shut` wide when not, sliding between the
/// two after a change. `end` pins the body to the frame's right edge: a panel on the left rides
/// that edge out of the window, and one on the right, pinned to its left edge, does the same the
/// other way. Unpinned, a left panel stays put and the frame closes over it, as for buttons that
/// stay on screen.
pub fn slide(
    id: &'static str,
    flips: usize,
    open: bool,
    (shut, full): (f32, f32),
    end: bool,
    body: impl IntoElement,
) -> AnyElement {
    let flips = if animations() { flips } else { 0 };
    let frame = div()
        .flex_none()
        .h_full()
        .flex()
        .overflow_hidden()
        .when(end, |d| d.justify_end())
        .child(body);
    if flips == 0 {
        return frame
            .w(px(if open { full } else { shut }))
            .into_any_element();
    }
    let (from, to) = if open { (shut, full) } else { (full, shut) };
    frame
        .with_animation(
            (id, flips),
            Animation::new(LENGTH).with_easing(ease_in_out),
            move |d, t| d.w(px(from + (to - from) * t)),
        )
        .into_any_element()
}

/// A value that eases to each new target instead of jumping there. A new target mid-move starts
/// from wherever the value is, so a quick run of moves stays smooth.
#[derive(Default)]
pub struct Glide {
    /// Where the current move started, where it ends, and when it started.
    state: Cell<Option<(f32, f32, Instant)>>,
    moves: Cell<usize>,
}

/// One frame's look at a `Glide`.
#[derive(Clone, Copy)]
pub struct Motion {
    from: f32,
    to: f32,
    moves: usize,
}

impl Glide {
    /// The move toward `to`. The first target is where the value starts, with no move.
    pub fn toward(&self, to: f32) -> Motion {
        let now = Instant::now();
        let from = match self.state.get() {
            None => {
                self.state.set(Some((to, to, now)));
                to
            }
            Some((from, at, start)) if at != to => {
                let t = (now - start).as_secs_f32() / GLIDE.as_secs_f32();
                let here = from + (at - from) * ease_out(t.min(1.));
                self.moves.set(self.moves.get() + 1);
                self.state.set(Some((here, to, now)));
                here
            }
            Some((from, _, _)) => from,
        };
        Motion {
            from,
            to,
            moves: self.moves.get(),
        }
    }
}

impl Motion {
    /// `el` drawn by `f` at the value as it moves. Each move replays under a fresh id.
    pub fn apply<E: IntoElement + 'static>(
        self,
        id: &'static str,
        el: E,
        f: impl Fn(E, f32) -> E + 'static,
    ) -> AnyElement {
        let Motion { from, to, moves } = self;
        if moves == 0 || !animations() {
            return f(el, to).into_any_element();
        }
        el.with_animation(
            ElementId::from((id, moves)),
            Animation::new(GLIDE).with_easing(ease_out),
            move |el, t| f(el, from + (to - from) * t),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glide_starts_still_and_counts_each_new_target() {
        let g = Glide::default();
        let m = g.toward(10.);
        assert_eq!((m.from, m.to, m.moves), (10., 10., 0));
        assert_eq!(g.toward(10.).moves, 0);
        let m = g.toward(40.);
        assert_eq!((m.from, m.to, m.moves), (10., 40., 1));
        // asking again for the same target is the same move
        assert_eq!(g.toward(40.).moves, 1);
    }

    #[test]
    fn only_a_change_slides_and_only_while_it_is_new() {
        let mut f = Flips::default();
        assert_eq!(f.see(false), 0);
        assert_eq!(f.see(false), 0);
        assert_eq!(f.see(true), 1);
        assert_eq!(f.see(true), 1);
        assert_eq!(f.see(false), 2);
        // long after, drawing it again plays nothing
        f.at = Some(Instant::now() - LENGTH * 3);
        assert_eq!(f.see(false), 0);
        assert_eq!(f.count(), 0);
    }
}
