// The intro: what HyprSpace is, which agents it found, how the app fits together, the defaults,
// and a first folder (the Tauri app's Onboarding.tsx and onboarding.css). It shows once, on a
// first run with no spaces; someone who already has spaces gets the flag set without seeing it.
// The command palette replays it. The Tauri app's sign-in and licensing steps are left behind.

mod sketches;
mod steps;

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Window, anchored, deferred, div,
    point, prelude::*, px,
};
use hyprspace_proto::Agent;

use crate::assets::icon;
use crate::root::Root;
use crate::{colors, widgets};

/// The exit, after the Tauri app's: the card sinks away while the backdrop closes in to a point
/// over the app, like an iris.
const SINK: Duration = Duration::from_millis(380);
const IRIS_AFTER: Duration = Duration::from_millis(120);
const IRIS: Duration = Duration::from_millis(600);

const STEPS: [&str; 6] = [
    "Welcome",
    "Your agents",
    "The basics",
    "The tools",
    "Defaults",
    "Start",
];

pub struct Intro {
    step: usize,
    /// The topic open in each tour.
    basics: usize,
    tools: usize,
    /// How many spaces there were when it opened, so picking a folder on the last step can
    /// tell that it worked.
    spaces: usize,
    copied: Option<Agent>,
    /// When the exit started.
    leaving: Option<Instant>,
}

impl Intro {
    pub fn new(spaces: usize) -> Self {
        Self {
            step: 0,
            basics: 0,
            tools: 0,
            spaces,
            copied: None,
            leaving: None,
        }
    }
}

impl Root {
    /// Opens the intro from the start, over everything.
    pub(crate) fn show_intro(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.intro = Some(Intro::new(self.local_spaces()));
        self.menu = None;
        cx.notify();
    }

