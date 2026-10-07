// Activity's chart, after T3 Code's analytics: tokens per day over the last 30 days, a line for
// each agent or each model, named where it ends, on a linear or a log scale (one busy day
// otherwise flattens every quiet one). The pointer picks a day and a card lists its figures; the
// legend under it hides and shows lines and gives each one's total and share.
// Everything comes from the agents' own files (`ProviderUsage::daily_models`).

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration as Days, Local, NaiveDate};
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, DispatchPhase, Entity, FontWeight,
    HitboxBehavior, Hsla, IntoElement, MouseMoveEvent, PathBuilder, Pixels, Point, SharedString,
    TextAlign, TextRun, Window, canvas, div, fill, point, prelude::*, px, size,
};
use hyprspace_proto::Agent;
use hyprspace_proto::usage::ProviderUsage;

use super::Limits;
use super::page::short;
use crate::colors;
use crate::root::Root;
use crate::widgets;

/// Days on the chart, ending today.
const N: usize = 30;
const HEIGHT: f32 = 240.;
/// Room left of the plot for the scale, under it for the dates, and right of it for the names.
const LEFT: f32 = 44.;
const BOTTOM: f32 = 26.;
const RIGHT: f32 = 128.;
const TOP: f32 = 10.;
/// Each agent's models get their own lines up to this many; the rest share one.
const MODELS_EACH: usize = 4;

/// A line per agent, or per model.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum By {
    #[default]
    Agent,
    Model,
}

/// What the chart remembers between frames, on `Limits`.
#[derive(Default)]
pub struct State {
    pub by: By,
    /// A log scale, a power of ten to each step.
    pub log: bool,
    pub hidden: HashSet<String>,
    /// The day under the pointer.
    pub hover: Option<usize>,
}

#[derive(Clone)]
struct Series {
    key: String,
    name: String,
    color: Hsla,
    values: Vec<u64>,
    total: u64,
}

/// What the chart draws, read out of `Limits` before anything is built.
pub struct Data {
    by: By,
    log: bool,
    hover: Option<usize>,
    hidden: HashSet<String>,
    days: Vec<NaiveDate>,
    series: Vec<Series>,
}

fn agent_of(id: &str) -> Option<Agent> {
    match id {
        "claude" => Some(Agent::Claude),
        "codex" => Some(Agent::Codex),
        _ => None,
    }
}

/// An agent's k-th line: its brand color, its gradient's second stop, then deeper takes on both.
fn shade(agent: Agent, k: usize) -> Hsla {
    let (a, b) = colors::brand(agent);
    let base = if k.is_multiple_of(2) { a } else { b };
    if k < 2 {
        base
    } else {
        Hsla {
            l: base.l * 0.72,
            ..base
        }
    }
}

/// A round top for the scale: 1, 2, 2.5 or 5 times a power of ten.
fn nice(v: u64) -> u64 {
    if v <= 1 {
        return 1;
    }
    let p = 10u64.pow((v as f64).log10().floor() as u32);
    for f in [1., 2., 2.5, 5., 10.] {
        let top = (f * p as f64).round() as u64;
        if top >= v {
            return top;
        }
    }
    10 * p
}

impl Data {
    pub fn new(l: &Limits) -> Self {
        let today = Local::now().date_naive();
        let days: Vec<NaiveDate> = (0..N)
            .map(|i| today - Days::days((N - 1 - i) as i64))
            .collect();
        let index: HashMap<String, usize> = days
            .iter()
            .enumerate()
            .map(|(i, d)| (d.format("%Y-%m-%d").to_string(), i))
            .collect();
        let mut series = Vec::new();
        for id in ["claude", "codex"] {
            let Some(u) = l.local.get(id).filter(|u| u.signed_in) else {
                continue;
            };
            let Some(agent) = agent_of(id) else {
                continue;
            };
            series.extend(lines(u, agent, l.chart.by, &index));
        }
        series.retain(|s| s.total > 0);
        Self {
            by: l.chart.by,
            log: l.chart.log,
            hover: l.chart.hover,
            hidden: l.chart.hidden.clone(),
            days,
            series,
        }
    }
}

