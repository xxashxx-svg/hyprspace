// The ring in the bar above the panes and the popover under it (the Tauri app's UsageMeter.tsx
// and usage.css). The ring follows the most urgent window across every provider; the popover
// shows each provider's windows, with tabs when Claude and Codex both report.

use std::f32::consts::{PI, TAU};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, PathBuilder, Render, Window,
    canvas, div, fill, point, prelude::*, px, size,
};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

use super::model::{Block, Tone, Win};
use super::{Limits, brand};
use crate::assets::mark;
use crate::time::now_ms;
use crate::{colors, widgets};

pub(super) fn tone_color(tone: Tone) -> Option<Hsla> {
    match tone {
        Tone::Calm => None,
        Tone::Warn => Some(colors::busy()),
        Tone::Crit => Some(colors::error()),
    }
}

/// A 16px ring, filled clockwise from the top to `pct`.
pub(crate) fn ring(pct: f32, color: Hsla) -> impl IntoElement {
    let track = colors::ink(0.22);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let c = bounds.center();
            let (r, w) = (6.6, 2.4);
            let at = |a: f32| point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
            let arc = |from: f32, to: f32| {
                let steps = (((to - from) / TAU) * 64.0).ceil().max(2.0) as usize;
                let mut b = PathBuilder::stroke(px(w));
                for i in 0..=steps {
                    let p = at(from + (to - from) * i as f32 / steps as f32);
                    if i == 0 { b.move_to(p) } else { b.line_to(p) }
                }
                b.build().ok()
            };
            if let Some(path) = arc(0.0, TAU) {
                window.paint_path(path, track);
            }
            let pct = pct.clamp(0.0, 100.0);
            if pct <= 0.0 {
                return;
            }
            let start = -PI / 2.0;
            let end = start + TAU * pct / 100.0;
            if let Some(path) = arc(start, end) {
                window.paint_path(path, color);
            }
            // round caps, as the Tauri app's stroke-linecap
            for a in [start, end] {
                let p = at(a);
                let half = px(w / 2.0);
                let cap = gpui::Bounds::new(point(p.x - half, p.y - half), size(px(w), px(w)));
                window.paint_quad(fill(cap, color).corner_radii(half));
            }
        },
    )
    .size(px(16.))
    .flex_none()
}

impl Render for Limits {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = now_ms() as i64;
        let pic = self.readings.picture(now);
        if pic.worst(now).is_none() && !pic.any_window() {
            return div().into_any_element();
        }
        let worst = pic.worst(now);
        let tone = worst.map_or(Tone::Calm, |w| w.tone(now));
        let pct = worst.map_or(0.0, |w| w.pct as f32);
        let faded = worst.is_none() || pic.claude_stale;
        let open = self.open.is_some();
        let chip = div()
            .id("usage-ring")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(open, |d| d.bg(colors::surface2()))
            .hover(|s| s.bg(colors::surface2()))
            .when(faded, |d| d.opacity(0.5))
            .child(ring(pct, tone_color(tone).unwrap_or_else(colors::text1)))
            .on_click(cx.listener(|l, e: &ClickEvent, _, cx| {
                l.open = match l.open {
                    Some(_) => None,
                    None => Some(e.position()),
                };
                cx.notify();
            }));
        let pop = self.open.map(|at| {
            let close = cx.listener(|l, _: &(), _, cx| {
                l.open = None;
                cx.notify();
            });
            // right-aligned under the ring
            let at = point(at.x - px(262.), at.y + px(18.));
            widgets::layer(
                at,
                widgets::Open::Down,
                window,
                move |w, cx| close(&(), w, cx),
                self.popover(&pic, now, cx),
            )
        });
        div()
            .flex()
            .flex_none()
            .child(chip)
            .children(pop)
            .into_any_element()
    }
}

