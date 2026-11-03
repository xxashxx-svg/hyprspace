// A run's calls and thinking as rows, each led by an icon for what it does. The step at work sits
// in a lit band with its words shimmering; when it ends the band fades and it settles into a plain
// row. Reads and searches in a row share one row that lists them while they happen and folds to a
// count once the agent moves on.

use std::f32::consts::TAU;
use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, ClickEvent, Context, ElementId, HighlightStyle, Hsla, SharedString, StyledText,
    Transformation, div, prelude::*, radians, relative,
};
use hyprspace_proto::Tool;
use hyprspace_theme::MONO;

use super::model::Item;
use super::render::{
    enter, fold_key, folding, settle_fold, took, tool_body, unfold, with_pictures,
};
use super::{TranscriptView, rows, tool};
use crate::assets::icon;
use crate::colors;
use crate::pace::Looping;
use crate::slide;
use crate::zoom::px;

const ROW: f32 = 24.;
const ICON: f32 = 16.;
const KID_ICON: f32 = 13.;
const WELL_LINES: usize = 4;
/// Taller than a full well, so growing its cap to this never clips it.
const WELL_H: f32 = 100.;
/// How long a step that just ended takes to settle: its band fades, its icon comes back.
const LEAVE: Duration = Duration::from_millis(450);
const LIGHT_UP: u64 = 260;
const SHINE: u64 = 1600;
const SPIN: u64 = 800;

enum Step {
    Explore(Range<usize>),
    One(usize),
}

/// Where a step is in its life, for its band and its icon.
#[derive(Clone, Copy, PartialEq)]
enum Lit {
    Off,
    On,
    /// Ended a moment ago, and still settling.
    Leaving,
}

fn explores(item: &Item) -> bool {
    matches!(
        item,
        Item::Tool {
            tool: Tool::Read { .. } | Tool::Search { .. },
            ..
        }
    )
}

fn steps(items: &[Item], start: usize, end: usize) -> Vec<Step> {
    let mut out = Vec::new();
    let mut ix = start;
    while ix < end {
        if matches!(items[ix], Item::Changes { .. }) {
            ix += 1;
            continue;
        }
        let n = items[ix..end].iter().take_while(|i| explores(i)).count();
        if n >= 2 {
            out.push(Step::Explore(ix..ix + n));
            ix += n;
        } else {
            out.push(Step::One(ix));
            ix += 1;
        }
    }
    out
}

/// The rows for the stretch of quiet items starting at `start`.
pub(super) fn stretch(
    v: &TranscriptView,
    start: usize,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let all = &v.model.items;
    let rows: Vec<AnyElement> = steps(all, start, rows::quiet_end(all, start))
        .into_iter()
        .map(|step| match step {
            Step::Explore(range) => {
                let ix = range.start;
                enter(v, ix, explore(v, range, cx))
            }
            Step::One(ix) => enter(v, ix, one(v, ix, cx)),
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .children(rows)
        .into_any_element()
}

fn glyph(t: &Tool) -> &'static str {
    match t {
        Tool::Command { .. } => "step-command",
        Tool::Read { .. } => "step-read",
        Tool::Edit { .. } => "step-edit",
        Tool::Search { .. } => "step-search",
        Tool::Web { .. } => "step-web",
        Tool::Agent { .. } => "step-agent",
        Tool::Mcp { .. } | Tool::Other { .. } => "step-tool",
    }
}

/// A read or a search inside an explore row: the file it opened, or the lens.
fn kid_glyph(t: &Tool) -> &'static str {
    match t {
        Tool::Read { .. } => "step-file",
        _ => glyph(t),
    }
}

fn blend(a: Hsla, b: Hsla, f: f32) -> Hsla {
    Hsla {
        h: a.h + (b.h - a.h) * f,
        s: a.s + (b.s - a.s) * f,
        l: a.l + (b.l - a.l) * f,
        a: a.a + (b.a - a.a) * f,
    }
}

