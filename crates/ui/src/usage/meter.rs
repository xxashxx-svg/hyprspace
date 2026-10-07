// The ring in the top bar and the popover under it (the Tauri app's UsageMeter.tsx
// and usage.css). The ring follows the most urgent window across every provider; the popover
// shows each provider's windows as Settings draws them, under a strip drawn like the dock's
// tabs: one tab per provider when Claude and Codex both report, and the plan on its right.

use std::f32::consts::{PI, TAU};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, IntoElement, PathBuilder, Render, Window,
    canvas, div, fill, point, prelude::*, px, size,
};
use hyprspace_proto::Agent;

use super::limits::{extra_row, limit_row};
use super::model::{Block, Tone};
use super::page::{note, rows};
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
            // right-aligned under the ring, clear of the title row it sits in
            let at = point(
                at.x - px(WIDTH - 10.),
                px(crate::root::titlebar::HEIGHT + 6.),
            );
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
        let shown = if tab == Agent::Claude {
            pic.claude.as_ref()
        } else {
            pic.codex.as_ref()
        };
        // the strip: a tab per provider, as the dock's Files and Git, or the one provider's name
        let tabs: Vec<AnyElement> = [pic.claude.as_ref(), pic.codex.as_ref()]
            .into_iter()
            .flatten()
            .map(|b| {
                let on = b.agent == tab || !both;
                let agent = b.agent;
                div()
                    .id(("usage-tab", agent as usize))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if on { colors::text1() } else { colors::text3() })
                    .when(on && both, |d| d.bg(colors::surface3()))
                    .when(!on, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
                            .on_click(cx.listener(move |l, _: &ClickEvent, _, cx| {
                                l.tab = agent;
                                cx.notify();
                            }))
                    })
                    .child(div().when(!on, |d| d.opacity(0.6)).child(mark(
                        agent,
                        13.,
                        brand(agent.cli()),
                    )))
                    .child(agent.name())
                    .into_any_element()
            })
            .filter(|_| shown.is_some())
            .collect();
        let plan = shown.and_then(|b| b.plan.clone()).map(|p| {
            div()
                .flex_none()
                .max_w(px(120.))
                .truncate()
                .px(px(7.))
                .py(px(1.))
                .rounded_full()
                .bg(colors::ink(0.06))
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text2())
                .child(p)
        });
        let strip = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .h(px(40.))
            .px(px(6.))
            .pr(px(10.))
            .border_b_1()
            .border_color(colors::border1())
            .children(tabs)
            .child(div().flex_1())
            .children(plan);
        let stale = shown.is_some_and(|b| b.agent == Agent::Claude) && pic.claude_stale;
        div()
            .id("usage-pop")
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .child(strip)
            .children(shown.map(|b| section(b, stale, now)))
            .into_any_element()
    }
}

/// The popover's width.
const WIDTH: f32 = 320.;

/// One provider's windows, a row each. The strip above names the provider and its plan.
fn section(b: &Block, stale: bool, now: i64) -> AnyElement {
    let tint = brand(b.agent.cli());
    let mut items: Vec<AnyElement> = b
        .windows
        .iter()
        .map(|w| limit_row(w, tint, now, false))
        .collect();
    if let Some(x) = &b.extra {
        items.push(extra_row(x, tint, false));
    }
    let said = b
        .note
        .clone()
        .or_else(|| stale.then(|| "No agent has reported in a while.".to_string()));
    div()
        .flex()
        .flex_col()
        .child(rows(items))
        .children(said.map(note))
        .into_any_element()
}
