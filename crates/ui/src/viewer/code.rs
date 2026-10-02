// A file as the viewer draws it: one row per line, a line-number gutter, syntax colors from the
// theme's terminal palette (so each theme and side keeps its own), and the line a terminal
// pointed at lit with its column marked.

use std::ops::Range;

use gpui::{
    AnyElement, Context, FontStyle, FontWeight, HighlightStyle, Hsla, IntoElement,
    ListHorizontalSizingBehavior, SharedString, StyledText, UniformListScrollHandle, div,
    prelude::*, px, uniform_list,
};
use hyprspace_syntax::{Kind, Line};
use hyprspace_theme::MONO;

use super::{Body, Viewer};
use crate::colors;

pub const ROW: f32 = 19.;
const TAB: usize = 4;

pub struct Code {
    lines: Vec<SharedString>,
    /// The original text of lines that had tabs, to move their colored ranges when they come.
    tabbed: Vec<(usize, String)>,
    spans: Vec<Line>,
    /// 0-based line and 1-based column to light up.
    target: Option<(usize, Option<u32>)>,
    widest: usize,
}

/// Tabs become spaces (GPUI draws a tab as nothing), and the colored ranges move with them.
fn expand(line: &str, spans: &[(Range<usize>, Kind)]) -> (String, Line) {
    if !line.contains('\t') {
        return (line.to_string(), spans.to_vec());
    }
    let mut out = String::with_capacity(line.len() + 8);
    // new offset of every old byte offset, end included
    let mut at = Vec::with_capacity(line.len() + 1);
    let mut col = 0;
    for (i, ch) in line.char_indices() {
        while at.len() <= i {
            at.push(out.len());
        }
        if ch == '\t' {
            let n = TAB - col % TAB;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    while at.len() <= line.len() {
        at.push(out.len());
    }
    let spans = spans
        .iter()
        .map(|(r, k)| (at[r.start]..at[r.end], *k))
        .collect();
    (out, spans)
}

fn color(kind: Kind) -> HighlightStyle {
    let ansi = |i: usize| colors::hsla(colors::theme().ansi[i]);
    let plain = |c: Hsla| HighlightStyle {
        color: Some(c),
        ..Default::default()
    };
    match kind {
        Kind::Comment => HighlightStyle {
            color: Some(colors::text3()),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        Kind::Keyword => plain(ansi(5)),
        Kind::String | Kind::Code => plain(ansi(2)),
        Kind::Escape | Kind::Type => plain(ansi(6)),
        Kind::Number | Kind::Constant | Kind::Attribute => plain(ansi(3)),
        Kind::Function | Kind::Macro => plain(ansi(4)),
        Kind::Tag => plain(ansi(1)),
        Kind::Link => plain(colors::link()),
        Kind::Heading => HighlightStyle {
            color: Some(ansi(4)),
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        Kind::Emphasis => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        Kind::Operator | Kind::Punctuation => plain(colors::text2()),
        Kind::Property | Kind::Variable => plain(colors::text1()),
    }
}

impl Code {
    pub fn new(text: &str, line: Option<u32>, col: Option<u32>) -> Self {
        let mut tabbed = Vec::new();
        let lines: Vec<SharedString> = text
            .split('\n')
            .enumerate()
            .map(|(i, l)| {
                let l = l.strip_suffix('\r').unwrap_or(l);
                if l.contains('\t') {
                    tabbed.push((i, l.to_string()));
                }
                expand(l, &[]).0.into()
            })
            .collect();
        let widest = lines
            .iter()
            .enumerate()
            .max_by_key(|(_, l)| l.chars().count())
            .map_or(0, |(i, _)| i);
        let mut code = Self {
            lines,
            tabbed,
            spans: Vec::new(),
            target: None,
            widest,
        };
        code.target(line, col);
        code
    }

    /// `line` is 1-based, as terminals and editors print it.
    pub fn target(&mut self, line: Option<u32>, col: Option<u32>) {
        self.target = line.filter(|l| *l >= 1).map(|l| {
            (
                (l as usize - 1).min(self.lines.len().saturating_sub(1)),
                col,
            )
        });
    }

    pub fn target_line(&self) -> Option<usize> {
        self.target.map(|(l, _)| l)
    }

    /// Takes the highlighter's ranges for the raw text and fits them to the drawn lines.
    pub fn color(&mut self, mut spans: Vec<Line>) {
        for (i, raw) in &self.tabbed {
            if let Some(s) = spans.get_mut(*i) {
                *s = expand(raw, s).1;
            }
        }
        self.spans = spans;
    }

    fn row(&self, ix: usize, gutter: f32) -> AnyElement {
        let text = self.lines[ix].clone();
        let mut styles: Vec<(Range<usize>, HighlightStyle)> = self
            .spans
            .get(ix)
            .map(|s| s.iter().map(|(r, k)| (r.clone(), color(*k))).collect())
            .unwrap_or_default();
        let hit = self.target.filter(|(l, _)| *l == ix);
        if let Some((_, Some(col))) = hit {
            // mark the column the path named, one character wide
            if let Some((start, ch)) = text.char_indices().nth(col.saturating_sub(1) as usize) {
                let mark = HighlightStyle {
                    background_color: Some(colors::accent().opacity(0.45)),
                    ..Default::default()
                };
                styles = overlay(styles, start..start + ch.len_utf8(), mark);
            }
        }
        div()
            .flex()
            .min_w_full()
            .h(px(ROW))
            .when(hit.is_some(), |d| d.bg(colors::accent().opacity(0.12)))
            .child(
                div()
                    .flex_none()
                    .w(px(gutter))
                    .pr(px(14.))
                    .text_right()
                    .text_color(if hit.is_some() {
                        colors::text1()
                    } else {
                        colors::text3().opacity(0.7)
                    })
                    .child(SharedString::from((ix + 1).to_string())),
            )
            .child(
                div()
                    .flex_none()
                    .pr(px(24.))
                    .whitespace_nowrap()
                    .text_color(colors::text1())
                    .child(StyledText::new(text).with_highlights(styles)),
            )
            .into_any_element()
    }

    pub fn render(&self, scroll: UniformListScrollHandle, cx: &mut Context<Viewer>) -> AnyElement {
        let digits = self.lines.len().to_string().len().max(3);
        let gutter = digits as f32 * 7.3 + 22.;
        uniform_list(
            "code",
            self.lines.len(),
            cx.processor(move |v, range: Range<usize>, _, _| match &v.body {
                Body::Code(c) => range.map(|ix| c.row(ix, gutter)).collect(),
                _ => Vec::new(),
            }),
        )
        .size_full()
        .py(px(8.))
        .font_family(MONO)
        .text_size(px(12.))
        .line_height(px(ROW))
        .track_scroll(&scroll)
        .with_width_from_item(Some(self.widest))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .into_any_element()
    }
}

/// Lays `style` over `at`, cutting whatever ranges it covers so the result stays sorted and
/// non-overlapping, as `StyledText` wants.
fn overlay(
    styles: Vec<(Range<usize>, HighlightStyle)>,
    at: Range<usize>,
    style: HighlightStyle,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut out = Vec::with_capacity(styles.len() + 2);
    let mut base = HighlightStyle::default();
    for (r, s) in styles {
        if r.end <= at.start || r.start >= at.end {
            out.push((r, s));
            continue;
        }
        if r.start < at.start {
            out.push((r.start..at.start, s));
        }
        base = s;
        if r.end > at.end {
            out.push((at.end..r.end, s));
        }
    }
    out.push((at, base.highlight(style)));
    out.sort_by_key(|(r, _)| r.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_and_ranges_follow() {
        let (s, spans) = expand("\tfn\ta", &[(1..3, Kind::Keyword), (4..5, Kind::Variable)]);
        assert_eq!(s, "    fn  a");
        assert_eq!(spans, [(4..6, Kind::Keyword), (8..9, Kind::Variable)]);
        let (plain, same) = expand("ab", &[(0..1, Kind::Keyword)]);
        assert_eq!((plain.as_str(), same.len()), ("ab", 1));
    }

    #[test]
    fn targets_are_one_based_and_clamped() {
        let mut c = Code::new("a\nb\r\nc", Some(2), Some(1));
        assert_eq!(c.target_line(), Some(1));
        assert_eq!(c.lines[1].as_ref(), "b");
        c.target(Some(99), None);
        assert_eq!(c.target_line(), Some(2));
        c.target(Some(0), None);
        assert_eq!(c.target_line(), None);
    }

    #[test]
    fn overlay_cuts_what_it_covers() {
        let s = HighlightStyle::default();
        let got = overlay(vec![(0..10, s)], 4..5, s);
        let ranges: Vec<_> = got.into_iter().map(|(r, _)| r).collect();
        assert_eq!(ranges, [0..4, 4..5, 5..10]);
    }
}
