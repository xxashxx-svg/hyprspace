// Activity's Overview and Models, after Claude Code's /stats: the period (the last 7 or 30 days,
// or everything the files go back to), a heatmap of the last nine months, the figures that sum a
// period up (favorite model, tokens, sessions, active days, streaks, the busiest day, the split
// between input, output and the cache), and every model's share with its own split. Claude's and
// Codex's figures together, from their own files (`ProviderUsage::daily_models`).

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration as Days, Local, NaiveDate};
use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, Hsla, div, prelude::*, px, relative,
};
use hyprspace_proto::Agent;
use hyprspace_proto::usage::{ModelUsage, ProviderUsage};

use super::Limits;
use super::activity::pretty_model;
use super::chart::shade;
use super::page::{grouped, short};
use crate::colors;
use crate::root::Root;
use crate::widgets;

/// How far back the figures look.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Period {
    Week,
    #[default]
    Month,
    All,
}

/// The heatmap's weeks, ending this one, and its cells.
const WEEKS: usize = 39;
const CELL: f32 = 12.;
const GAP: f32 = 3.;
/// How long "everything" can stretch, at most.
const ALL_DAYS: i64 = 365;
const MODELS_SHOWN: usize = 8;

const AGENTS: [(&str, Agent); 2] = [("claude", Agent::Claude), ("codex", Agent::Codex)];

fn signed_in(l: &Limits) -> impl Iterator<Item = (&ProviderUsage, Agent)> {
    AGENTS
        .iter()
        .filter_map(|(id, a)| l.local.get(*id).filter(|u| u.signed_in).map(|u| (u, *a)))
}

