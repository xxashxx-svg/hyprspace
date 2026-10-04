// An image over the whole window, after T3 Code's: Ctrl+click on an image in a terminal opens it
// here rather than in the viewer card. The wheel zooms around the pointer, easing into each step; dragging
// moves the image; a double-click fits it again. Esc, the close button or a click outside closes
// it. It is its own view, so a frame of zoom or drag redraws only this, and it holds the keyboard
// while it is open, so nothing typed reaches the terminal under it.

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, StyledImage, Window, div, img, point, prelude::*, px,
};

use crate::assets::icon;
use crate::colors;

/// The share of the window an image fills when it opens. A smaller one shows at its own size.
const FIT: (f32, f32) = (0.86, 0.78);
/// Before its size is read, the image draws in a box this big.
const UNSIZED: (f32, f32) = (480., 320.);
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 10.;
/// Each wheel line zooms by this much; a mouse wheel's notch is three lines on Windows, about a
/// fifth. A touchpad's pixels count as lines this many to one.
const STEP: f32 = 1.06;
const PIXELS_PER_LINE: f32 = 20.;
/// The share of the way to the wheel's zoom each frame covers.
const EASE: f32 = 0.28;

/// The lightbox asks to close: its button, or a click outside the image.
pub struct Close;

pub struct Lightbox {
    path: PathBuf,
    name: String,
    size: Option<(u32, u32)>,
    zoom: f32,
    /// The zoom the wheel is heading to, and the point it zooms around.
    target: f32,
    anchor: Point<Pixels>,
    /// How far the image's center sits from the window's.
    offset: Point<Pixels>,
    /// While dragging: where the pointer went down, and the offset then.
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
    /// The window's center, as of the last frame.
    center: Point<Pixels>,
    focus: FocusHandle,
    /// What had the keyboard before, which gets it back on close. Taken on the first frame.
    back: Option<FocusHandle>,
    took_focus: bool,
}

impl Focusable for Lightbox {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<Close> for Lightbox {}

/// The image's size when it opens: fitted into the window's share, never scaled up.
fn fitted(size: Option<(u32, u32)>, window: (f32, f32)) -> (f32, f32) {
    let Some((w, h)) = size.filter(|(w, h)| *w > 0 && *h > 0) else {
        return UNSIZED;
    };
    let (w, h) = (w as f32, h as f32);
    let scale = (window.0 * FIT.0 / w).min(window.1 * FIT.1 / h).min(1.);
    (w * scale, h * scale)
}

/// The offset after zooming from `from` to `to` around `anchor`, so the point under it stays put.
/// `center` is the image's center before the zoom.
fn zoom_around(
    offset: Point<Pixels>,
    center: Point<Pixels>,
    anchor: Point<Pixels>,
    from: f32,
    to: f32,
) -> Point<Pixels> {
    let k = 1. - to / from;
    point(
        offset.x + (anchor.x - center.x) * k,
        offset.y + (anchor.y - center.y) * k,
    )
}

impl Lightbox {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        // the size is read off the file's header, away from the UI thread
        let file = path.clone();
        cx.spawn(async move |this, cx| {
            let size = cx
                .background_executor()
                .spawn(async move { image::image_dimensions(&file).ok() })
                .await;
            let _ = this.update(cx, |l, cx| {
                l.size = size;
                cx.notify();
            });
        })
        .detach();
        Self {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            path,
            size: None,
            zoom: 1.,
            target: 1.,
            anchor: point(px(0.), px(0.)),
            offset: point(px(0.), px(0.)),
            drag: None,
            center: point(px(0.), px(0.)),
            focus: cx.focus_handle(),
            back: None,
            took_focus: false,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // after the click that closed it, which would otherwise hand the keyboard back to this
        if let Some(back) = self.back.take() {
            window.defer(cx, move |window, cx| window.focus(&back, cx));
        }
        cx.emit(Close);
    }

    fn wheel(&mut self, e: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let lines = match e.delta {
            ScrollDelta::Lines(d) => d.y,
            ScrollDelta::Pixels(d) => f32::from(d.y) / PIXELS_PER_LINE,
        };
        self.target = (self.target * STEP.powf(lines)).clamp(MIN_ZOOM, MAX_ZOOM);
        self.anchor = e.position;
        cx.notify();
    }

    /// One frame of easing toward the wheel's zoom.
    fn ease(&mut self, window: &mut Window) {
        if self.zoom == self.target {
            return;
        }
        let near = (self.target - self.zoom).abs() < 0.002;
        let next = if near {
            self.target
        } else {
            self.zoom + (self.target - self.zoom) * EASE
        };
        let center = point(self.center.x + self.offset.x, self.center.y + self.offset.y);
        self.offset = zoom_around(self.offset, center, self.anchor, self.zoom, next);
        self.zoom = next;
        if !near {
            window.request_animation_frame();
        }
    }

