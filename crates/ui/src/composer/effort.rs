// The effort menu, after Codex's: the level's name over a chip for the model, and a thick pill
// slider with a round knob across the model's reasoning levels, lowest on the left. The top level
// lights the fill with a gradient and a few twinkling sparks. A switch for the 1M context window
// sits under it when the model has one. Clicking or dragging moves the knob and the arrows step
// it. A level applies when the mouse lets go or the menu closes, so a drag across four levels is
// one change, not four.

use std::cell::Cell;
use std::f32::consts::PI;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, ClickEvent, Context, DispatchPhase, FontWeight,
    Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Window, canvas, div, linear_color_stop, linear_gradient, prelude::*, px, relative,
};

use super::model_menu::{Choice, Host, ModelMenu, Spec, close, to_models};
use super::pickers::effort_label;
use crate::assets::icon;
use crate::colors;
use crate::slide::Glide;

/// The card is a fixed width, so the track's length is known before it is first painted.
pub const CARD: f32 = 260.;
const PAD: f32 = 14.;
const TRACK: f32 = CARD - 2. * PAD;
const TRACK_H: f32 = 26.;
const KNOB: f32 = 22.;
/// From the track's ends to the knob's center at either end.
const END: f32 = (TRACK_H - KNOB) / 2. + KNOB / 2.;
/// The window switch's two segments.
const SEGMENT: f32 = 60.;

#[derive(Default)]
pub struct Slider {
    /// The level under the knob while it moves, before it applies.
    level: Option<String>,
    dragging: bool,
    track: Rc<Cell<Option<Bounds<Pixels>>>>,
    knob: Glide,
    window: Glide,
}

impl Slider {
    /// The level the knob moved to and hasn't applied yet.
    pub fn take(&mut self) -> Option<String> {
        self.dragging = false;
        self.level.take()
    }
}

/// The level the knob is on: one being moved, else the pick, with the default standing for the
/// level it comes to. None when that isn't known.
fn at(slider: &Slider, spec: &Spec) -> Option<usize> {
    let level = slider.level.as_deref().unwrap_or(&spec.effort);
    let level = match level {
        "" => spec.default_effort.as_deref()?,
        l => l,
    };
    spec.efforts.iter().position(|l| l == level)
}

/// The knob's center along the track at `f`, 0 for the lowest level and 1 for the top.
fn knob_x(f: f32) -> f32 {
    END + f * (TRACK - 2. * END)
}

/// The level nearest `x` along a track with `n` stops.
fn index_at(x: Pixels, track: Bounds<Pixels>, n: usize) -> usize {
    if n < 2 {
        return 0;
    }
    let travel = track.size.width - px(2. * END);
    let f = ((x - track.origin.x - px(END)) / travel).clamp(0., 1.);
    (f * (n - 1) as f32).round() as usize
}

/// The top level's second color: the accent turned a little around the wheel.
fn peak(accent: Hsla) -> Hsla {
    Hsla {
        h: (accent.h + 0.13).fract(),
        ..accent
    }
}

/// Applies `level` unless it's what the thread already runs at.
pub(super) fn apply<H: Host>(h: &mut H, level: String, cx: &mut Context<H>) {
    let Some(spec) = h.model_spec() else {
        return;
    };
    let now = match spec.effort.as_str() {
        "" => spec.default_effort.clone().unwrap_or_default(),
        e => e.to_string(),
    };
    if level != now {
        h.choose(Choice::Effort(level), cx);
    }
}

fn slide_to<H: Host>(h: &mut H, x: Pixels, cx: &mut Context<H>) {
    let Some(spec) = h.model_spec() else {
        return;
    };
    let Some(m) = h.model_menu() else {
        return;
    };
    let Some(track) = m.slider.track.get() else {
        return;
    };
    let level = spec
        .efforts
        .get(index_at(x, track, spec.efforts.len()))
        .cloned();
    if level != m.slider.level {
        m.slider.level = level;
        cx.notify();
    }
}

fn release<H: Host>(h: &mut H, cx: &mut Context<H>) {
    let Some(m) = h.model_menu() else {
        return;
    };
    if !m.slider.dragging {
        return;
    }
    if let Some(level) = m.slider.take() {
        apply(h, level, cx);
    }
    cx.notify();
}

