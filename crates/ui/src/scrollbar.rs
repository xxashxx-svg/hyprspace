// A thin thumb over a list's right edge that shows where the view is, and drags it. GPUI's list
// draws none.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, Bounds, Hitbox, HitboxBehavior, IntoElement, ListState, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, SharedString, Styled, Window, canvas, fill, point, size,
};

use crate::colors;
use crate::zoom::px;

thread_local! {
    /// Where the pointer took hold of each dragged thumb, from the thumb's top.
    static HELD: RefCell<HashMap<SharedString, Pixels>> = RefCell::default();
}

struct Thumb {
    rect: Bounds<Pixels>,
    hitbox: Hitbox,
    track: (Pixels, Pixels),
    max: Pixels,
}

/// The thumb for `list`, drawn over the element it's a child of. `moved` hears each drag step,
/// with whether it went up.
pub fn bar(
    key: impl Into<SharedString>,
    list: ListState,
    moved: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let key = key.into();
    let moved = Rc::new(moved);
    let at = list.clone();
    canvas(
        move |bounds, window, _| {
            let view = at.viewport_bounds();
            let height = view.size.height;
            let max = at.max_offset_for_scrollbar().y;
            if max <= px(1.) || height <= px(0.) {
                return None;
            }
            let top = -at.scroll_px_offset_for_scrollbar().y;
            let thumb = (height * (height / (height + max))).max(px(28.));
            let room = height - thumb;
            let y = view.top() + room * (top / max).clamp(0., 1.);
            let rect = Bounds::new(point(bounds.right() - px(8.), y), size(px(6.), thumb));
            let grab = Bounds::new(point(bounds.right() - px(12.), y), size(px(12.), thumb));
            Some(Thumb {
                rect,
                hitbox: window.insert_hitbox(grab, HitboxBehavior::BlockMouse),
                track: (view.top(), room),
                max,
            })
        },
        move |_, thumb, window, _| {
            let Some(Thumb {
                rect,
                hitbox,
                track,
                max,
            }) = thumb
            else {
                return;
            };
            let held = HELD.with(|h| h.borrow().get(&key).copied());
            let hot = held.is_some() || hitbox.is_hovered(window);
            window.paint_quad(
                fill(rect, colors::ink(if hot { 0.18 } else { 0.08 })).corner_radii(px(3.)),
            );
            let view = window.current_view();
            {
                let (key, hitbox, list) = (key.clone(), hitbox.clone(), list.clone());
                window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                    if phase.bubble() && e.button == MouseButton::Left && hitbox.is_hovered(window)
                    {
                        HELD.with(|h| {
                            h.borrow_mut()
                                .insert(key.clone(), e.position.y - rect.top())
                        });
                        list.scrollbar_drag_started();
                        cx.stop_propagation();
                        cx.notify(view);
                    }
                });
            }
            {
                let (key, list, moved) = (key.clone(), list.clone(), moved.clone());
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    match HELD.with(|h| h.borrow().get(&key).copied()) {
                        Some(grab) if e.dragging() => {
                            let (top, room) = track;
                            let t = if room > px(0.) {
                                ((e.position.y - grab - top) / room).clamp(0., 1.)
                            } else {
                                0.
                            };
                            let up = e.position.y - grab < rect.top();
                            list.set_offset_from_scrollbar(point(px(0.), -(max * t)));
                            moved(up, window, cx);
                            cx.notify(view);
                        }
                        _ if hitbox.is_hovered(window) != hot => cx.notify(view),
                        _ => {}
                    }
                });
            }
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase.bubble() && HELD.with(|h| h.borrow_mut().remove(&key)).is_some() {
                    list.scrollbar_drag_ended();
                    cx.notify(view);
                }
            });
        },
    )
    .absolute()
    .inset_0()
}