/// The step's icon in `color`, which shrinks away for a spinner while it works and grows back as
/// it ends.
fn mark(key: ElementId, name: &'static str, size: f32, color: Hsla, lit: Lit) -> AnyElement {
    let on = lit == Lit::On;
    if lit == Lit::Off || !slide::animations() {
        return if on {
            icon("spin", size, colors::text1())
        } else {
            icon(name, size, color)
        }
        .into_any_element();
    }
    let ms = if on {
        LIGHT_UP
    } else {
        LEAVE.as_millis() as u64
    };
    let layer = |el: AnyElement, part: &'static str, shown: bool, from: f32| {
        let grow = slide::ease_in(
            div().flex_none().child(el),
            (key.clone(), part),
            ms,
            move |d, t| {
                let f = if shown { t } else { 1. - t };
                d.size(px(size * (from + (1. - from) * f))).opacity(f)
            },
        );
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(grow)
    };
    let spin = icon("spin", size, colors::text1())
        .size_full()
        .looping(SPIN, |s, t| {
            s.with_transformation(Transformation::rotate(radians(t * TAU)))
        })
        .into_any_element();
    let glyph = icon(name, size, color).size_full().into_any_element();
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(layer(
            glyph,
            if on { "glyph-out" } else { "glyph-in" },
            !on,
            0.4,
        ))
        .child(layer(
            spin,
            if on { "spin-in" } else { "spin-out" },
            on,
            0.5,
        ))
        .into_any_element()
}

/// `text` with a light passing over it while its step works. `from` and `of` place it in the
/// whole line, so the verb and what it acted on shine as one.
fn shine(text: SharedString, from: usize, of: usize) -> AnyElement {
    if !slide::animations() {
        return div()
            .text_color(colors::text1())
            .child(text)
            .into_any_element();
    }
    let (dim, lit) = (colors::text3(), colors::text1());
    let chars: Vec<Range<usize>> = text
        .char_indices()
        .map(|(i, c)| i..i + c.len_utf8())
        .collect();
    let of = of.max(1) as f32;
    StyledText::new(text)
        .looping(SHINE, move |s, phase| {
            let center = -0.3 + 1.6 * phase;
            s.with_highlights(chars.iter().enumerate().map(|(k, r)| {
                let d = ((from + k) as f32 / of - center).abs() / 0.22;
                let f = (1. - d).clamp(0., 1.);
                let color = blend(dim, lit, f * f * (3. - 2. * f));
                (
                    r.clone(),
                    HighlightStyle {
                        color: Some(color),
                        ..Default::default()
                    },
                )
            }))
        })
        .into_any_element()
}

/// The band a working step sits in, lit while it works and fading as it ends.
fn band(key: ElementId, lit: Lit, body: impl IntoElement) -> AnyElement {
    let base = div()
        .rounded(px(8.))
        .mx(px(-8.))
        .px(px(8.))
        .flex()
        .flex_col()
        .child(body);
    let paint = |d: gpui::Div, f: f32| d.bg(colors::ink(0.045 * f));
    match lit {
        Lit::Off => base.into_any_element(),
        _ if !slide::animations() => {
            paint(base, if lit == Lit::On { 1. } else { 0. }).into_any_element()
        }
        Lit::On => slide::ease_in(base, (key, "band-in"), LIGHT_UP, paint),
        Lit::Leaving => slide::ease_in(
            base,
            (key, "band-out"),
            LEAVE.as_millis() as u64,
            move |d, t| paint(d, 1. - t),
        ),
    }
}