pub(super) fn key<H: Host>(
    h: &mut H,
    spec: &Spec,
    e: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<H>,
) {
    let n = spec.efforts.len();
    match e.keystroke.key.as_str() {
        "enter" | "escape" => close(h, window, cx),
        k @ ("left" | "right" | "up" | "down" | "home" | "end") if n > 0 => {
            let Some(m) = h.model_menu() else {
                return;
            };
            let now = at(&m.slider, spec);
            let i = match k {
                "left" | "down" => now.map_or(0, |i| i.saturating_sub(1)),
                "right" | "up" => now.map_or(0, |i| (i + 1).min(n - 1)),
                "home" => 0,
                _ => n - 1,
            };
            m.slider.level = Some(spec.efforts[i].clone());
            cx.notify();
        }
        _ => return,
    }
    cx.stop_propagation();
}

/// Sparks over the top level's fill, each twinkling on its own beat.
fn sparks() -> impl Iterator<Item = AnyElement> {
    (0..14usize).map(|k| {
        let x = (k * 37 % 100) as f32 / 100.;
        let y = 4. + (k * 7 % 17) as f32;
        let size = if k % 3 == 0 { 2.5 } else { 1.5 };
        div()
            .absolute()
            .left(relative(x))
            .top(px(y))
            .size(px(size))
            .rounded_full()
            .bg(colors::on_accent())
            .with_animation(
                ("spark", k),
                Animation::new(Duration::from_millis(1100 + k as u64 * 230)).repeat(),
                |d, t| d.opacity(0.1 + 0.8 * (t * PI).sin()),
            )
            .into_any_element()
    })
}