    /// On a first load: a brand-new install sees the intro, anyone else gets the flag set.
    pub(crate) fn first_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.intro_seen {
            return;
        }
        if self.state.spaces.is_empty() && self.open_arg.is_none() {
            self.show_intro(window, cx);
        } else {
            self.state.intro_seen = true;
            self.save();
        }
    }

    /// Plays the exit, then takes the intro away. Seen counts from the start of it.
    fn finish_intro(&mut self, cx: &mut Context<Self>) {
        self.state.intro_seen = true;
        self.save();
        let Some(intro) = &mut self.intro else {
            return;
        };
        if !crate::slide::animations() {
            self.intro = None;
        } else if intro.leaving.is_none() {
            intro.leaving = Some(Instant::now());
            self._intro_exit = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(IRIS_AFTER + IRIS).await;
                let _ = this.update(cx, |r, cx| {
                    r.intro = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    fn intro_step(&mut self, step: usize, cx: &mut Context<Self>) {
        if let Some(i) = &mut self.intro {
            i.step = step.min(STEPS.len() - 1);
        }
        cx.notify();
    }

    pub(crate) fn intro_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let intro = self.intro.as_ref()?;
        // a folder picked on the last step: the space is there, so the intro is done
        if intro.leaving.is_none() && self.local_spaces() > intro.spaces {
            self.finish_intro(cx);
        }
        let intro = self.intro.as_ref()?;
        let step = intro.step;
        let leaving = intro.leaving.map(|at| at.elapsed());
        if leaving.is_some() {
            window.request_animation_frame();
        }
        let size = window.viewport_size();
        let body = match step {
            0 => self.intro_welcome(),
            1 => self.intro_agents(cx),
            2 => self.intro_tour(true, cx),
            3 => self.intro_tour(false, cx),
            4 => self.intro_defaults(window, cx),
            _ => self.intro_start(cx),
        };
        let nav = self.intro_nav(step, cx);
        let last = step == STEPS.len() - 1;
        let foot = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(36.))
            .py(px(14.))
            .border_t_1()
            .border_color(colors::border1())
            .when(step > 0, |d| {
                d.child(
                    widgets::button("intro-back", "Back")
                        .child(icon("arrow-left", 13., colors::text2()))
                        .flex_row_reverse()
                        .on_click(
                            cx.listener(move |r, _: &ClickEvent, _, cx| r.intro_step(step - 1, cx)),
                        ),
                )
            })
            .child(div().flex_1())
            .child(if last {
                widgets::button("intro-later", "I'll look around first")
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.finish_intro(cx)))
                    .into_any_element()
            } else {
                widgets::primary("intro-next", if step == 0 { "Get started" } else { "Next" })
                    .child(icon("arrow-right", 13., colors::on_accent()))
                    .on_click(
                        cx.listener(move |r, _: &ClickEvent, _, cx| r.intro_step(step + 1, cx)),
                    )
                    .into_any_element()
            });
        let main = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("intro-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .pt(px(30.))
                    .px(px(36.))
                    .pb(px(20.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text3())
                            .child(format!("STEP {} OF {}", step + 1, STEPS.len())),
                    )
                    .child(body),
            )
            .child(foot);
        // sinks: fades and drops, faster as it goes
        let sink = leaving.map_or(0., |d| {
            let t = (d.as_secs_f32() / SINK.as_secs_f32()).min(1.);
            t * t
        });
        let frame = div()
            .id("intro")
            .occlude()
            .relative()
            .top(px(18. * sink))
            .opacity(1. - sink)
            .flex()
            .w(px(980.).min(size.width - px(48.)))
            .h(px(660.).min(size.height - px(48.)))
            .rounded(px(16.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .overflow_hidden()
            .child(nav)
            .child(main);
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("intro-backdrop")
                        .occlude()
                        .relative()
                        .w(size.width)
                        .h(size.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|d| match leaving {
                            None => d.bg(colors::bg()),
                            Some(d_) => d.child(iris(d_, size)),
                        })
                        .child(frame),
                ),
            )
            .with_priority(3)
            .into_any_element(),
        )
    }

    fn intro_nav(&self, step: usize, cx: &mut Context<Self>) -> AnyElement {
        let steps = STEPS.iter().enumerate().map(|(i, name)| {
            let (on, done) = (i == step, i < step);
            let n = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.))
                .rounded_full()
                .border_1()
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .map(|d| {
                    if on {
                        d.border_color(colors::text1())
                            .bg(colors::text1())
                            .text_color(colors::surface2())
                            .child((i + 1).to_string())
                    } else if done {
                        d.border_color(gpui::transparent_black())
                            .bg(colors::ink(0.1))
                            .child(icon("check", 11., colors::ok()))
                    } else {
                        d.border_color(colors::border2()).child((i + 1).to_string())
                    }
                });
            div()
                .id(("intro-step", i))
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(34.))
                .px(px(8.))
                .rounded(px(8.))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(if on {
                    colors::text1()
                } else if done {
                    colors::text2()
                } else {
                    colors::text3()
                })
                .when(on, |d| d.bg(colors::ink(0.07)))
                .when(!on, |d| {
                    d.hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
                })
                .child(n)
                .child(*name)
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.intro_step(i, cx)))
        });
        div()
            .flex_none()
            .w(px(216.))
            .flex()
            .flex_col()
            .pt(px(20.))
            .px(px(12.))
            .pb(px(14.))
            .border_r_1()
            .border_color(colors::border1())
            .bg(colors::ink(0.02))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .px(px(8.))
                    .pb(px(18.))
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(icon("sparkles", 18., colors::accent()))
                    .child("HyprSpace"),
            )
            .child(div().flex().flex_col().gap(px(2.)).children(steps))
            .child(div().flex_1())
            .child(
                div()
                    .id("intro-skip")
                    .flex()
                    .items_center()
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(7.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
                    .child("Skip the intro")
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| r.finish_intro(cx))),
            )
            .into_any_element()
    }
}

/// The backdrop as a circle closing in on the middle of the window, `elapsed` into the exit. It
/// starts wide enough to cover the corners, and a faint accent ring rides its edge.
fn iris(elapsed: Duration, size: gpui::Size<gpui::Pixels>) -> AnyElement {
    let t = (elapsed.saturating_sub(IRIS_AFTER).as_secs_f32() / IRIS.as_secs_f32()).min(1.);
    // ease in and out
    let t = if t < 0.5 {
        4. * t * t * t
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    };
    let (w, h) = (f32::from(size.width), f32::from(size.height));
    let r = (w.hypot(h) / 2. + 2.) * (1. - t);
    div()
        .absolute()
        .left(px(w / 2. - r))
        .top(px(h / 2. - r))
        .size(px(2. * r))
        .rounded_full()
        .bg(colors::bg())
        .border_2()
        .border_color(colors::accent().opacity(0.45 * t.min(1. - t) * 2.))
        .into_any_element()
}