/// A step's line: its icon, a muted verb, what it acted on, then `right`. Clicking it runs
/// `toggle` and flips the fold under `id`.
#[allow(clippy::too_many_arguments)]
fn line(
    id: ElementId,
    lead: AnyElement,
    verb: SharedString,
    what: Option<(SharedString, bool)>,
    working: bool,
    right: Option<AnyElement>,
    small: bool,
    cx: &mut Context<TranscriptView>,
    toggle: Option<impl Fn(&mut TranscriptView) + 'static>,
) -> AnyElement {
    let key = fold_key(cx, &id);
    let of = verb.chars().count() + what.as_ref().map_or(0, |(w, _)| w.chars().count());
    let skip = verb.chars().count();
    let verb = if working {
        shine(verb, 0, of)
    } else {
        div().child(verb).into_any_element()
    };
    let what = what.map(|(text, mono)| {
        let text = if working {
            shine(text, skip, of)
        } else {
            div().child(text).into_any_element()
        };
        div()
            .min_w_0()
            .truncate()
            .text_color(colors::text2())
            .when(mono, |d| {
                d.font_family(MONO)
                    .text_size(px(if small { 11.5 } else { 12. }))
            })
            .child(text)
    });
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(if small { 7. } else { 8. }))
        .min_h(px(if small { 21. } else { ROW }))
        .min_w_0()
        .text_size(px(if small { 12.5 } else { 13. }))
        .text_color(colors::text3())
        .child(lead)
        .child(div().flex_none().child(verb))
        .children(what)
        .child(div().flex_1())
        .children(right)
        .when_some(toggle, |d, toggle| {
            d.cursor_pointer()
                .hover(|s| s.text_color(colors::text2()))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    slide::flip(key.clone());
                    toggle(v);
                    settle_fold(cx);
                    cx.notify();
                }))
        })
        .into_any_element()
}

/// "Run cargo test" as ("Run", "cargo test").
fn split(label: &str) -> (SharedString, Option<SharedString>) {
    match label.split_once(' ') {
        Some((verb, rest)) => (verb.to_string().into(), Some(rest.to_string().into())),
        None => (label.to_string().into(), None),
    }
}

fn secs(d: Duration) -> Option<String> {
    (d.as_secs() >= 1).then(|| took(d.as_secs()))
}

fn one(v: &TranscriptView, ix: usize, cx: &mut Context<TranscriptView>) -> AnyElement {
    match &v.model.items[ix] {
        Item::Thinking { text, open } => {
            let open = *open;
            let working = v.model.running() && v.model.tail() == Some(ix);
            let id: ElementId = ("thinking", ix).into();
            let lit = if working { Lit::On } else { Lit::Off };
            let head = line(
                id.clone(),
                mark(
                    ("thought", ix).into(),
                    "step-think",
                    ICON,
                    colors::text3(),
                    lit,
                ),
                if working { "Thinking" } else { "Thought" }.into(),
                None,
                working,
                None,
                false,
                cx,
                Some(move |v: &mut TranscriptView| {
                    if let Some(Item::Thinking { open, .. }) = v.model.items.get_mut(ix) {
                        *open = !*open;
                    }
                }),
            );
            let body = (open || folding(cx, id.clone())).then(|| {
                unfold(
                    cx,
                    id,
                    open,
                    div()
                        .pl(px(ICON + 8.))
                        .pb(px(4.))
                        .text_size(px(12.5))
                        .line_height(relative(1.55))
                        .italic()
                        .text_color(colors::text3())
                        .child(text.trim().to_string()),
                )
            });
            div()
                .flex()
                .flex_col()
                .child(head)
                .children(body)
                .into_any_element()
        }
        Item::Tool { .. } => call(v, ix, false, cx),
        other => super::render::item(v, ix, other, cx),
    }
}