pub(super) fn body<H: Host>(m: &ModelMenu, spec: &Spec, cx: &mut Context<H>) -> AnyElement {
    let s = &m.slider;
    let now = at(s, spec);
    let n = spec.efforts.len();
    let top = n >= 3 && now == Some(n - 1);
    let accent = colors::accent();
    let model = spec
        .models
        .iter()
        .find(|x| x.agent == spec.agent && x.id == crate::models::windowed(&spec.model, false))
        .map_or_else(|| "Default".to_string(), |x| x.label.clone());
    let changed = !spec.effort.is_empty() || s.level.is_some();

    let name = match now {
        Some(i) => div()
            .text_color(if top { peak(accent) } else { accent })
            .child(effort_label(&spec.efforts[i])),
        None => div().text_color(colors::text1()).child("Default"),
    }
    .text_size(px(14.))
    .font_weight(FontWeight::SEMIBOLD)
    // each new level fades up into place
    .with_animation(
        ("effort-name", now.map_or(0, |i| i + 1)),
        Animation::new(Duration::from_millis(160)).with_easing(crate::slide::ease_out),
        |d, t| d.opacity(0.3 + 0.7 * t),
    );
    let chip = div()
        .id("effort-model")
        .flex()
        .items_center()
        .gap(px(2.))
        .pl(px(7.))
        .pr(px(4.))
        .py(px(2.))
        .rounded(px(6.))
        .bg(colors::ink(0.06))
        .hover(|s| s.bg(colors::ink(0.1)))
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(colors::text2())
        .child(model)
        .child(icon("chevron-right", 11., colors::text3()))
        .on_click(cx.listener(|h: &mut H, _: &ClickEvent, window, cx| to_models(h, window, cx)));
    let reset = changed.then(|| {
        div()
            .id("effort-reset")
            .absolute()
            .top(px(0.))
            .right(px(0.))
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.08)))
            .child(icon("rotate-ccw", 13., colors::text3()))
            .on_click(cx.listener(|h: &mut H, _: &ClickEvent, _, cx| {
                if let Some(m) = h.model_menu() {
                    m.slider.take();
                }
                h.choose(Choice::Effort(String::new()), cx);
                cx.notify();
            }))
    });
    let head = div()
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.))
        .child(name)
        .child(chip)
        .children(reset);

    let slider = (n > 0).then(|| {
        let frac = move |i: usize| {
            if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.5
            }
        };
        let motion = s.knob.toward(now.map_or(0., frac));
        let fill = now.map(|_| {
            let bg = if top {
                linear_gradient(
                    90.,
                    linear_color_stop(accent, 0.),
                    linear_color_stop(peak(accent), 1.),
                )
            } else {
                accent.into()
            };
            motion.apply(
                "effort-fill",
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h(px(TRACK_H))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(bg)
                    .when(top, |d| d.children(sparks())),
                |d, f| d.w(px(knob_x(f) + END)),
            )
        });
        let stops = (0..n).map(|i| {
            let lit = now.is_some_and(|a| i <= a);
            div()
                .absolute()
                .top(px((TRACK_H - 4.) / 2.))
                .left(px(knob_x(frac(i)) - 2.))
                .size(px(4.))
                .rounded_full()
                .bg(if lit {
                    colors::on_accent().opacity(0.55)
                } else {
                    colors::ink(0.3)
                })
        });
        let knob = motion.apply(
            "effort-knob",
            div()
                .absolute()
                .top(px((TRACK_H - KNOB) / 2.))
                .size(px(KNOB))
                .rounded_full()
                .bg(colors::on_accent())
                .shadow_sm()
                // without a level picked, it waits at the low end
                .when(now.is_none(), |d| d.opacity(0.5)),
            |d, f| d.left(px(knob_x(f) - KNOB / 2.)),
        );
        let track = s.track.clone();
        let host = cx.weak_entity();
        // a drag keeps going outside the menu, so it listens on the window, not the track
        let listen = canvas(
            move |b, _, _| track.set(Some(b)),
            move |_, _, window, _| {
                let moved = host.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble && e.pressed_button == Some(MouseButton::Left)
                    {
                        let _ = moved.update(cx, |h, cx| {
                            if h.model_menu().as_ref().is_some_and(|m| m.slider.dragging) {
                                slide_to(h, e.position.x, cx);
                            }
                        });
                    }
                });
                let up = host.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        let _ = up.update(cx, |h, cx| release(h, cx));
                    }
                });
            },
        )
        .absolute()
        .size_full();
        div()
            .id("effort-slider")
            .relative()
            .w(px(TRACK))
            .h(px(TRACK_H))
            .rounded_full()
            .bg(colors::ink(0.08))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|h: &mut H, e: &MouseDownEvent, _, cx| {
                    if let Some(m) = h.model_menu() {
                        m.slider.dragging = true;
                    }
                    slide_to(h, e.position.x, cx);
                }),
            )
            .children(fill)
            .children(stops)
            .child(knob)
            .child(listen)
    });

    let window = spec.long.map(|long| {
        let pill = s.window.toward(if long { SEGMENT } else { 0. }).apply(
            "window-pill",
            div()
                .absolute()
                .top(px(2.))
                .left(px(2.))
                .w(px(SEGMENT))
                .h(px(22.))
                .rounded(px(6.))
                .bg(colors::ink(0.1)),
            |d, x| d.ml(px(x)),
        );
        let segment = |on: bool, label: &'static str| {
            div()
                .id(("window", on as usize))
                .w(px(SEGMENT))
                .h(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.5))
                .cursor_pointer()
                .text_color(if long == on {
                    colors::text1()
                } else {
                    colors::text3()
                })
                .child(label)
                .on_click(cx.listener(move |h: &mut H, _: &ClickEvent, _, cx| {
                    h.choose(Choice::Long(on), cx);
                    cx.notify();
                }))
        };
        div()
            .flex()
            .items_center()
            .pt(px(10.))
            .border_t_1()
            .border_color(colors::ink(0.08))
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .child("Context"),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .p(px(2.))
                    .rounded(px(8.))
                    .bg(colors::ink(0.05))
                    .child(pill)
                    .child(segment(false, "Standard"))
                    .child(segment(true, "1M")),
            )
    });

    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(PAD))
        .child(head)
        .children(slider)
        .children(window)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::super::model_menu::tests::spec;
    use super::*;
    use gpui::{point, size};

    #[test]
    fn the_knob_sits_on_the_level_the_default_comes_to() {
        let s = spec();
        let mut slider = Slider::default();
        assert_eq!(at(&slider, &s), Some(1));
        slider.level = Some("low".into());
        assert_eq!(at(&slider, &s), Some(0));
        let unknown = Spec {
            default_effort: None,
            ..spec()
        };
        assert_eq!(at(&Slider::default(), &unknown), None);
    }

    #[test]
    fn a_click_picks_the_nearest_stop() {
        // the knob's travel is the track less its ends, 13 px a side
        let track = Bounds::new(point(px(100.), px(0.)), size(px(226.), px(26.)));
        assert_eq!(index_at(px(90.), track, 5), 0);
        assert_eq!(index_at(px(113.), track, 5), 0);
        assert_eq!(index_at(px(163.), track, 5), 1);
        assert_eq!(index_at(px(263.), track, 5), 3);
        assert_eq!(index_at(px(400.), track, 5), 4);
        assert_eq!(knob_x(0.), END);
        assert_eq!(knob_x(1.), TRACK - END);
    }
}
