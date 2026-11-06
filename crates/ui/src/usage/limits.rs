// Settings, Usage, Limits: each plan's windows as how much is used, with when they reset.

use chrono::{Local, TimeZone};
use gpui::{AnyElement, FontWeight, Hsla, IntoElement, div, prelude::*};
use hyprspace_proto::usage::LiveExtra;

use super::meter::tone_color;
use super::model::{Block, Win};
use super::page::*;
use super::{Limits, brand};
use crate::assets::icon;
use crate::colors;
use crate::time::ago;
use crate::zoom::px;

fn resets(ms: i64, now: i64) -> String {
    let Some(t) = Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    let time = t.format("%-I:%M %p");
    if ms - now < 20 * 3_600_000 {
        format!("Resets at {time}")
    } else {
        format!("Resets {} {time}", t.format("%A"))
    }
}

pub(super) struct Line {
    pub label: String,
    pub sub: String,
    pub pct: f32,
    pub fill: Hsla,
    pub value: String,
}

impl Line {
    fn window(w: &Win, tint: Hsla, now: i64) -> Self {
        let gone = w.expired(now);
        let used = w.pct.round() as i64;
        Line {
            label: w.label.clone(),
            sub: match w.resets_at {
                _ if gone => "Just reset".into(),
                Some(r) => resets(r, now),
                None => String::new(),
            },
            pct: if gone { 0. } else { used as f32 },
            fill: tone_color(w.tone(now)).unwrap_or(tint),
            value: if gone {
                "-".into()
            } else {
                format!("{used}% used")
            },
        }
    }

    fn extra(x: &LiveExtra, tint: Hsla) -> Self {
        let cur = if x.currency.as_deref() == Some("USD") {
            "$"
        } else {
            ""
        };
        Line {
            label: "Extra usage".into(),
            sub: "This month".into(),
            pct: x.percent as f32,
            fill: tint,
            value: format!("{cur}{:.2} of {cur}{:.2}", x.used, x.limit),
        }
    }
}

pub(super) fn lines(b: &Block, now: i64) -> Vec<Line> {
    let tint = brand(b.agent.cli());
    b.windows
        .iter()
        .map(|w| Line::window(w, tint, now))
        .chain(b.extra.iter().map(|x| Line::extra(x, tint)))
        .collect()
}

fn row(l: &Line) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(24.))
        .p(px(16.))
        .child(
            div()
                .flex_none()
                .w(px(200.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(l.label.clone()),
                )
                .when(!l.sub.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child(l.sub.clone()),
                    )
                }),
        )
        .child(bar(l.pct, l.fill).flex_1())
        .child(
            div()
                .flex_none()
                .w(px(110.))
                .flex()
                .justify_end()
                .text_size(px(12.5))
                .text_color(colors::text2())
                .child(l.value.clone()),
        )
        .into_any_element()
}

fn limit_card(b: &Block, note_text: Option<String>, now: i64) -> AnyElement {
    let id = b.agent.cli();
    let age = b
        .updated_at
        .map(|at| match ago(at as u64, now as u64).as_str() {
            "now" => "Updated just now".to_string(),
            a => format!("Updated {a} ago"),
        });
    let items = lines(b, now).iter().map(row).collect();
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
        .px(px(16.))
        .py(px(12.))
        .rounded(px(12.))
        .bg(colors::ink(0.035))
        .text_size(px(13.))
        .text_color(colors::text2())
        .child(icon("triangle-alert", 14., c))
        .child(
            div()
                .flex()
                .gap(px(4.))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
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
                    .child("No limits yet. Sign in to Claude or Codex on this machine.")
                    .into_any_element(),
            );
        }
        if cards {
            out.push(foot_text(
                "Amber or red means you'll run out before it resets. OpenCode and Grok report no limits.",
            ));
        }
        out
    }
}
