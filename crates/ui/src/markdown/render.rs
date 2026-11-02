// Draws parsed markdown blocks as GPUI elements: styled text for paragraphs, with links that
// open in the browser and inline code in the mono font. Text size and line height come from the
// caller, so a reply and a quoted snippet can differ.

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement, ObjectFit,
    SharedString, StrikethroughStyle, StyledText, UnderlineStyle, div, img, prelude::*, relative,
};
use hyprspace_theme::MONO;

use super::code::CodeBlock;
use super::select;
use super::{Block, Inline, Style};
use crate::colors;
use crate::zoom::px;

/// `key` makes the element ids unique per transcript item.
/// `fade` is each top-level block's opacity while it fades in; a block past its end is solid.
pub fn render(blocks: &[Block], key: &str, fade: &[f32]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(
            blocks
                .iter()
                .enumerate()
                .map(|(i, b)| shown(block(b, &format!("{key}-{i}")), fade.get(i))),
        )
        .into_any_element()
}

fn shown(el: AnyElement, fade: Option<&f32>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .when_some(fade.filter(|a| **a < 1.), |d, a| d.opacity(*a))
        .child(el)
        .into_any_element()
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
    fade: &[f32],
    all: &[Unit],
    range: std::ops::Range<usize>,
) -> AnyElement {
    let units = &all[range.clone()];
    let mut groups = Vec::new();
    let mut k = 0;
    while k < units.len() {
        match units[k] {
            Unit::Block(i) => {
                groups.push(shown(block(&blocks[i], &format!("{key}-{i}")), fade.get(i)));
                k += 1;
            }
            Unit::Entry(i, _) => {
                let run = units[k..]
                    .iter()
                    .take_while(|u| matches!(u, Unit::Entry(j, _) if *j == i))
                    .count();
                if let Block::List { start, items } = &blocks[i] {
                    let list = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(units[k..k + run].iter().filter_map(|&u| match u {
                            Unit::Entry(_, n) => {
                                Some(entry(*start, n, &items[n], &format!("{key}-{i}")))
                            }
                            Unit::Block(_) => None,
                        }))
                        .into_any_element();
                    groups.push(shown(list, fade.get(i)));
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

fn entry(start: Option<u64>, n: usize, item: &[Block], key: &str) -> gpui::Div {
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
                .child(from(item, &format!("{key}-{n}"))),
        )
}

fn from(blocks: &[Block], key: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(
            blocks
                .iter()
                .enumerate()
                .map(|(i, b)| block(b, &format!("{key}-{i}"))),
        )
        .into_any_element()
}

fn block(b: &Block, key: &str) -> AnyElement {
    match b {
        Block::Paragraph(i) => inline(i, key),
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
                .child(inline(i, key))
                .into_any_element()
        }
        Block::Code { lang, text } => CodeBlock::new(key, lang, text).into_any_element(),
        Block::List { start, items } => div()
            .flex()
            .flex_col()
            .gap_1()
            .children(
                items
                    .iter()
                    .enumerate()
                    .map(|(n, item)| entry(*start, n, item, key)),
            )
            .into_any_element(),
        Block::Quote(inner) => div()
            .pl_3()
            .border_l_2()
            .border_color(colors::border2())
            .text_color(colors::text2())
            .child(from(inner, key))
            .into_any_element(),
        Block::Rule => div()
            .h(px(1.))
            .my_1()
            .bg(colors::border2())
            .into_any_element(),
        Block::Table { head, rows } => table(head, rows, key),
        Block::Image { url, alt } => image(url, alt, key),
    }
}

/// A picture on the user's computer, which opens full size on a click. Anything else reads as a
/// link: a picture on the web is never fetched.
fn image(url: &str, alt: &str, key: &str) -> AnyElement {
    let links = super::links::current();
    let file = links
        .as_ref()
        .and_then(|l| (l.file)(url))
        .filter(|p| crate::attach::is_image(p));
    let (Some(path), Some(links)) = (file, links) else {
        let words = if alt.trim().is_empty() { url } else { alt };
        let words = Inline {
            text: words.to_string(),
            spans: vec![(
                0..words.len(),
                Style {
                    link: Some(url.to_string()),
                    ..Default::default()
                },
            )],
        };
        return inline(&words, key);
    };
    let show = links.show.clone();
    let open = path.clone();
    div()
        .id(SharedString::from(format!("{key}-img")))
        .w(px(420.))
        .max_w_full()
        .h(px(260.))
        .rounded(px(8.))
        .overflow_hidden()
        .border_1()
        .border_color(colors::border2())
        .bg(colors::ink(0.03))
        .cursor_pointer()
        .child(img(path).size_full().object_fit(ObjectFit::Contain))
        .on_click(move |_, window, cx| show(open.clone(), window, cx))
        .into_any_element()
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

fn table(head: &[Inline], rows: &[Vec<Inline>], key: &str) -> AnyElement {
    let sizes = columns(head, rows);
    let row = |cells: &[Inline], key: String, head: bool| {
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
                    .child(inline(c, &format!("{key}-{i}")))
            }))
    };
    div()
        .id(SharedString::from(format!("{key}-table")))
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .rounded(px(12.))
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
pub fn inline(i: &Inline, key: &str) -> AnyElement {
    let handler = super::links::current();
    // a path written as code that names a file opens it, as a link would
    let named: Vec<bool> = i
        .spans
        .iter()
        .map(|(r, s)| {
            s.code
                && s.link.is_none()
                && handler
                    .as_ref()
                    .is_some_and(|h| (h.file)(&i.text[r.clone()]).is_some())
        })
        .collect();
    let highlights: Vec<_> = i
        .spans
        .iter()
        .zip(&named)
        .map(|((range, s), named)| {
            let link = s.link.is_some() || *named;
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
        .zip(&named)
        .filter_map(|((r, s), named)| {
            let url = s
                .link
                .clone()
                .or_else(|| named.then(|| i.text[r.clone()].to_string()))?;
            Some((r.clone(), url))
        })
        .collect();
    let body = if links.is_empty() {
        text.into_any_element()
    } else {
        let ranges = links.iter().map(|(r, _)| r.clone()).collect();
        let urls: Vec<String> = links.into_iter().map(|(_, l)| l).collect();
        InteractiveText::new(SharedString::from(format!("md-{key}")), text)
            .on_click(ranges, move |ix, window, cx| {
                // a drag that ends on the link it started on selected its words
                if select::selected_text().is_some() {
                    return;
                }
                if let Some(url) = urls.get(ix) {
                    match &handler {
                        Some(h) => (h.open)(url, window, cx),
                        None => cx.open_url(url),
                    }
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