/// One call: its line, the output of a command at work, why a command failed, and what it
/// asked and said while open. A `small` one sits inside an explore row, with no band of its own.
fn call(
    v: &TranscriptView,
    ix: usize,
    small: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let Item::Tool {
        id,
        tool: t,
        done,
        open,
        images,
        live,
    } = &v.model.items[ix]
    else {
        return div().into_any_element();
    };
    let key: SharedString = format!("tool-{ix}").into();
    let timing = v.model.timing(id);
    let leaving = timing
        .and_then(|(since, took)| Some(since.elapsed().saturating_sub(took?)))
        .is_some_and(|after| after < LEAVE);
    let lit = match done {
        None => Lit::On,
        Some(_) if leaving => Lit::Leaving,
        Some(_) => Lit::Off,
    };
    let color = match done {
        Some((false, _)) => colors::error().opacity(0.6),
        _ => colors::text3(),
    };
    let (name, size) = if small {
        (kid_glyph(t), KID_ICON)
    } else {
        (glyph(t), ICON)
    };
    let right = call_right(
        ix,
        t,
        done.as_ref(),
        timing.map(|(since, took)| took.unwrap_or_else(|| since.elapsed())),
    );
    let (verb, what) = split(&tool::label(t));
    let mono = matches!(
        t,
        Tool::Command { .. } | Tool::Read { .. } | Tool::Edit { .. } | Tool::Search { .. }
    );
    let head = line(
        (key.clone(), 0usize).into(),
        mark(("mark", ix).into(), name, size, color, lit),
        verb,
        what.map(|w| (w, mono)),
        done.is_none(),
        right,
        small,
        cx,
        Some(move |v: &mut TranscriptView| {
            if let Some(Item::Tool { open, .. }) = v.model.items.get_mut(ix) {
                *open = !*open;
            }
        }),
    );
    let indent = size + 8.;
    // a command still running shows its newest lines; its first ones scrolled off long ago
    let shown = done
        .as_ref()
        .map_or(crate::clip::tail(live), |(_, out)| out.as_str());
    let body = tool_body(&key, t, shown, *open, indent, cx);
    let command = matches!(t, Tool::Command { .. });
    let well = match done {
        _ if !command || *open => None,
        None => tail(ix, live, false, indent),
        Some((_, out)) if leaving => tail(ix, out, true, indent),
        Some(_) => None,
    };
    let why = match done {
        Some((false, out)) if command && !*open => failure(ix, out, indent),
        _ => None,
    };
    let card = div()
        .flex()
        .flex_col()
        .child(head)
        .children(well)
        .children(why)
        .children(body);
    let card = if small {
        card.into_any_element()
    } else {
        band(key.clone().into(), lit, card)
    };
    with_pictures(card, &key, *open, images, cx)
}

fn call_right(
    ix: usize,
    t: &Tool,
    done: Option<&(bool, String)>,
    took: Option<Duration>,
) -> Option<AnyElement> {
    let time = took.and_then(secs);
    let counts = match t {
        Tool::Edit { changes } if done.is_some() => {
            let (a, d) = tool::counts(changes);
            Some(
                div()
                    .flex()
                    .gap_1()
                    .child(div().text_color(colors::diff_add()).child(format!("+{a}")))
                    .child(div().text_color(colors::diff_del()).child(format!("-{d}")))
                    .into_any_element(),
            )
        }
        _ => None,
    };
    let result = match done {
        None => None,
        Some((false, _)) => Some(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_color(colors::error())
                .child(icon("step-failed", 13., colors::error()))
                .child("Failed")
                .into_any_element(),
        ),
        Some(_) => None,
    };
    if counts.is_none() && result.is_none() && time.is_none() {
        return None;
    }
    let settle = |el: AnyElement, part: &'static str| {
        slide::ease_in(div().flex().child(el), (part, ix), 320, |d, t| d.opacity(t))
    };
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .children(counts.map(|c| settle(c, "counts")))
            .children(result.map(|r| settle(r, "result")))
            .children(time)
            .into_any_element(),
    )
}

/// The newest lines a running command printed, in a small well under its line that grows open,
/// or shrinks shut when `closing`.
fn tail(ix: usize, text: &str, closing: bool, indent: f32) -> Option<AnyElement> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let shown = &lines[lines.len().saturating_sub(WELL_LINES)..];
    let last = shown.len() - 1;
    let well = div()
        .ml(px(indent))
        .overflow_hidden()
        .px(px(11.))
        .py(px(7.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::ink(0.06))
        .bg(colors::ink(0.025))
        .flex()
        .flex_col()
        .font_family(MONO)
        .text_size(px(11.5))
        .line_height(relative(1.6))
        .children(shown.iter().enumerate().map(|(n, l)| {
            div()
                .truncate()
                .text_color(if n == last {
                    colors::text2()
                } else {
                    colors::text3()
                })
                .child(l.to_string())
        }));
    let shown = move |t: f32| if closing { 1. - t } else { t };
    let id: ElementId = (if closing { "well-out" } else { "well" }, ix).into();
    let ms = if closing {
        LEAVE.as_millis() as u64
    } else {
        320
    };
    Some(slide::ease_in(well, id, ms, move |d, t| {
        let s = shown(t);
        d.max_h(px(WELL_H * s))
            .mt(px(4. * s))
            .mb(px(6. * s))
            .opacity(s)
    }))
}

