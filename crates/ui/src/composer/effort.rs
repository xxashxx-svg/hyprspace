// The effort menu: a slider across the model's reasoning levels, lowest on the left, and a switch
// for the 1M context window when the model has one. Clicking or dragging moves the knob and the
// arrows step it. A level applies when the mouse lets go or the menu closes, so a drag across
// four levels is one change, not four.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, DispatchPhase, FontWeight, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window, canvas, div,
    prelude::*, px, relative,
};

use super::model_menu::{Choice, Host, ModelMenu, Spec, badge, close};
use super::pickers::effort_label;
use crate::colors;
use crate::slide::Glide;

/// The window switch's two segments.
const SEGMENT: f32 = 68.;

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

/// The level nearest `x` along a track with `n` stops.
fn index_at(x: Pixels, track: Bounds<Pixels>, n: usize) -> usize {
    if n < 2 {
        return 0;
    }
    let f = ((x - track.origin.x) / track.size.width).clamp(0., 1.);
    (f * (n - 1) as f32).round() as usize
}

/// A level's name under its stop, short enough for six side by side.
fn tick(level: &str) -> String {
    match level {
        "minimal" => "Min".into(),
        "medium" => "Med".into(),
        "xhigh" => "XHigh".into(),
        other => effort_label(other),
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

pub(super) fn body<H: Host>(m: &ModelMenu, spec: &Spec, cx: &mut Context<H>) -> AnyElement {
    let s = &m.slider;
    let now = at(s, spec);
    let n = spec.efforts.len();
    let is_default = now.is_some_and(|i| spec.default_effort.as_ref() == Some(&spec.efforts[i]));
    // without a known default level, the CLI's own default is a choice of its own
    let unknown = spec.default_effort.is_none();
    let on_default = now.is_none();
    let head = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .flex_1()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child("Reasoning"),
        )
        .children(now.map(|i| {
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(effort_label(&spec.efforts[i]))
        }))
        .when(is_default, |d| d.child(badge("Default")))
        .when(unknown, |d| {
            d.child(
                div()
                    .id("effort-default")
                    .px(px(7.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .text_size(px(11.))
                    .cursor_pointer()
                    .when(on_default, |d| {
                        d.bg(colors::ink(0.09)).text_color(colors::text1())
                    })
                    .when(!on_default, |d| {
                        d.text_color(colors::text3())
                            .hover(|s| s.bg(colors::ink(0.05)))
                    })
                    .child("Default")
                    .on_click(cx.listener(|h: &mut H, _: &ClickEvent, _, cx| {
                        if let Some(m) = h.model_menu() {
                            m.slider.take();
                        }
                        h.choose(Choice::Effort(String::new()), cx);
                        cx.notify();
                    })),
            )
        });

    let slider = (n > 0).then(|| {
        let frac = move |i: usize| {
            if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.5
            }
        };
        let motion = s.knob.toward(now.map_or(0., frac));
        let rail = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(9.))
            .h(px(4.))
            .rounded_full()
            .bg(colors::ink(0.1));
        let stops = (0..n).map(|i| {
            let lit = now.is_some_and(|a| i <= a);
            div()
                .absolute()
                .top(px(8.))
                .left(relative(frac(i)))
                .ml(px(-3.))
                .size(px(6.))
                .rounded_full()
                .bg(if lit {
                    colors::accent()
                } else {
                    colors::ink(0.22)
                })
        });
        let fill = now.map(|_| {
            motion.apply(
                "effort-fill",
                div()
                    .absolute()
                    .left_0()
                    .top(px(9.))
                    .h(px(4.))
                    .rounded_full()
                    .bg(colors::accent()),
                |d, f| d.w(relative(f)),
            )
        });
        let knob = now.map(|_| {
            motion.apply(
                "effort-knob",
                div()
                    .absolute()
                    .top(px(3.))
                    .ml(px(-8.))
                    .size(px(16.))
                    .rounded_full()
                    .border_2()
                    .border_color(colors::accent())
                    .bg(colors::bg()),
                |d, f| d.left(relative(f)),
            )
        });
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
        let track = div()
            .relative()
            .h(px(22.))
            // the stops sit over the middle of each name below
            .mx(relative(0.5 / n as f32))
            .child(rail)
            .children(fill)
            .children(stops)
            .children(knob)
            .child(listen);
        let ticks = div()
            .flex()
            .children(spec.efforts.iter().enumerate().map(|(i, level)| {
                let here = now == Some(i);
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_size(px(10.5))
                    .text_color(if here {
                        colors::text1()
                    } else {
                        colors::text3()
                    })
                    .when(here, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(tick(level))
            }));
        div()
            .id("effort-slider")
            .flex()
            .flex_col()
            .gap(px(2.))
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
            .child(track)
            .child(ticks)
    });

    let window = spec.long.map(|long| {
        let pill = s.window.toward(if long { SEGMENT } else { 0. }).apply(
            "window-pill",
            div()
                .absolute()
                .top(px(2.))
                .left(px(2.))
                .w(px(SEGMENT))
                .h(px(24.))
                .rounded(px(6.))
                .bg(colors::ink(0.1)),
            |d, x| d.ml(px(x)),
        );
        let segment = |on: bool, label: &'static str| {
            div()
                .id(("window", on as usize))
                .w(px(SEGMENT))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
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
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text3())
                    .child("Context window"),
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
        .p(px(12.))
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
        let track = Bounds::new(point(px(100.), px(0.)), size(px(200.), px(20.)));
        assert_eq!(index_at(px(90.), track, 5), 0);
        assert_eq!(index_at(px(160.), track, 5), 1);
        assert_eq!(index_at(px(240.), track, 5), 3);
        assert_eq!(index_at(px(400.), track, 5), 4);
        assert_eq!(tick("xhigh"), "XHigh");
        assert_eq!(tick("high"), "High");
    }
}
