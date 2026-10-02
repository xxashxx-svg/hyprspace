// Draws parsed markdown blocks as GPUI elements: styled text for paragraphs, with links that
// open in the browser and inline code in the mono font. Text size and line height come from the
// caller, so a reply and a quoted snippet can differ.

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement, SharedString,
    StrikethroughStyle, StyledText, UnderlineStyle, div, prelude::*, px,
};
use hyprspace_theme::MONO;

use super::code::CodeBlock;
use super::select;
use super::{Block, Inline};
use crate::colors;

/// `key` makes the element ids unique per transcript item.
pub fn render(blocks: &[Block], key: &str) -> AnyElement {
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
            .children(items.iter().enumerate().map(|(n, item)| {
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
                            .child(render(item, &format!("{key}-{n}"))),
                    )
            }))
            .into_any_element(),
        Block::Quote(inner) => div()
            .pl_3()
            .border_l_2()
            .border_color(colors::border2())
            .text_color(colors::text2())
            .child(render(inner, key))
            .into_any_element(),
        Block::Rule => div()
            .h(px(1.))
            .my_1()
            .bg(colors::border2())
            .into_any_element(),
        Block::Table { head, rows } => table(head, rows, key),
    }
}

fn table(head: &[Inline], rows: &[Vec<Inline>], key: &str) -> AnyElement {
    let row = |cells: &[Inline], key: String, bold: bool| {
        div()
            .flex()
            .border_b_1()
            .border_color(colors::border1())
            .children(cells.iter().enumerate().map(|(i, c)| {
                div()
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py_1()
                    .when(bold, |d| d.font_weight(FontWeight::SEMIBOLD))
                    .child(inline(c, &format!("{key}-{i}")))
            }))
    };
    div()
        .flex()
        .flex_col()
        .rounded_md()
        .border_1()
        .border_color(colors::border1())
        .child(row(head, format!("{key}-h"), true))
        .children(
            rows.iter()
                .enumerate()
                .map(|(i, r)| row(r, format!("{key}-{i}"), false)),
        )
        .into_any_element()
}

/// One run of styled text. Links get their own click targets.
pub fn inline(i: &Inline, key: &str) -> AnyElement {
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
                    background_color: s.code.then(|| colors::ink(0.08)),
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
