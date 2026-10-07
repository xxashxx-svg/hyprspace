// Settings, Usage, Limits: each plan's windows as how much is left, with when they reset.

use chrono::{Local, TimeZone};
use gpui::{AnyElement, Div, FontWeight, Hsla, IntoElement, div, prelude::*, px, relative};
use hyprspace_proto::usage::LiveExtra;

use super::meter::tone_color;
use super::model::{Block, Win};
use super::page::*;
use super::{Limits, brand};
use crate::assets::icon;
use crate::colors;
use crate::time::ago;

/// When a window resets, as a clock time: "11:20 PM" today, "Fri 9:00 AM" further out.
fn clock(ms: i64, now: i64) -> String {
    let Some(t) = Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    let time = t.format("%-I:%M %p").to_string();
    if ms - now < 20 * 3_600_000 {
        time
    } else {
        format!("{} {time}", t.format("%a"))
    }
}

/// The bar shows what is LEFT, in the agent's color with a firm edge where it ends.
fn track(left: f32, tint: Hsla, used: Option<String>, chip: Option<String>) -> Div {
    let edge = left > 0.0 && left < 100.0;
    div()
        .relative()
        .flex_1()
        .h(px(34.))
        .rounded(px(9.))
        .bg(colors::ink(0.05))
        .border_1()
        .border_color(colors::ink(0.06))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(relative(left.clamp(0.0, 100.0) / 100.0))
                .bg(tint.opacity(0.3))
                .when(edge, |d| d.border_r_2().border_color(tint)),
        )
        .children(used.map(|u| {
            div()
                .absolute()
                .left(px(12.))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(u)
        }))
        .children(chip.map(|c| {
            div()
                .absolute()
                .right(px(6.))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .h(px(22.))
                        .px(px(8.))
                        .rounded(px(6.))
                        .bg(colors::surface3())
                        .border_1()
                        .border_color(colors::border1())
                        .text_size(px(11.))
                        .text_color(colors::text2())
                        .child(icon("rotate-cw", 10., colors::text2()))
                        .child(c),
                )
        }))
}

/// A limit window: the numbers on the left, the bar on the right.
fn limit_row(w: &Win, tint: Hsla, now: i64) -> AnyElement {
    let gone = w.expired(now);
    let used = w.pct.round() as i64;
    let left = 100 - used;
    let tone = w.tone(now);
    let (fill, figure) = match tone_color(tone) {
        Some(c) => (c, c),
        None => (tint, colors::text1()),
    };
    let sub = if gone {
        "Window reset. Updates on the next turn.".to_string()
    } else if w.resets_at.is_some() && used > 0 {
        format!("+{used}% back in {}", w.reset_label(now))
    } else if w.resets_at.is_some() {
        format!("Full, resets in {}", w.reset_label(now))
    } else {
        format!("{used}% used")
    };
    let number = if gone {
        div()
            .text_size(px(26.))
            .line_height(px(30.))
            .text_color(colors::text3())
            .child("-")
    } else {
        big(format!("{left}%"), "left", figure)
    };
    div()
        .flex()
        .items_center()
        .gap(px(20.))
        .p(px(16.))
        .child(
            div()
                .flex_none()
                .w(px(170.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text2())
                        .child(w.label.clone()),
                )
                .child(number)
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(sub),
                ),
        )
        .child(track(
            if gone { 0.0 } else { left as f32 },
            fill,
            (!gone).then(|| format!("{used}% used")),
            w.resets_at.filter(|_| !gone).map(|r| clock(r, now)),
        ))
        .into_any_element()
}

fn extra_row(x: &LiveExtra, tint: Hsla) -> AnyElement {
    let cur = if x.currency.as_deref() == Some("USD") {
        "$"
    } else {
        ""
    };
    let pct = x.percent.round() as i64;
    div()
        .flex()
        .items_center()
        .gap(px(20.))
        .p(px(16.))
        .child(
            div()
                .flex_none()
                .w(px(170.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text2())
                        .child("Extra usage"),
                )
                .child(big(
                    format!("{cur}{:.2}", x.used),
                    &format!("of {cur}{:.2}", x.limit),
                    colors::text1(),
                ))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("This month"),
                ),
        )
        .child(track(pct as f32, tint, Some(format!("{pct}% used")), None))
        .into_any_element()
}

fn limit_card(b: &Block, note_text: Option<String>, now: i64) -> AnyElement {
    let id = b.agent.cli();
    let tint = brand(id);
    let age = b
        .updated_at
        .map(|at| match ago(at as u64, now as u64).as_str() {
            "now" => "Updated just now".to_string(),
            a => format!("Updated {a} ago"),
        });
    let mut items: Vec<AnyElement> = b.windows.iter().map(|w| limit_row(w, tint, now)).collect();
    if let Some(x) = &b.extra {
        items.push(extra_row(x, tint));
    }
    card(id, b.agent.name(), b.plan.clone(), age)
        .child(rows(items))
        .children(note_text.map(note))
        .into_any_element()
}

fn callout(who: &str, text: &str) -> AnyElement {
    let c = colors::busy();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(11.))
        .rounded(px(10.))
        .border_1()
        .border_color(c.opacity(0.35))
        .bg(c.opacity(0.11))
        .text_size(px(13.))
        .text_color(c)
        .child(icon("triangle-alert", 14., c))
        .child(
            div()
                .flex()
                .gap(px(4.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{who}:")),
                )
                .child(text.to_string()),
        )
        .into_any_element()
}

impl Limits {
    pub(super) fn limits_view(&self, now: i64) -> Vec<AnyElement> {
        let pic = self.readings.picture(now);
        let mut out = Vec::new();
        if let Some(m) = &pic.claude_missing {
            out.push(callout("Claude", m));
        }
        if let Some(m) = &pic.codex_missing {
            out.push(callout("Codex", m));
        }
        if let Some(b) = &pic.claude {
            let stale = pic
                .claude_stale
                .then(|| "No agent has reported in a while.".to_string());
            out.push(limit_card(b, b.note.clone().or(stale), now));
        }
        if let Some(b) = &pic.codex {
            out.push(limit_card(b, b.note.clone(), now));
        }
        let cards = pic.claude.is_some() || pic.codex.is_some();
        if !cards && out.is_empty() {
            out.push(
                div()
                    .py(px(36.))
                    .px(px(16.))
                    .rounded(px(12.))
                    .border_1()
                    .border_dashed()
                    .border_color(colors::border2())
                    .flex()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .child("No limits yet. They show up here once Claude or Codex is signed in on this machine.")
                    .into_any_element(),
            );
        }
        if cards {
            out.push(foot_text(
                "A bar turns amber or red when you're using it up faster than the window runs out. OpenCode and Grok don't report plan limits, so they only show under Activity.",
            ));
        }
        out
    }
}