/// The line of a failed command's output that says what failed, else its last line.
fn failure(ix: usize, out: &str, indent: f32) -> Option<AnyElement> {
    let lines = || out.lines().map(str::trim).filter(|l| !l.is_empty());
    let said = lines()
        .find(|l| {
            let lower = l.to_lowercase();
            lower.contains("failed") || lower.starts_with("error") || lower.contains("panicked")
        })
        .or_else(|| lines().next_back())?;
    let line = div()
        .ml(px(indent))
        .overflow_hidden()
        .truncate()
        .font_family(MONO)
        .text_size(px(11.5))
        .text_color(colors::error().opacity(0.85))
        .child(said.to_string());
    Some(slide::ease_in(line, ("why", ix), 320, |d, t| {
        d.max_h(px(20. * t)).mb(px(4. * t)).opacity(t)
    }))
}

/// Reads and searches in a row: "Exploring" while they're the run's latest work, then their count,
/// and their calls under it while open.
fn explore(
    v: &TranscriptView,
    range: Range<usize>,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let start = range.start;
    let all = &v.model.items;
    let calls = &all[range.clone()];
    let active = v.model.running() && range.end == all.len();
    let tools = calls.iter().filter_map(|i| match i {
        Item::Tool { tool, .. } => Some(tool),
        _ => None,
    });
    let failed = calls
        .iter()
        .filter(|i| {
            matches!(
                i,
                Item::Tool {
                    done: Some((false, _)),
                    ..
                }
            )
        })
        .count();
    let id: ElementId = ("explore", start).into();
    let key = fold_key(cx, &id);
    let next = v.born.get(&range.end);
    // calls that arrive together never showed the list open, so there is nothing to fold away
    let shown_open = match (v.born.get(&(range.end - 1)), next) {
        (Some(last), Some(next)) => last.elapsed() > next.elapsed() + Duration::from_millis(400),
        _ => false,
    };
    let leaving = next.is_some_and(|at| at.elapsed() < LEAVE);
    // fold away as the next kind of step arrives, unless someone opened it
    if shown_open
        && leaving
        && !v.open_runs.contains(&start)
        && slide::flipped(&key, Duration::from_secs(1)).is_none()
    {
        slide::flip(key.clone());
        settle_fold(cx);
    }
    // grow open when a second read turns a single one into this group
    let formed = active
        && range.len() == 2
        && v.born
            .get(&(start + 1))
            .is_some_and(|at| at.elapsed() < Duration::from_millis(400));
    if formed && slide::flipped(&key, Duration::from_secs(1)).is_none() {
        slide::flip(key.clone());
        settle_fold(cx);
    }
    let open = active || v.open_runs.contains(&start);
    let (verb, what) = if active {
        let now = calls.iter().rev().find_map(|i| match i {
            Item::Tool { tool, .. } => Some(split(&tool::label(tool)).1),
            _ => None,
        });
        ("Exploring".into(), now.flatten().map(|w| (w, true)))
    } else {
        (tool::summary(tools).into(), None)
    };
    let right = (failed > 0).then(|| {
        div()
            .flex_none()
            .text_size(px(12.))
            .text_color(colors::error())
            .child(format!("{failed} failed"))
            .into_any_element()
    });
    let lit = if active {
        Lit::On
    } else if leaving {
        Lit::Leaving
    } else {
        Lit::Off
    };
    let color = if failed > 0 {
        colors::error().opacity(0.6)
    } else {
        colors::text3()
    };
    let head = line(
        id.clone(),
        mark(("explored", start).into(), "step-read", ICON, color, lit),
        verb,
        what,
        active,
        right,
        false,
        cx,
        (!active).then_some(move |v: &mut TranscriptView| {
            if !v.open_runs.remove(&start) {
                v.open_runs.insert(start);
            }
        }),
    );
    let body = (open || folding(cx, id.clone())).then(|| {
        let rows: Vec<AnyElement> = range
            .map(|ix| enter(v, ix, call(v, ix, true, cx)))
            .collect();
        unfold(
            cx,
            id,
            open,
            div()
                .pl(px(ICON + 8.))
                .pb(px(4.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(rows),
        )
    });
    band(
        ("explore-band", start).into(),
        lit,
        div().flex().flex_col().child(head).children(body),
    )
}