/// One provider's lines: the whole agent, or its busiest models and the rest together.
fn lines(u: &ProviderUsage, agent: Agent, by: By, index: &HashMap<String, usize>) -> Vec<Series> {
    let mut per: HashMap<&str, Vec<u64>> = HashMap::new();
    for d in &u.daily_models {
        if let Some(&i) = index.get(&d.date) {
            per.entry(&d.model).or_insert_with(|| vec![0; N])[i] += d.tokens;
        }
    }
    let sum = |v: &Vec<u64>| v.iter().sum::<u64>();
    match by {
        By::Agent => {
            let mut values = vec![0; N];
            for v in per.values() {
                for (i, n) in v.iter().enumerate() {
                    values[i] += n;
                }
            }
            vec![Series {
                key: u.id.clone(),
                name: u.label.clone(),
                color: shade(agent, 0),
                total: sum(&values),
                values,
            }]
        }
        By::Model => {
            let mut models: Vec<(&str, Vec<u64>)> = per.into_iter().collect();
            models.sort_by_key(|(_, v)| std::cmp::Reverse(sum(v)));
            let rest: Vec<(&str, Vec<u64>)> = models.split_off(models.len().min(MODELS_EACH));
            let mut out: Vec<Series> = models
                .into_iter()
                .enumerate()
                .map(|(k, (model, values))| Series {
                    key: format!("{}:{model}", u.id),
                    name: super::activity::pretty_model(model),
                    color: shade(agent, k),
                    total: sum(&values),
                    values,
                })
                .collect();
            if !rest.is_empty() {
                let mut values = vec![0; N];
                for (_, v) in &rest {
                    for (i, n) in v.iter().enumerate() {
                        values[i] += n;
                    }
                }
                out.push(Series {
                    key: format!("{}:other", u.id),
                    name: format!("Other {}", agent.name()),
                    color: colors::text3(),
                    total: sum(&values),
                    values,
                });
            }
            out
        }
    }
}

fn day_label(d: NaiveDate) -> String {
    format!("{} {}", d.format("%b"), d.day())
}

