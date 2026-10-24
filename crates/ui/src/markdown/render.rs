// Draws parsed markdown blocks as GPUI elements: styled text for paragraphs, with links that
// open in the browser and inline code in the mono font. Text size and line height come from the
// caller, so a reply and a quoted snippet can differ.

use std::ops::Range;

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement, SharedString,
    StrikethroughStyle, StyledText, UnderlineStyle, div, prelude::*, relative,
};
use hyprspace_theme::MONO;

use super::code::CodeBlock;
use super::fade::{block_size, local, merge};
use super::select;
use super::{Block, Inline};
use crate::colors;
use crate::zoom::px;

/// `key` makes the element ids unique per transcript item.
pub fn render(blocks: &[Block], key: &str, fade: &[(Range<usize>, f32)]) -> AnyElement {
    from(blocks, key, 0, fade)
}

/// The pieces a long reply can be cut into: a top-level block, or one item of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Block(usize),
    Entry(usize, usize),
}

pub fn units(blocks: &[Block]) -> Vec<Unit> {
    let mut out = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::List { items, .. } => out.extend((0..items.len()).map(|n| Unit::Entry(i, n))),
            _ => out.push(Unit::Block(i)),
        }
    }
    out
}

/// The units `range` of `all`, drawn as `render` draws them, for a reply cut into rows.
pub fn part(
    blocks: &[Block],
    key: &str,
    fade: &[(Range<usize>, f32)],
    all: &[Unit],
    range: Range<usize>,
) -> AnyElement {
    let at = |u: Unit| -> usize {
        if fade.is_empty() {
            return 0;
        }
        let (i, n) = match u {
            Unit::Block(i) => (i, 0),
            Unit::Entry(i, n) => (i, n),
        };
        let before: usize = blocks[..i].iter().map(block_size).sum();
        match &blocks[i] {
            Block::List { items, .. } => {
                before
                    + items[..n]
                        .iter()
                        .map(|b| super::fade::size(b))
                        .sum::<usize>()
            }
            _ => before,
        }
    };
    let units = &all[range.clone()];
    let mut groups = Vec::new();
    let mut k = 0;
    while k < units.len() {
        match units[k] {
            Unit::Block(i) => {
                groups.push(block(&blocks[i], &format!("{key}-{i}"), at(units[k]), fade));
                k += 1;
            }
            Unit::Entry(i, _) => {
                let run = units[k..]
                    .iter()
                    .take_while(|u| matches!(u, Unit::Entry(j, _) if *j == i))
                    .count();
                if let Block::List { start, items } = &blocks[i] {
                    groups.push(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(units[k..k + run].iter().filter_map(|&u| match u {
                                Unit::Entry(_, n) => Some(entry(
                                    *start,
                                    n,
                                    &items[n],
                                    &format!("{key}-{i}"),
                                    at(u),
                                    fade,
                                )),
                                Unit::Block(_) => None,
                            }))
                            .into_any_element(),
                    );
                }
                k += run;
            }
        }
    }
    let seed = (range.start > 0 && super::find::hunting()).then(|| {
        let mut above = Vec::new();
        for &u in &all[..range.start] {
            super::find::unit_texts(blocks, u, &mut above);
        }
        super::find::seed(key, above)
    });
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(seed)
        .children(groups)
        .into_any_element()
}

fn entry(
    start: Option<u64>,
    n: usize,
    item: &[Block],
    key: &str,
    at: usize,
    fade: &[(Range<usize>, f32)],
) -> gpui::Div {
    let marker = match start {
        Some(s) => format!("{}.", s + n as u64),
        None => "•".into(),
    };
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .flex_none()
                .min_w(px(14.))
                .text_color(colors::text2())
                .child(marker),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(from(item, &format!("{key}-{n}"), at, fade)),
        )
}

fn from(blocks: &[Block], key: &str, mut at: usize, fade: &[(Range<usize>, f32)]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(blocks.iter().enumerate().map(|(i, b)| {
            let el = block(b, &format!("{key}-{i}"), at, fade);
            if !fade.is_empty() {
                at += block_size(b);
            }
            el
        }))
        .into_any_element()
}

fn block(b: &Block, key: &str, at: usize, fade: &[(Range<usize>, f32)]) -> AnyElement {
    match b {
        Block::Paragraph(i) => inline(i, key, at, fade),
        Block::Heading(level, i) => {
            let size = match level {
                1 => px(18.),
                2 => px(16.),
                _ => px(14.5),
            };
            div()
                .pt_1()
                .text_size(size)
                .font_weight(FontWeight::SEMIBOLD)
                .child(inline(i, key, at, fade))
                .into_any_element()
        }
        Block::Code { lang, text } => CodeBlock::new(key, lang, text)
            .fade(at, fade)
            .into_any_element(),
        Block::List { start, items } => div()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.iter().enumerate().scan(at, |at, (n, item)| {
                let here = *at;
                if !fade.is_empty() {
                    *at += super::fade::size(item);
                }
                Some(entry(*start, n, item, key, here, fade))
            }))
            .into_any_element(),
        Block::Quote(inner) => div()
            .pl_3()
            .border_l_2()
            .border_color(colors::border2())
            .text_color(colors::text2())
            .child(from(inner, key, at, fade))
            .into_any_element(),
        Block::Rule => div()
            .h(px(1.))
            .my_1()
            .bg(colors::border2())
            .into_any_element(),
        Block::Table { head, rows } => table(head, rows, key, at, fade),
    }
}

