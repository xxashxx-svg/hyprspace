use std::cell::RefCell;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, EntityId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window,
};

// a loop paced by the display redraws the whole window up to 180 times a second, and GPUI's own
// timers on Windows only fire on the 15.6 ms system tick
const TICK: Duration = Duration::from_micros(16_667);

thread_local! {
    static EPOCH: Instant = Instant::now();
    static DUE: RefCell<Option<HashSet<EntityId>>> = const { RefCell::new(None) };
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

pub fn next(window: &Window, cx: &mut App) {
    want(window.current_view(), cx);
}

fn want(view: EntityId, cx: &mut App) {
    let idle = DUE.with(|d| {
        let mut d = d.borrow_mut();
        let idle = d.is_none();
        d.get_or_insert_default().insert(view);
        idle
    });
    if !idle {
        return;
    }
    let (mut tx, mut ticks) = mpsc::channel(1);
    std::thread::spawn(move || {
        while !tx.is_closed() {
            std::thread::sleep(TICK);
            let _ = tx.try_send(());
        }
    });
    cx.spawn(async move |cx| {
        while ticks.next().await.is_some() {
            let views = DUE.with(|d| d.borrow_mut().as_mut().map(std::mem::take));
            let Some(views) = views.filter(|v| !v.is_empty()) else {
                DUE.with(|d| d.borrow_mut().take());
                break;
            };
            cx.update(|cx| {
                for view in views {
                    cx.notify(view);
                }
            });
        }
    })
    .detach();
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
            want(window.current_view(), cx);
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