impl Limits {
    fn popover(&self, pic: &super::model::Picture, now: i64, cx: &mut Context<Self>) -> AnyElement {
        let both = pic.claude.is_some() && pic.codex.is_some();
        let tab = if pic.claude.is_none() {
            Agent::Codex
        } else if pic.codex.is_none() {
            Agent::Claude
        } else {
            self.tab
        };
        let mut body = div()
            .id("usage-pop")
            .w(px(272.))
            .py(px(4.))
            .flex()
            .flex_col()
            .rounded(px(11.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow());
        if both {
            let tabs = [pic.claude.as_ref(), pic.codex.as_ref()]
                .into_iter()
                .flatten()
                .map(|b| {
                    let on = b.agent == tab;
                    let agent = b.agent;
                    div()
                        .id(("usage-tab", agent as usize))
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.))
                        .h(px(26.))
                        .rounded(px(6.))
                        .text_size(px(12.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(colors::surface3()).text_color(colors::text1()))
                        .when(!on, |d| {
                            d.text_color(colors::text3())
                                .hover(|s| s.text_color(colors::text1()))
                        })
                        .child(div().when(!on, |d| d.opacity(0.6)).child(mark(
                            agent,
                            12.,
                            brand(agent.cli()),
                        )))
                        .child(agent.name())
                        .on_click(cx.listener(move |l, _: &ClickEvent, _, cx| {
                            l.tab = agent;
                            cx.notify();
                        }))
                });
            body = body.child(
                div()
                    .flex()
                    .gap(px(3.))
                    .mx(px(8.))
                    .mt(px(6.))
                    .mb(px(4.))
                    .p(px(3.))
                    .rounded(px(8.))
                    .bg(colors::ink(0.05))
                    .children(tabs),
            );
        }
        let shown = if tab == Agent::Claude {
            pic.claude.as_ref()
        } else {
            pic.codex.as_ref()
        };
        if let Some(b) = shown {
            body = body.child(section(b, both, now));
            if b.agent == Agent::Claude && pic.claude_stale {
                body = body.child(
                    div()
                        .px(px(12.))
                        .pb(px(8.))
                        .text_size(px(11.))
                        .text_color(colors::text3())
                        .child("No agent has reported in a while."),
                );
            }
        }
        body.into_any_element()
    }
}

/// One provider: a header, then a row per window. Under a tab the tab names it, so the header
/// keeps only the plan.
fn section(b: &Block, tabbed: bool, now: i64) -> AnyElement {
    let plan = b.plan.clone().map(|p| {
        div()
            .when(!tabbed, |d| d.ml_auto())
            .max_w(px(150.))
            .truncate()
            .font_family(MONO)
            .text_size(px(10.))
            .text_color(colors::text3())
            .child(p)
    });
    let head = if tabbed {
        plan.map(|p| div().flex().items_center().h(px(18.)).mb(px(8.)).child(p))
    } else {
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(24.))
                .mb(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .child(mark(b.agent, 14., brand(b.agent.cli()))),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child(b.agent.name()),
                )
                .children(plan),
        )
    };
    let mut rows = div().flex().flex_col().gap(px(13.));
    for w in &b.windows {
        rows = rows.child(window_row(w, brand(b.agent.cli()), now));
    }
    if let Some(x) = &b.extra {
        let cur = if x.currency.as_deref() == Some("USD") {
            "$"
        } else {
            ""
        };
        rows = rows.child(row(
            "Extra usage",
            format!("{}%", x.percent.round()),
            None,
            x.percent as f32,
            brand(b.agent.cli()),
            format!("{cur}{:.2} of {cur}{:.2} this month", x.used, x.limit),
        ));
    }
    if let Some(note) = &b.note {
        rows = rows.child(foot(note.clone()));
    }
    div()
        .pt(px(8.))
        .px(px(12.))
        .pb(px(12.))
        .children(head)
        .child(rows)
        .into_any_element()
}

fn window_row(w: &Win, brand: Hsla, now: i64) -> AnyElement {
    let gone = w.expired(now);
    let tone = w.tone(now);
    let value = if gone {
        "-".to_string()
    } else {
        format!("{}%", w.pct.round())
    };
    let value_color = if gone {
        Some(colors::text3())
    } else {
        tone_color(tone)
    };
    let foot = if gone {
        "window reset, updates next turn".to_string()
    } else if w.resets_at.is_some() {
        format!("resets in {}", w.reset_label(now))
    } else {
        String::new()
    };
    let bar = tone_color(tone).unwrap_or(brand);
    row(
        &w.label,
        value,
        value_color,
        if gone { 0.0 } else { w.pct as f32 },
        bar,
        foot,
    )
}

fn row(
    label: &str,
    value: String,
    value_color: Option<Hsla>,
    pct: f32,
    bar: Hsla,
    note: String,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(colors::text2())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(MONO)
                        .text_size(px(16.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(value_color.unwrap_or_else(colors::text1))
                        .child(value),
                ),
        )
        .child(
            div().h(px(5.)).rounded_full().bg(colors::ink(0.07)).child(
                div()
                    .h_full()
                    .w(gpui::relative(pct.clamp(0.0, 100.0) / 100.0))
                    .rounded_full()
                    .bg(bar),
            ),
        )
        .when(!note.is_empty(), |d| d.child(foot(note)))
        .into_any_element()
}

fn foot(text: String) -> impl IntoElement {
    div()
        .font_family(MONO)
        .text_size(px(10.))
        .text_color(colors::text3())
        .child(text)
}