fn parse(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// The days a period covers, oldest first, ending today. Everything starts on the first day
/// any file knows of, and covers a month at least.
pub fn period_days(l: &Limits) -> Vec<NaiveDate> {
    let today = Local::now().date_naive();
    let n = match l.chart.period {
        Period::Week => 7,
        Period::Month => 30,
        Period::All => {
            let first = signed_in(l)
                .flat_map(|(u, _)| {
                    u.daily_models
                        .iter()
                        .map(|d| d.date.as_str())
                        .chain(u.day_sessions.iter().map(|d| d.date.as_str()))
                })
                .filter_map(parse)
                .min()
                .unwrap_or(today);
            ((today - first).num_days() + 1).clamp(30, ALL_DAYS)
        }
    };
    (0..n).map(|i| today - Days::days(n - 1 - i)).collect()
}

/// Input, output and the cache, over a stretch.
#[derive(Clone, Copy, Default)]
struct Split {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    total: u64,
}

impl Split {
    fn add(&mut self, o: Split) {
        self.input += o.input;
        self.output += o.output;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
        self.total += o.total;
    }
}

/// `total` split the way `m` splits over all time. A day from Claude's stats file has a total
/// but no split of its own (the transcripts that had it are gone after 30 days), so it takes
/// the model's lifetime one.
fn split_like(m: &ModelUsage, total: u64) -> Split {
    let all = (m.input_tokens + m.output_tokens + m.cache_tokens).max(1) as u128;
    let part = |n: u64| (n as u128 * total as u128 / all) as u64;
    Split {
        input: part(m.input_tokens),
        output: part(m.output_tokens),
        cache_read: part(m.cache_tokens.saturating_sub(m.cache_write_tokens)),
        cache_write: part(m.cache_write_tokens),
        total,
    }
}

struct Model {
    name: String,
    color: Hsla,
    split: Split,
}

/// Everything the cards show, read out of `Limits` before they are built.
pub struct Snapshot {
    period: Period,
    days: Vec<NaiveDate>,
    /// Tokens on each day of the period, and on each day of the heatmap.
    per_day: Vec<u64>,
    heat: BTreeMap<NaiveDate, u64>,
    sessions: u64,
    split: Split,
    models: Vec<Model>,
}

impl Snapshot {
    pub fn new(l: &Limits) -> Self {
        let days = period_days(l);
        let first = days[0];
        let index: HashMap<NaiveDate, usize> =
            days.iter().enumerate().map(|(i, d)| (*d, i)).collect();
        let mut per_day = vec![0u64; days.len()];
        let mut heat: BTreeMap<NaiveDate, u64> = BTreeMap::new();
        let mut sessions = 0;
        let mut split = Split::default();
        let mut models = Vec::new();
        for (u, agent) in signed_in(l) {
            let lifetime: HashMap<&str, &ModelUsage> = u
                .models
                .iter()
                .filter(|m| m.input_tokens + m.output_tokens + m.cache_tokens > 0)
                .map(|m| (m.model.as_str(), m))
                .collect();
            let mut mine: BTreeMap<&str, Split> = BTreeMap::new();
            for d in &u.daily_models {
                let Some(date) = parse(&d.date) else {
                    continue;
                };
                *heat.entry(date).or_default() += d.total;
                if let Some(&i) = index.get(&date) {
                    per_day[i] += d.total;
                    let known = Split {
                        input: d.input,
                        output: d.output,
                        cache_read: d.cache_read,
                        cache_write: d.cache_write,
                        total: d.total,
                    };
                    let parts = d.input + d.output + d.cache_read + d.cache_write;
                    let s = match lifetime.get(d.model.as_str()) {
                        Some(m) if parts != d.total => split_like(m, d.total),
                        _ => known,
                    };
                    mine.entry(&d.model).or_default().add(s);
                }
            }
            sessions += u
                .day_sessions
                .iter()
                .filter(|d| parse(&d.date).is_some_and(|d| d >= first))
                .map(|d| d.count)
                .sum::<u64>();
            let mut list: Vec<(&str, Split)> = mine.into_iter().collect();
            list.sort_by_key(|(_, s)| std::cmp::Reverse(s.total));
            for (k, (model, s)) in list.into_iter().enumerate() {
                split.add(s);
                models.push(Model {
                    name: pretty_model(model),
                    color: shade(agent, k),
                    split: s,
                });
            }
        }
        models.sort_by_key(|m| std::cmp::Reverse(m.split.total));
        Self {
            period: l.chart.period,
            days,
            per_day,
            heat,
            sessions,
            split,
            models,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.heat.values().all(|v| *v == 0)
    }
}

/// The longest run of active days, and the run that reaches today (or yesterday, when today
/// hasn't started yet).
fn streaks(active: &[bool]) -> (usize, usize) {
    let (mut best, mut run) = (0, 0);
    for a in active {
        run = if *a { run + 1 } else { 0 };
        best = best.max(run);
    }
    let mut tail = active.iter().rev();
    if active.last() == Some(&false) {
        tail.next();
    }
    let current = tail.take_while(|a| **a).count();
    (best, current)
}

fn day_label(d: NaiveDate) -> String {
    format!("{} {}", d.format("%b"), d.day())
}

/// The four steps of the heatmap's color, from the busy days' quarters.
fn levels(values: &[u64]) -> [u64; 3] {
    let mut busy: Vec<u64> = values.iter().copied().filter(|v| *v > 0).collect();
    if busy.is_empty() {
        return [1, 1, 1];
    }
    busy.sort_unstable();
    let at = |q: f32| busy[((busy.len() - 1) as f32 * q) as usize];
    [at(0.25), at(0.5), at(0.75)]
}

fn shade_of(v: u64, steps: &[u64; 3]) -> Hsla {
    let accent = colors::accent();
    match v {
        0 => colors::ink(0.06),
        v if v <= steps[0] => accent.opacity(0.3),
        v if v <= steps[1] => accent.opacity(0.5),
        v if v <= steps[2] => accent.opacity(0.75),
        _ => accent,
    }
}

fn heatmap(s: &Snapshot) -> AnyElement {
    let today = Local::now().date_naive();
    // columns are weeks, Sunday on top, the last one holding today
    let end = today + Days::days(6 - today.weekday().num_days_from_sunday() as i64);
    let start = end - Days::days(WEEKS as i64 * 7 - 1);
    let values: Vec<u64> = (0..WEEKS * 7)
        .map(|i| {
            let d = start + Days::days(i as i64);
            s.heat.get(&d).copied().unwrap_or(0)
        })
        .collect();
    let steps = levels(&values);
    let step = CELL + GAP;
    let months = (0..WEEKS).filter_map(|w| {
        let d = start + Days::days(w as i64 * 7);
        let prev = d - Days::days(7);
        (w == 0 || d.month() != prev.month()).then(|| {
            div()
                .absolute()
                .left(px(w as f32 * step))
                .text_size(px(10.5))
                .text_color(colors::text3())
                .child(d.format("%b").to_string())
        })
    });
    let values = &values;
    let week = |w: usize| {
        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .children((0..7).map(move |r| {
                let d = start + Days::days((w * 7 + r) as i64);
                if d > today {
                    return div().size(px(CELL)).into_any_element();
                }
                let v = values[w * 7 + r];
                let label = if v == 0 {
                    format!("{}, nothing", day_label(d))
                } else {
                    format!("{}, {} tokens", day_label(d), short(v))
                };
                div()
                    .id(("heat", w * 7 + r))
                    .size(px(CELL))
                    .rounded(px(3.))
                    .bg(shade_of(v, &steps))
                    .hover(|st| st.border_1().border_color(colors::text2()))
                    .tooltip(widgets::tip_text(label))
                    .into_any_element()
            }))
    };
    let weekday = |r: usize, name: &'static str| {
        div()
            .absolute()
            .top(px(r as f32 * step - 1.))
            .text_size(px(10.5))
            .text_color(colors::text3())
            .child(name)
    };
    let swatch = |c: Hsla| div().size(px(CELL)).rounded(px(3.)).bg(c);
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().relative().h(px(14.)).ml(px(30.)).children(months))
        .child(
            div()
                .flex()
                .child(
                    div()
                        .relative()
                        .w(px(30.))
                        .child(weekday(1, "Mon"))
                        .child(weekday(3, "Wed"))
                        .child(weekday(5, "Fri")),
                )
                .child(div().flex().gap(px(GAP)).children((0..WEEKS).map(week))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .ml(px(30.))
                .mt(px(4.))
                .text_size(px(10.5))
                .text_color(colors::text3())
                .child("Less")
                .child(swatch(colors::ink(0.06)))
                .child(swatch(colors::accent().opacity(0.3)))
                .child(swatch(colors::accent().opacity(0.5)))
                .child(swatch(colors::accent().opacity(0.75)))
                .child(swatch(colors::accent()))
                .child("More"),
        )
        .into_any_element()
}

/// One figure in the grid: a quiet label, then its value.
fn figure(label: &'static str, value: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .w(px(110.))
                .text_size(px(12.5))
                .text_color(colors::text3())
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(value),
        )
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// The Overview card: the period, the heatmap and the figures.
pub fn overview(s: &Snapshot, limits: &Entity<Limits>, cx: &mut Context<Root>) -> AnyElement {
    let active: Vec<bool> = s.per_day.iter().map(|v| *v > 0).collect();
    let (longest, current) = streaks(&active);
    let active_days = active.iter().filter(|a| **a).count();
    // everything counts from the first active day, as Claude's own does
    let span = match s.period {
        Period::All => active
            .iter()
            .position(|a| *a)
            .map_or(0, |i| active.len() - i),
        _ => active.len(),
    };
    let busiest = s
        .per_day
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| **v)
        .filter(|(_, v)| **v > 0)
        .map(|(i, v)| (s.days[i], *v));
    let favorite = s.models.first();
    let period = widgets::segments().children(
        [
            (Period::Week, "7 days"),
            (Period::Month, "30 days"),
            (Period::All, "All time"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (p, name))| {
            let l = limits.clone();
            widgets::segment(("period", i), None, name, s.period == p, false).on_click(cx.listener(
                move |_, _: &ClickEvent, _, cx| {
                    l.update(cx, |l, cx| {
                        l.chart.period = p;
                        l.chart.hover = None;
                        cx.notify();
                    });
                },
            ))
        }),
    );
    let split = s.split;
    let breakdown = [
        ("Input", split.input),
        ("Output", split.output),
        ("Cache read", split.cache_read),
        ("Cache write", split.cache_write),
    ]
    .into_iter()
    .filter(|(_, v)| *v > 0)
    .map(|(name, v)| format!("{name} {}", short(v)))
    .collect::<Vec<_>>()
    .join("  \u{b7}  ");
    let grid = div()
        .grid()
        .grid_cols(2)
        .gap_x(px(24.))
        .gap_y(px(12.))
        .child(figure(
            "Favorite model",
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .children(
                    favorite.map(|m| div().flex_none().size(px(8.)).rounded_full().bg(m.color)),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(favorite.map_or("None yet".to_string(), |m| m.name.clone())),
                ),
        ))
        .child(figure("Total tokens", short(split.total)))
        .child(figure("Sessions", grouped(s.sessions)))
        .child(figure(
            "Active days",
            div()
                .flex()
                .items_end()
                .gap(px(3.))
                .child(active_days.to_string())
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(colors::text3())
                        .child(format!("of {span}")),
                ),
        ))
        .child(figure("Longest streak", plural(longest, "day")))
        .child(figure("Current streak", plural(current, "day")))
        .child(figure(
            "Most active day",
            busiest.map_or("None yet".to_string(), |(d, _)| day_label(d)),
        ))
        .child(figure(
            "Daily average",
            short(split.total / active_days.max(1) as u64),
        ));
    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .p(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::surface2())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child("Overview"),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child("Claude and Codex together, cache included"),
                        ),
                )
                .child(period),
        )
        .child(heatmap(s))
        .child(div().h(px(1.)).bg(colors::border1()))
        .child(grid)
        .when(!breakdown.is_empty(), |d| {
            d.child(
                div()
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .child(breakdown),
            )
        })
        .into_any_element()
}

