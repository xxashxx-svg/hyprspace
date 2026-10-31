use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, Div, EventEmitter, FocusHandle, Focusable,
    FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit,
    Pixels, RenderImage, ScrollWheelEvent, Stateful, StyledImage, Window, canvas, div, img, point,
    prelude::*,
};
use hyprspace_media::{Player, State};

use crate::assets::icon;
use crate::colors;
use crate::viewer::lightbox::fitted;
use crate::viewer::pan::Pan;
use crate::zoom::px;

const SKIP: f64 = 5.;
/// A seek while paused shows its frame a beat later, so frames are still read for this long.
const SETTLE: Duration = Duration::from_millis(600);
const BAR_H: f32 = 40.;
const MIN_BAR_W: f32 = 420.;

#[derive(Clone, Copy, PartialEq)]
enum Slider {
    Seek,
    Volume,
}

pub enum VideoEvent {
    Close,
    OpenOutside(PathBuf),
}

pub struct Video {
    path: PathBuf,
    name: String,
    player: Player,
    shown: Option<Arc<RenderImage>>,
    state: State,
    settle_until: Instant,
    held: Option<Slider>,
    pan: Pan,
    track: Rc<Cell<Bounds<Pixels>>>,
    volume_track: Rc<Cell<Bounds<Pixels>>>,
    volume_open: bool,
    focus: FocusHandle,
    back: Option<FocusHandle>,
    took_focus: bool,
}

impl Focusable for Video {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<VideoEvent> for Video {}

fn slider(id: &'static str, share: f32, bounds: Rc<Cell<Bounds<Pixels>>>) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .h(px(18.))
        .flex()
        .items_center()
        .cursor_pointer()
        .child(
            canvas(|_, _, _| (), move |b, _, _, _| bounds.set(b))
                .absolute()
                .size_full(),
        )
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.))
                .rounded_full()
                .bg(colors::ink(0.14))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(gpui::relative(share))
                        .rounded_full()
                        .bg(colors::accent()),
                ),
        )
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.) as u64;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

impl Video {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            player: Player::open(&path, (1920, 1080)),
            path,
            shown: None,
            state: State::default(),
            settle_until: Instant::now(),
            held: None,
            pan: Pan::new(),
            track: Rc::new(Cell::new(Bounds::default())),
            volume_track: Rc::new(Cell::new(Bounds::default())),
            volume_open: false,
            focus: cx.focus_handle(),
            back: None,
            took_focus: false,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.shown.take() {
            let _ = window.drop_image(old);
        }
        if let Some(back) = self.back.take() {
            window.defer(cx, move |window, cx| window.focus(&back, cx));
        }
        cx.emit(VideoEvent::Close);
    }

    fn settle(&mut self, cx: &mut Context<Self>) {
        self.settle_until = Instant::now() + SETTLE;
        cx.notify();
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.state.playing {
            self.player.pause();
        } else {
            self.player.play();
        }
        self.settle(cx);
    }

    fn seek(&mut self, to: f64, cx: &mut Context<Self>) {
        let to = to.clamp(0., self.state.duration.max(0.));
        self.player.seek(to);
        self.state.position = to;
        self.settle(cx);
    }

    fn set_volume(&mut self, volume: f64, cx: &mut Context<Self>) {
        self.player.set_volume(volume);
        if self.state.muted && volume > 0. {
            self.player.mute(false);
        }
        self.state.volume = volume;
        self.settle(cx);
    }

    fn slide(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(held) = self.held else {
            return;
        };
        let b = match held {
            Slider::Seek => self.track.get(),
            Slider::Volume => self.volume_track.get(),
        };
        if b.size.width <= px(0.) {
            return;
        }
        let share = ((x - b.origin.x) / b.size.width).clamp(0., 1.) as f64;
        match held {
            Slider::Seek => self.seek(share * self.state.duration, cx),
            Slider::Volume => self.set_volume(share, cx),
        }
    }

    fn hold(&mut self, slider: Slider, e: &MouseDownEvent, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.held = Some(slider);
        self.slide(e.position.x, cx);
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => self.close(window, cx),
            "space" => self.toggle(cx),
            "left" => self.seek(self.state.position - SKIP, cx),
            "right" => self.seek(self.state.position + SKIP, cx),
            "m" => {
                self.player.mute(!self.state.muted);
                self.settle(cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn round_button(
        &self,
        id: &'static str,
        name: &'static str,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex_none()
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.08)))
            .child(icon(name, 14., colors::text1()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |v, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    action(v, window, cx);
                }),
            )
    }

    fn controls(&self, width: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.state;
        let share = if s.duration > 0. {
            (s.position / s.duration).clamp(0., 1.) as f32
        } else {
            0.
        };
        let silent = s.muted || s.volume <= 0.;
        let volume = if silent { 0. } else { s.volume as f32 };
        let seek = slider("video-track", share, self.track.clone())
            .flex_1()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, e, _, cx| v.hold(Slider::Seek, e, cx)),
            );
        let open = self.volume_open || self.held == Some(Slider::Volume);
        let sound = div()
            .id("video-volume")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .on_hover(cx.listener(|v, hovered: &bool, _, cx| {
                v.volume_open = *hovered;
                cx.notify();
            }))
            .when(open, |d| {
                d.child(
                    slider("video-volume-track", volume, self.volume_track.clone())
                        .w(px(72.))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|v, e, _, cx| v.hold(Slider::Volume, e, cx)),
                        ),
                )
            })
            .child(self.round_button(
                "video-mute",
                if silent { "volume-x" } else { "volume-2" },
                cx,
                |v, _, cx| {
                    v.player.mute(!v.state.muted);
                    v.settle(cx);
                },
            ));
        div()
            .id("video-controls")
            .w(px(width))
            .h(px(BAR_H))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(12.))
            .bg(colors::surface2())
            .border_1()
            .border_color(colors::border2())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(self.round_button(
                "video-play",
                if s.playing { "pause" } else { "play" },
                cx,
                |v, _, cx| v.toggle(cx),
            ))
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .child(format!("{} / {}", clock(s.position), clock(s.duration))),
            )
            .child(seek)
            .child(sound)
    }

    fn failed(&self, cx: &mut Context<Self>) -> AnyElement {
        let path = self.path.clone();
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(colors::surface2())
            .border_1()
            .border_color(colors::border2())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child("This video can't play here."),
            )
            .child(
                crate::widgets::button("video-outside", "Open in your player").on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(VideoEvent::OpenOutside(path.clone()))),
                ),
            )
            .into_any_element()
    }
}

