// Hovers that fade in and out. GPUI's `.hover()` and `.group_hover()` styles switch in one frame,
// and an element takes only one `on_hover`, so these read the pointer from a hitbox of their own.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseMoveEvent, Pixels, Position, SharedString,
    Style, Window, fill, relative,
};

use crate::slide::{animations, ease_out};
use crate::zoom::px;

const FADE: Duration = Duration::from_millis(120);
const REVEAL: Duration = Duration::from_millis(200);

thread_local! {
    static WATCHED: RefCell<HashMap<SharedString, Fade>> = RefCell::default();
}

/// A fill of `over` behind the element's other children while the pointer is on it. Put it
/// first, and give it the element's corner radius.
pub fn fill_on_hover(id: impl Into<ElementId>, over: Hsla, radius: f32) -> Hover {
    Hover {
        id: id.into(),
        does: Does::Fill { over, radius },
    }
}

/// Watches the pointer over its parent under `key`, for controls that show while it's there.
/// They take their opacity from [`level`].
pub fn watch(key: SharedString) -> Hover {
    Hover {
        id: ElementId::Name(key.clone()),
        does: Does::Watch(key),
    }
}

/// How far the controls under `key` have faded in, 0 to 1.
pub fn level(key: &SharedString) -> f32 {
    WATCHED.with(|w| w.borrow().get(key).map_or(0., Fade::level))
}

pub struct Hover {
    id: ElementId,
    does: Does,
}

enum Does {
    Fill { over: Hsla, radius: f32 },
    Watch(SharedString),
}

#[derive(Clone, Copy)]
struct Fade {
    on: bool,
    from: f32,
    at: Instant,
    length: Duration,
}

impl Fade {
    fn new(on: bool, length: Duration) -> Self {
        Fade {
            on,
            from: if on { 1. } else { 0. },
            at: Instant::now() - length,
            length,
        }
    }

    fn turn(self, on: bool) -> Self {
        if self.on == on {
            return self;
        }
        Fade {
            on,
            from: self.level(),
            at: Instant::now(),
            length: self.length,
        }
    }

    fn level(&self) -> f32 {
        let to = if self.on { 1. } else { 0. };
        if !animations() {
            return to;
        }
        let t = ease_out((self.at.elapsed().as_secs_f32() / self.length.as_secs_f32()).min(1.));
        self.from + (to - self.from) * t
    }

    fn settled(&self) -> bool {
        !animations() || self.at.elapsed() >= self.length
    }
}

impl IntoElement for Hover {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Hover {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
    ) -> (LayoutId, ()) {
        let mut style = Style {
            position: Position::Absolute,
            ..Default::default()
        };
        style.inset.top = Pixels::ZERO.into();
        style.inset.left = Pixels::ZERO.into();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) -> Hitbox {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let on = hitbox.is_hovered(window);
        let fade = match &self.does {
            Does::Fill { over, radius } => {
                let fade = window.with_element_state(id.expect("has an id"), |state, _| {
                    let fade = state.unwrap_or(Fade::new(on, FADE)).turn(on);
                    (fade, fade)
                });
                let level = fade.level();
                if level > 0. {
                    window.paint_quad(fill(bounds, over.opacity(level)).corner_radii(px(*radius)));
                }
                fade
            }
            Does::Watch(key) => WATCHED.with(|w| {
                let mut w = w.borrow_mut();
                let fade = w
                    .get(key)
                    .copied()
                    .unwrap_or(Fade::new(on, REVEAL))
                    .turn(on);
                if !on && fade.settled() {
                    w.remove(key);
                } else {
                    w.insert(key.clone(), fade);
                }
                fade
            }),
        };
        if !fade.settled() {
            crate::pace::next(window, cx);
        }
        let view = window.current_view();
        let hitbox = hitbox.clone();
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
            if phase == gpui::DispatchPhase::Capture && hitbox.is_hovered(window) != on {
                cx.notify(view);
            }
        });
    }
}