/// A line of text in the UI font, shaped for painting.
fn text(s: String, size_px: f32, color: Hsla, bold: bool, window: &mut Window) -> gpui::ShapedLine {
    let mut font = window.text_style().font();
    if bold {
        font.weight = FontWeight::SEMIBOLD;
    }
    let run = TextRun {
        len: s.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(s.into(), px(size_px), &[run], None)
}

/// Paints the plot: the scale, the dates, a line per series named where it ends, and the day
/// under the pointer with its figures.
/// Where a value sits up the scale, 0 to 1, and the steps to mark with their values. Linear
/// steps in quarters of a round top; log steps a power of ten each, from the top down four, with
/// the lowest step on the floor.
fn scale(max: u64, log: bool) -> (Box<dyn Fn(u64) -> f32>, Vec<u64>) {
    if log {
        let top_pow = (max.max(10) as f64).log10().ceil() as u32;
        let (top, low) = (
            10f64.powi(top_pow as i32),
            10f64.powi(top_pow.saturating_sub(4) as i32),
        );
        let at = move |v: u64| {
            ((v as f64).max(low).ln() - low.ln()) as f32 / (top.ln() - low.ln()) as f32
        };
        let steps = (top_pow.saturating_sub(4)..=top_pow)
            .map(|k| 10u64.pow(k))
            .collect();
        (Box::new(at), steps)
    } else {
        let top = nice(max.max(1));
        let at = move |v: u64| v as f32 / top as f32;
        (Box::new(at), (0..=4).map(|k| top * k / 4).collect())
    }
}

fn paint(
    bounds: Bounds<Pixels>,
    days: &[NaiveDate],
    shown: &[Series],
    hover: Option<usize>,
    log: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let plot = Bounds::from_corners(
        point(bounds.left() + px(LEFT), bounds.top() + px(TOP)),
        point(bounds.right() - px(RIGHT), bounds.bottom() - px(BOTTOM)),
    );
    let max = shown
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .max()
        .unwrap_or(0);
    let (at, steps) = scale(max, log);
    let x = |i: usize| plot.left() + plot.size.width * (i as f32 / (N - 1) as f32);
    let y = |v: u64| plot.bottom() - plot.size.height * at(v);
    let lh = px(14.);

    // the scale, the grid under each step and the floor
    window.paint_quad(fill(
        Bounds::new(
            point(plot.left(), plot.bottom()),
            size(plot.size.width, px(1.)),
        ),
        colors::ink(0.12),
    ));
    for v in steps {
        let gy = y(v);
        if v > 0 {
            window.paint_quad(fill(
                Bounds::new(point(plot.left(), gy), size(plot.size.width, px(1.))),
                colors::ink(0.05),
            ));
        }
        let label = text(short(v), 10.5, colors::text3(), false, window);
        let at = point(plot.left() - px(8.) - label.width(), gy - lh / 2.);
        let _ = label.paint(at, lh, TextAlign::Left, None, window, cx);
    }
    // the dates: the first, the middle and today
    for (i, name) in [
        (0, day_label(days[0])),
        (N / 2, day_label(days[N / 2])),
        (N - 1, "Today".to_string()),
    ] {
        let label = text(name, 10.5, colors::text3(), false, window);
        let lx = match i {
            0 => x(i),
            i if i == N - 1 => x(i) - label.width(),
            _ => x(i) - label.width() / 2.,
        };
        let _ = label.paint(
            point(lx, plot.bottom() + px(8.)),
            lh,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
    // the day under the pointer, behind the lines
    if let Some(h) = hover {
        window.paint_quad(fill(
            Bounds::new(point(x(h), plot.top()), size(px(1.), plot.size.height)),
            colors::ink(0.18),
        ));
    }
    // the lines, quietest first so the busiest sits on top. A day with no use breaks a line
    // rather than dragging it to the floor and back; a day on its own is its dot.
    let mut order: Vec<&Series> = shown.iter().collect();
    order.sort_by_key(|s| s.total);
    for s in &order {
        for run in s
            .values
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .split(|(_, v)| **v == 0)
            .filter(|r| r.len() > 1)
        {
            let mut b = PathBuilder::stroke(px(2.));
            for (k, (i, v)) in run.iter().enumerate() {
                let p = point(x(*i), y(**v));
                if k == 0 { b.move_to(p) } else { b.line_to(p) }
            }
            if let Ok(path) = b.build() {
                window.paint_path(path, s.color);
            }
        }
        for (i, v) in s.values.iter().enumerate() {
            if *v == 0 {
                continue;
            }
            let r = if hover == Some(i) { px(4.) } else { px(2.5) };
            let c = point(x(i), y(*v));
            window.paint_quad(
                fill(
                    Bounds::new(point(c.x - r, c.y - r), size(r * 2., r * 2.)),
                    s.color,
                )
                .corner_radii(r),
            );
        }
    }
    // names where the lines end, at their last day of use, nudged apart, with a hairline back
    let mut ends: Vec<(Pixels, &Series)> = shown
        .iter()
        .map(|s| {
            let last = s
                .values
                .iter()
                .rev()
                .find(|v| **v > 0)
                .copied()
                .unwrap_or(0);
            (y(last), s)
        })
        .collect();
    ends.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let gap = px(17.);
    let mut placed: Vec<Pixels> = Vec::new();
    for (ey, _) in &ends {
        let want = (*ey).max(plot.top() + gap / 2.);
        let at = match placed.last() {
            Some(prev) if want < *prev + gap => *prev + gap,
            _ => want,
        };
        placed.push(at);
    }
    // pushed past the bottom: walk them back up
    let floor = plot.bottom();
    for i in (0..placed.len()).rev() {
        let limit = if i + 1 < placed.len() {
            placed[i + 1] - gap
        } else {
            floor
        };
        placed[i] = placed[i].min(limit);
    }
    let lx = plot.right() + px(18.);
    for ((ey, s), ly) in ends.iter().zip(placed) {
        let mut b = PathBuilder::stroke(px(1.));
        b.move_to(point(plot.right() + px(3.), *ey));
        b.line_to(point(lx - px(4.), ly));
        if let Ok(path) = b.build() {
            window.paint_path(path, colors::ink(0.14));
        }
        let name = text(s.name.clone(), 11.5, s.color, true, window);
        let _ = name.paint(
            point(lx, ly - lh / 2.),
            lh,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
    // the day's figures, beside the line it marks
    if let Some(h) = hover {
        let mut rows: Vec<&Series> = shown.iter().filter(|s| s.values[h] > 0).collect();
        rows.sort_by_key(|s| std::cmp::Reverse(s.values[h]));
        let head = text(day_label(days[h]), 11., colors::text1(), true, window);
        let lines: Vec<(Hsla, gpui::ShapedLine, gpui::ShapedLine)> = if rows.is_empty() {
            vec![(
                colors::text3(),
                text("Nothing".into(), 11., colors::text3(), false, window),
                text(String::new(), 11., colors::text1(), true, window),
            )]
        } else {
            rows.iter()
                .map(|s| {
                    (
                        s.color,
                        text(s.name.clone(), 11., colors::text2(), false, window),
                        text(short(s.values[h]), 11., colors::text1(), true, window),
                    )
                })
                .collect()
        };
        let row_h = px(17.);
        let name_w = lines
            .iter()
            .map(|(_, n, _)| n.width())
            .fold(head.width(), |a, b| a.max(b));
        let value_w = lines
            .iter()
            .map(|(_, _, v)| v.width())
            .fold(px(0.), |a, b| a.max(b));
        let w = px(10.) + px(12.) + name_w + px(14.) + value_w + px(10.);
        let hgt = px(8.) + row_h + row_h * lines.len() as f32 + px(6.);
        let left = if x(h) + px(12.) + w > bounds.right() {
            x(h) - px(12.) - w
        } else {
            x(h) + px(12.)
        };
        let card = Bounds::new(point(left, plot.top() + px(4.)), size(w, hgt));
        window.paint_quad(
            fill(card, colors::surface3())
                .corner_radii(px(6.))
                .border_widths(px(1.))
                .border_color(colors::border2()),
        );
        let mut at: Point<Pixels> = point(card.left() + px(10.), card.top() + px(8.));
        let _ = head.paint(at, row_h, TextAlign::Left, None, window, cx);
        at.y += row_h;
        for (c, name, value) in &lines {
            let dot = px(6.);
            window.paint_quad(
                fill(
                    Bounds::new(point(at.x, at.y + (row_h - dot) / 2.), size(dot, dot)),
                    *c,
                )
                .corner_radii(dot / 2.),
            );
            let _ = name.paint(
                point(at.x + px(12.), at.y),
                row_h,
                TextAlign::Left,
                None,
                window,
                cx,
            );
            let vx = card.right() - px(10.) - value.width();
            let _ = value.paint(point(vx, at.y), row_h, TextAlign::Left, None, window, cx);
            at.y += row_h;
        }
    }
}

/// The chart's card, or nothing while no agent has figures for the last 30 days.
pub fn render(d: Data, limits: &Entity<Limits>, cx: &mut Context<Root>) -> Option<AnyElement> {
    if d.series.is_empty() {
        return None;
    }
    let shown: Vec<Series> = d
        .series
        .iter()
        .filter(|s| !d.hidden.contains(&s.key))
        .cloned()
        .collect();
    let all: u64 = shown.iter().map(|s| s.total).sum::<u64>().max(1);
    let head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .pt(px(14.))
        .pb(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child("Tokens per day"),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("In and out, the last 30 days"),
                ),
        )
        .child(
            widgets::segments().children(
                [(false, "Linear"), (true, "Log")]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (log, name))| {
                        let l = limits.clone();
                        widgets::segment(("chart-scale", i), None, name, d.log == log, false)
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                l.update(cx, |l, cx| {
                                    l.chart.log = log;
                                    cx.notify();
                                });
                            }))
                    }),
            ),
        )
        .child(
            widgets::segments().children(
                [(By::Agent, "By agent"), (By::Model, "By model")]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (by, name))| {
                        let l = limits.clone();
                        widgets::segment(("chart-by", i), None, name, d.by == by, false).on_click(
                            cx.listener(move |_, _: &ClickEvent, _, cx| {
                                l.update(cx, |l, cx| {
                                    l.chart.by = by;
                                    l.chart.hover = None;
                                    cx.notify();
                                });
                            }),
                        )
                    }),
            ),
        );
    let weak = limits.downgrade();
    let (days, hover, log) = (d.days.clone(), d.hover, d.log);
    let drawn = shown.clone();
    let plot = canvas(
        |bounds, window, _| (bounds, window.insert_hitbox(bounds, HitboxBehavior::Normal)),
        move |_, (bounds, hitbox), window, cx| {
            paint(bounds, &days, &drawn, hover, log, window, cx);
            let left = bounds.left() + px(LEFT);
            let width = bounds.size.width - px(LEFT + RIGHT);
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let day = hitbox.is_hovered(window).then(|| {
                    let t = ((e.position.x - left) / width).clamp(0., 1.);
                    (t * (N - 1) as f32).round() as usize
                });
                let _ = weak.update(cx, |l, cx| {
                    if l.chart.hover != day {
                        l.chart.hover = day;
                        cx.notify();
                    }
                });
            });
        },
    )
    .w_full()
    .h(px(HEIGHT));
    let legend = div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .px(px(16.))
        .pt(px(6.))
        .pb(px(14.))
        .children(d.series.iter().enumerate().map(|(i, s)| {
            let off = d.hidden.contains(&s.key);
            let key = s.key.clone();
            let l = limits.clone();
            let share = if off {
                String::new()
            } else {
                format!("{}%", (s.total as f64 * 100. / all as f64).round())
            };
            div()
                .id(("chart-legend", i))
                .flex()
                .items_center()
                .gap(px(7.))
                .h(px(28.))
                .px(px(10.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border1())
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|st| st.bg(colors::ink(0.04)))
                .when(off, |d| d.opacity(0.45))
                .tooltip(widgets::tip(if off {
                    "Show this line"
                } else {
                    "Hide this line"
                }))
                .child(div().size(px(8.)).rounded_full().bg(s.color))
                .child(
                    div()
                        .text_color(colors::text1())
                        .child(SharedString::from(s.name.clone())),
                )
                .child(div().text_color(colors::text3()).child(short(s.total)))
                .when(!share.is_empty(), |d| {
                    d.child(div().text_color(colors::text3()).child(share))
                })
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    let key = key.clone();
                    l.update(cx, |l, cx| {
                        if !l.chart.hidden.remove(&key) {
                            l.chart.hidden.insert(key);
                        }
                        cx.notify();
                    });
                }))
        }));
    Some(
        div()
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .child(head)
            .child(div().px(px(8.)).child(plot))
            .child(legend)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_tops_out_on_a_round_number() {
        assert_eq!(nice(0), 1);
        assert_eq!(nice(7), 10);
        assert_eq!(nice(12), 20);
        assert_eq!(nice(230), 250);
        assert_eq!(nice(18_500_000), 20_000_000);
        assert_eq!(nice(5_000), 5_000);
    }

    #[test]
    fn a_log_scale_steps_by_powers_of_ten() {
        let (at, steps) = scale(18_500_000, true);
        assert_eq!(steps, [10_000, 100_000, 1_000_000, 10_000_000, 100_000_000]);
        assert_eq!(at(0), 0.);
        assert!((at(100_000_000) - 1.).abs() < 1e-6);
        // a quiet day stays well off the floor next to a busy one, and less than the low step
        // sits on the floor
        assert!(at(500_000) > 0.4);
        assert_eq!(at(500), 0.);
        let (lin, steps) = scale(18_500_000, false);
        assert_eq!(steps.last(), Some(&20_000_000));
        assert!(lin(500_000) < 0.03);
    }
}