impl Render for Video {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.took_focus {
            self.took_focus = true;
            self.back = window.focused(cx);
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        if let Some(next) = self.player.frame().and_then(crate::attach::render_image)
            && let Some(old) = self.shown.replace(next)
        {
            let _ = window.drop_image(old);
        }
        let before = self.state;
        self.state = self.player.state();
        match self.held {
            Some(Slider::Seek) => self.state.position = before.position,
            Some(Slider::Volume) => self.state.volume = before.volume,
            None => {}
        }
        let waiting = self.shown.is_none() && !self.state.failed;
        if self.state.playing
            || waiting
            || self.held.is_some()
            || Instant::now() < self.settle_until
        {
            crate::pace::next(window, cx);
        }

        let view = window.viewport_size();
        let z = crate::zoom::zoom();
        let (vw, vh) = (f32::from(view.width) / z, f32::from(view.height) / z);
        let size = self.shown.as_ref().map(|i| {
            let s = i.size(0);
            (s.width.0 as u32, s.height.0 as u32)
        });
        let (fw, fh) = fitted(size, (vw, vh - BAR_H - 12.));
        let home = ((vh - fh - BAR_H - 12.) / 2.).max(12.);
        self.pan.center = point(px(vw / 2.), px(home + fh / 2.));
        self.pan.ease(window);
        let (w, h) = (fw * self.pan.zoom, fh * self.pan.zoom);
        let left = vw / 2. + f32::from(self.pan.offset.x) / z - w / 2.;
        let top = home + fh / 2. + f32::from(self.pan.offset.y) / z - h / 2.;
        let bar_w = fw.max(MIN_BAR_W).min(vw - 24.);
        let dragging = self.pan.dragging();
        let scrim = colors::hsla(colors::theme().shadow);

        let picture: AnyElement = if self.state.failed {
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(vh / 2. - 60.))
                .flex()
                .justify_center()
                .child(self.failed(cx))
                .into_any_element()
        } else {
            div()
                .id("video-picture")
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(w))
                .h(px(h))
                .rounded(px(8.))
                .overflow_hidden()
                .bg(colors::ink(0.06))
                .cursor(if dragging {
                    CursorStyle::ClosedHand
                } else {
                    CursorStyle::OpenHand
                })
                .children(
                    self.shown
                        .clone()
                        .map(|i| img(i).size_full().object_fit(ObjectFit::Contain)),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|v, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if e.click_count == 2 {
                            v.pan.fit();
                        }
                        v.pan.grab(e.position);
                        cx.notify();
                    }),
                )
                .into_any_element()
        };
        let controls = (!self.state.failed).then(|| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(home + fh + 12.))
                .flex()
                .justify_center()
                .child(self.controls(bar_w, cx))
        });
        let close = div()
            .id("video-close")
            .absolute()
            .right(px(16.))
            .top(px(16.))
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
                cx.listener(|v, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    v.close(window, cx);
                }),
            );
        let caption = div()
            .absolute()
            .left(px(16.))
            .top(px(20.))
            .text_size(px(12.))
            .text_color(colors::text2())
            .child(if self.pan.target() == 1. {
                self.name.clone()
            } else {
                format!("{}  {}%", self.name, (self.pan.target() * 100.).round())
            });
        let body = div()
            .id("video")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .relative()
            .w(view.width)
            .h(view.height)
            .occlude()
            .bg(scrim.opacity(0.9))
            .when(dragging, |d| d.cursor(CursorStyle::ClosedHand))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|v, _: &MouseDownEvent, window, cx| v.close(window, cx)),
            )
            .on_scroll_wheel(cx.listener(|v, e: &ScrollWheelEvent, _, cx| {
                v.pan.wheel(e);
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, cx| {
                if v.pan.drag(e) {
                    cx.notify();
                }
                if v.held.is_none() {
                    return;
                }
                // a button let go outside the window ends the scrub at the next move
                if e.pressed_button != Some(MouseButton::Left) {
                    v.held = None;
                    cx.notify();
                } else {
                    v.slide(e.position.x, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|v, e: &MouseUpEvent, _, cx| {
                    v.held = None;
                    if v.pan.release(e.position) {
                        v.toggle(cx);
                    }
                    cx.notify();
                }),
            )
            .child(picture)
            .children(controls)
            .child(caption)
            .child(close);
        let body: AnyElement = crate::slide::ease_in(body, "video-in", 140, |d, t| d.opacity(t));
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_a_player() {
        assert_eq!(clock(0.), "0:00");
        assert_eq!(clock(65.4), "1:05");
        assert_eq!(clock(3_725.), "1:02:05");
        assert_eq!(clock(-1.), "0:00");
    }
}