fn columns(head: &[Inline], rows: &[Vec<Inline>]) -> Vec<(f32, f32)> {
    let count = rows
        .iter()
        .map(Vec::len)
        .chain([head.len()])
        .max()
        .unwrap_or(0);
    (0..count)
        .map(|c| {
            let cells = || {
                std::iter::once(head.get(c))
                    .chain(rows.iter().map(|r| r.get(c)))
                    .flatten()
            };
            let longest = cells().map(|i| i.text.chars().count()).max().unwrap_or(0);
            let word = cells()
                .flat_map(|i| i.text.split_whitespace())
                .map(|w| w.chars().count())
                .max()
                .unwrap_or(0);
            (longest.clamp(3, 48) as f32, word.min(16) as f32 * 7.5 + 20.)
        })
        .collect()
}

fn table(
    head: &[Inline],
    rows: &[Vec<Inline>],
    key: &str,
    at: usize,
    fade: &[(Range<usize>, f32)],
) -> AnyElement {
    let sizes = columns(head, rows);
    let mut at = at;
    let mut row = |cells: &[Inline], key: String, head: bool| {
        div()
            .flex()
            .mx(px(4.))
            .when(!head, |d| d.border_t_1().border_color(colors::border1()))
            .children(cells.iter().enumerate().map(|(i, c)| {
                let (grow, min) = sizes.get(i).copied().unwrap_or((3., 0.));
                let mut cell = div().flex_basis(relative(0.)).min_w(px(min));
                cell.style().flex_grow = Some(grow);
                cell.style().flex_shrink = Some(1.);
                cell.px(px(10.))
                    .py(px(7.))
                    .when(head, |d| {
                        d.font_weight(FontWeight::MEDIUM)
                            .text_size(px(12.5))
                            .text_color(colors::text2())
                    })
                    .child({
                        let el = inline(c, &format!("{key}-{i}"), at, fade);
                        at += c.text.chars().count();
                        el
                    })
            }))
    };
    div()
        .id(SharedString::from(format!("{key}-table")))
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .rounded(px(10.))
        .bg(colors::ink(0.035))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_full()
                .py(px(2.))
                .child(row(head, format!("{key}-h"), true))
                .children(
                    rows.iter()
                        .enumerate()
                        .map(|(i, r)| row(r, format!("{key}-{i}"), false)),
                ),
        )
        .into_any_element()
}

/// One run of styled text. Links get their own click targets.
pub fn inline(i: &Inline, key: &str, at: usize, fade: &[(Range<usize>, f32)]) -> AnyElement {
    let highlights: Vec<_> = i
        .spans
        .iter()
        .map(|(range, s)| {
            let link = s.link.is_some();
            (
                range.clone(),
                HighlightStyle {
                    color: link.then(colors::link),
                    font_weight: s.bold.then_some(FontWeight::SEMIBOLD),
                    font_style: s.italic.then_some(FontStyle::Italic),
                    // GPUI can't round a highlight's corners, so the wash stays faint
                    background_color: s.code.then(|| colors::ink(0.06)),
                    underline: link.then(|| UnderlineStyle {
                        thickness: px(1.),
                        ..Default::default()
                    }),
                    strikethrough: s.strike.then(|| StrikethroughStyle {
                        thickness: px(1.),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
        })
        .collect();
    let highlights = merge(i.text.len(), highlights, &local(&i.text, at, fade));
    let fonts: Vec<_> = i
        .spans
        .iter()
        .filter(|(_, s)| s.code)
        .map(|(r, _)| (r.clone(), SharedString::from(MONO)))
        .collect();
    let text = StyledText::new(i.text.clone())
        .with_highlights(highlights)
        .with_font_family_overrides(fonts);
    let layout = text.layout().clone();
    let links: Vec<_> = i
        .spans
        .iter()
        .filter_map(|(r, s)| s.link.clone().map(|l| (r.clone(), l)))
        .collect();
    let body = if links.is_empty() {
        text.into_any_element()
    } else {
        let ranges = links.iter().map(|(r, _)| r.clone()).collect();
        let urls: Vec<String> = links.into_iter().map(|(_, l)| l).collect();
        InteractiveText::new(SharedString::from(format!("md-{key}")), text)
            .on_click(ranges, move |ix, _, cx| {
                if let Some(url) = urls.get(ix) {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    };
    select::wrap(key, i.text.clone().into(), layout, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(text: &str) -> Inline {
        Inline {
            text: text.into(),
            spans: Vec::new(),
        }
    }

    #[test]
    fn columns_take_room_by_what_they_hold() {
        let head = ["#", "Planet", "Notable"].map(cell).to_vec();
        let rows = vec![
            [
                "1",
                "Mercury",
                "Smallest planet. Surface swings from about -180C at night to 430C in the day",
            ]
            .map(cell)
            .to_vec(),
            ["2", "Venus", "Hottest planet"].map(cell).to_vec(),
        ];
        let c = columns(&head, &rows);
        assert_eq!(c.iter().map(|c| c.0).collect::<Vec<_>>(), [3., 7., 48.]);
        assert!(c[0].1 < c[1].1 && c[1].1 < c[2].1);
        assert_eq!(c[2].1, 7.5 * 8. + 20.);
    }
}
