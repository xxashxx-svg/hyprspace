use std::cell::RefCell;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, EntityId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

// A loop paced by the display redraws the whole window up to 180 times a second, so animations
// redraw at most this often. The timing comes from the display's own refreshes, so each redraw
// lands on one.
const TICK: Duration = Duration::from_micros(16_667);
/// Streamed text's beat: it reveals and fades in steps the eye reads as smooth, with half the
/// redraws.
pub const BEAT: Duration = Duration::from_micros(33_333);

thread_local! {
    static EPOCH: Instant = Instant::now();
    static DUE: RefCell<Due> = RefCell::new(Due::default());
}

/// The views waiting for a redraw: on the next tick, or on the next beat, which comes every
/// other tick so the two never add up to more redraws than ticks.
#[derive(Default)]
struct Due {
    tick: HashSet<EntityId>,
    beat: HashSet<EntityId>,
    /// Watching the display's refreshes for when the next redraw is due.
    armed: bool,
    /// The last redraw, the last one that took the beat's views, and the last refresh seen.
    ticked: Option<Instant>,
    beaten: Option<Instant>,
    seen: Option<Instant>,
}

pub trait Looping: IntoElement + Sized + 'static {
    fn looping(self, ms: u64, f: impl Fn(Self, f32) -> Self + 'static) -> Loop<Self> {
        Loop {
            element: Some(self),
            period: Duration::from_millis(ms),
            f: Box::new(f),
        }
    }
}

impl<E: IntoElement + 'static> Looping for E {}

pub struct Loop<E> {
    element: Option<E>,
    period: Duration,
    f: Box<dyn Fn(E, f32) -> E>,
}

fn phase(period: Duration) -> f32 {
    let t = EPOCH.with(|e| e.elapsed()).as_secs_f32();
    (t / period.as_secs_f32()).fract()
}

pub fn next(window: &Window, _: &mut App) {
    want(window, window.current_view(), false);
}

pub fn next_for(window: &Window, view: EntityId) {
    want(window, view, false);
}

/// Redraws the view on streamed text's beat.
pub fn beat(window: &Window, _: &mut App) {
    want(window, window.current_view(), true);
}

fn want(window: &Window, view: EntityId, beat: bool) {
    let arm = DUE.with(|d| {
        let mut d = d.borrow_mut();
        if beat {
            d.beat.insert(view);
        } else {
            d.tick.insert(view);
        }
        !std::mem::replace(&mut d.armed, true)
    });
    if arm {
        watch(window);
    }
}

/// Looks at each display refresh until a tick is due, then redraws the views that asked. A
/// refresh within half of one of being due counts, so a 120 or 144 Hz screen still lands close
/// to the tick.
fn watch(window: &Window) {
    window.on_next_frame(move |window, cx| {
        let now = Instant::now();
        let ready = DUE.with(|d| {
            let mut d = d.borrow_mut();
            let refresh = d.seen.replace(now).map_or(Duration::ZERO, |s| now - s);
            let due = |at: Option<Instant>, period: Duration| {
                at.is_none_or(|t| now - t + refresh / 2 >= period)
            };
            if !due(d.ticked, TICK) {
                return Some(Vec::new());
            }
            let mut views: Vec<EntityId> = d.tick.drain().collect();
            if !d.beat.is_empty() && due(d.beaten, BEAT) {
                d.beaten = Some(now);
                views.extend(d.beat.drain());
            }
            if views.is_empty() && d.beat.is_empty() {
                *d = Due::default();
                return None;
            }
            if !views.is_empty() {
                d.ticked = Some(now);
            }
            Some(views)
        });
        if let Some(views) = ready {
            for view in views {
                cx.notify(view);
            }
            watch(window);
        }
    });
}

impl<E: IntoElement + 'static> IntoElement for Loop<E> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<E: IntoElement + 'static> Element for Loop<E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let element = self.element.take().expect("laid out once");
        let t = if cx.reduce_motion() {
            0.
        } else {
            want(window, window.current_view(), false);
            phase(self.period)
        };
        let mut element = (self.f)(element, t).into_any_element();
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}