/// The Models card: each model's share of the period, with its own split.
pub fn models(s: &Snapshot) -> Option<AnyElement> {
    if s.models.is_empty() {
        return None;
    }
    let all: u64 = s.models.iter().map(|m| m.split.total).sum::<u64>().max(1);
    let block = |m: &Model| {
        let share = m.split.total as f64 * 100. / all as f64;
        let line = |parts: Vec<(&str, u64)>| {
            parts
                .into_iter()
                .filter(|(_, v)| *v > 0)
                .map(|(name, v)| format!("{name} {}", short(v)))
                .collect::<Vec<_>>()
                .join("  \u{b7}  ")
        };
        let io = line(vec![("In", m.split.input), ("Out", m.split.output)]);
        let cache = line(vec![
            ("Cache read", m.split.cache_read),
            ("Cache write", m.split.cache_write),
        ]);
        div()
            .flex()
            .flex_col()
            .gap(px(5.))
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(div().flex_none().size(px(8.)).rounded_full().bg(m.color))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(m.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child(format!("{share:.1}%")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(short(m.split.total)),
                    ),
            )
            .child(
                div().h(px(4.)).rounded_full().bg(colors::ink(0.06)).child(
                    div()
                        .h_full()
                        .rounded_full()
                        .w(relative((share / 100.) as f32))
                        .bg(m.color),
                ),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .when(!io.is_empty(), |d| d.child(io))
                    .when(!cache.is_empty(), |d| d.child(cache)),
            )
    };
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(16.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child("Models"),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_x(px(24.))
                    .gap_y(px(16.))
                    .children(s.models.iter().take(MODELS_SHOWN).map(block)),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaks_count_runs_and_the_one_reaching_today() {
        let t = true;
        let f = false;
        assert_eq!(streaks(&[t, t, f, t, t, t, f]), (3, 3));
        assert_eq!(streaks(&[t, t, f, t, t, t, t]), (4, 4));
        assert_eq!(streaks(&[t, f, f]), (1, 0));
        assert_eq!(streaks(&[]), (0, 0));
    }

    #[test]
    fn the_heatmap_steps_by_the_busy_days_quarters() {
        let steps = levels(&[0, 10, 20, 30, 40, 0]);
        assert_eq!(steps, [10, 20, 30]);
        assert_eq!(levels(&[0, 0]), [1, 1, 1]);
    }

    #[test]
    fn a_day_without_a_split_takes_the_models_lifetime_one() {
        let m = ModelUsage {
            input_tokens: 10,
            output_tokens: 90,
            cache_tokens: 900,
            cache_write_tokens: 100,
            ..Default::default()
        };
        let s = split_like(&m, 2000);
        assert_eq!(
            (s.input, s.output, s.cache_read, s.cache_write, s.total),
            (20, 180, 1600, 200, 2000)
        );
    }
}