    fn fit(&mut self) {
        self.zoom = 1.;
        self.target = 1.;
        self.offset = point(px(0.), px(0.));
    }
}

impl Render for Lightbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.took_focus {
            self.took_focus = true;
            self.back = window.focused(cx);
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        let view = window.viewport_size();
        let (vw, vh) = (f32::from(view.width), f32::from(view.height));
        self.center = point(view.width / 2., view.height / 2.);
        self.ease(window);
        let (fw, fh) = fitted(self.size, (vw, vh));
        let (w, h) = (fw * self.zoom, fh * self.zoom);
        let left = vw / 2. + f32::from(self.offset.x) - w / 2.;
        let top = vh / 2. + f32::from(self.offset.y) - h / 2.;
        let dragging = self.drag.is_some();
        let scrim = colors::hsla(colors::theme().shadow);
        let close = div()
            .id("lightbox-close")
            .absolute()
            // past the image's top right corner, kept on screen
            .left(px((left + w + 8.).min(vw - 40.)))
            .top(px((top - 36.).max(12.)))
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded_full()
            .bg(colors::surface3())
            .border_1()
            .border_color(colors::border2())
            .cursor_pointer()
            .hover(|s| s.bg(colors::surface2()))
            .child(icon("x", 14., colors::text1()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    l.close(window, cx);
                }),
            );
        let caption = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px((top + h + 12.).min(vh - 34.)))
            .flex()
            .justify_center()
            .child(
                div()
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .bg(colors::surface2())
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .child(if self.target == 1. {
                        self.name.clone()
                    } else {
                        format!("{}  {}%", self.name, (self.target * 100.).round())
                    }),
            );
        let image = div()
            .id("lightbox-image")
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(w))
            .h(px(h))
            .cursor(if dragging {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .child(
                img(self.path.clone())
                    .size_full()
                    .rounded(px(8.))
                    .object_fit(ObjectFit::Contain),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if e.click_count == 2 {
                        l.fit();
                        l.drag = None;
                    } else {
                        l.drag = Some((e.position, l.offset));
                    }
                    cx.notify();
                }),
            );
        let body = div()
            .id("lightbox")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|l, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    l.close(window, cx);
                }
            }))
            .relative()
            .w(view.width)
            .h(view.height)
            .occlude()
            .bg(scrim.opacity(0.86))
            .when(dragging, |d| d.cursor(CursorStyle::ClosedHand))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, _: &MouseDownEvent, window, cx| l.close(window, cx)),
            )
            .on_scroll_wheel(cx.listener(|l, e: &ScrollWheelEvent, _, cx| l.wheel(e, cx)))
            .on_mouse_move(cx.listener(|l, e: &MouseMoveEvent, _, cx| {
                let Some((from, start)) = l.drag else {
                    return;
                };
                // a button let go outside the window ends the drag at the next move
                if e.pressed_button != Some(MouseButton::Left) {
                    l.drag = None;
                } else {
                    l.offset = point(
                        start.x + e.position.x - from.x,
                        start.y + e.position.y - from.y,
                    );
                }
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|l, _: &MouseUpEvent, _, cx| {
                    l.drag = None;
                    cx.notify();
                }),
            )
            .child(image)
            .child(caption)
            .child(close);
        let body: AnyElement = crate::slide::ease_in(body, "lightbox-in", 140, |d, t| d.opacity(t));
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_fitted_and_never_scaled_up() {
        assert_eq!(fitted(Some((200, 100)), (1000., 800.)), (200., 100.));
        let (w, h) = fitted(Some((4000, 2000)), (1000., 800.));
        assert!((w - 860.).abs() < 0.01 && (h - 430.).abs() < 0.01);
        assert_eq!(fitted(None, (1000., 800.)), UNSIZED);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_still() {
        let center = point(px(500.), px(400.));
        let anchor = point(px(600.), px(450.));
        let offset = zoom_around(point(px(0.), px(0.)), center, anchor, 1., 2.);
        // the image point under the anchor was 100,50 from its center; at 2x it is 200,100, and
        // the new center sits that far back from the anchor
        assert_eq!(offset, point(px(-100.), px(-50.)));
        let back = zoom_around(offset, center + offset, anchor, 2., 1.);
        assert_eq!(back, point(px(0.), px(0.)));
    }
}
