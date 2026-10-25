use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use gpui::{IntoElement, Styled, TextLayout, fill};

use super::Block;
use crate::colors;

struct Hunt {
    query: String,
    current: Option<(usize, usize)>,
}

thread_local! {
    static HUNTS: RefCell<HashMap<Arc<str>, Hunt>> = RefCell::new(HashMap::new());
    static SEEN: RefCell<(usize, usize)> = const { RefCell::new((usize::MAX, 0)) };
}

pub fn set(surface: &Arc<str>, hunt: Option<(&str, Option<(usize, usize)>)>) {
    HUNTS.with(|h| {
        let mut h = h.borrow_mut();
        match hunt {
            Some((query, current)) if !query.is_empty() => {
                h.insert(
                    surface.clone(),
                    Hunt {
                        query: query.to_string(),
                        current,
                    },
                );
            }
            _ => {
                h.remove(surface);
            }
        }
    });
}

pub(super) fn hunting() -> bool {
    HUNTS.with(|h| !h.borrow().is_empty())
}

/// Paint first in a later piece of a reply cut into rows: the pieces above it may be off screen
/// and unpainted, so it counts their matches itself and the current match keeps its number.
pub(super) fn seed(key: &str, above: Vec<String>) -> impl IntoElement + use<> {
    let item = item_of(key);
    gpui::canvas(
        |_, _, _| {},
        move |_, _, _, _| {
            let Some(item) = item else {
                return;
            };
            let surface = super::select::surface();
            let Some(query) = HUNTS.with(|h| h.borrow().get(&surface).map(|h| h.query.clone()))
            else {
                return;
            };
            let n = above.iter().map(|t| occurrences(t, &query).len()).sum();
            SEEN.with(|s| *s.borrow_mut() = (item, n));
        },
    )
    .absolute()
    .size_0()
}

pub(super) fn reset() {
    SEEN.with(|s| *s.borrow_mut() = (usize::MAX, 0));
}

pub(super) fn paint(
    surface: &Arc<str>,
    key: &str,
    text: &str,
    layout: &TextLayout,
    window: &mut gpui::Window,
) {
    let Some(item) = item_of(key) else {
        return;
    };
    let Some((query, current)) = HUNTS.with(|h| {
        h.borrow()
            .get(surface)
            .map(|h| (h.query.clone(), h.current))
    }) else {
        return;
    };
    let first = SEEN.with(|s| {
        let mut s = s.borrow_mut();
        if s.0 != item {
            *s = (item, 0);
        }
        s.1
    });
    let found = occurrences(text, &query);
    SEEN.with(|s| s.borrow_mut().1 += found.len());
    for (n, range) in found.iter().enumerate() {
        let on = current == Some((item, first + n));
        let wash = colors::busy().opacity(if on { 0.6 } else { 0.25 });
        for rect in super::select::rects(layout, range) {
            window.paint_quad(fill(rect, wash));
        }
    }
}

fn item_of(key: &str) -> Option<usize> {
    let key = key.strip_prefix("code-").unwrap_or(key);
    let rest = key.strip_prefix('u').or_else(|| key.strip_prefix('t'))?;
    rest.split('-').next()?.parse().ok()
}

pub fn occurrences(text: &str, query: &str) -> Vec<Range<usize>> {
    let (hay, needle) = (text.as_bytes(), query.as_bytes());
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut at = 0;
    while at + needle.len() <= hay.len() {
        if text.is_char_boundary(at) && hay[at..at + needle.len()].eq_ignore_ascii_case(needle) {
            out.push(at..at + needle.len());
            at += needle.len();
        } else {
            at += 1;
        }
    }
    out
}

/// The text of one piece of a reply, as `texts` gives it.
pub fn unit_texts(blocks: &[Block], unit: super::Unit, out: &mut Vec<String>) {
    match unit {
        super::Unit::Block(i) => texts(&blocks[i..=i], out),
        super::Unit::Entry(i, n) => {
            if let Block::List { items, .. } = &blocks[i] {
                texts(&items[n], out);
            }
        }
    }
}

pub fn texts(blocks: &[Block], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Paragraph(i) | Block::Heading(_, i) => out.push(i.text.clone()),
            Block::Code { text, .. } => out.push(text.trim_end().to_string()),
            Block::List { items, .. } => {
                for item in items {
                    texts(item, out);
                }
            }
            Block::Quote(inner) => texts(inner, out),
            Block::Rule | Block::Image { .. } => {}
            Block::Table { head, rows } => {
                out.extend(head.iter().map(|c| c.text.clone()));
                for row in rows {
                    out.extend(row.iter().map(|c| c.text.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ignore_ascii_case_and_never_overlap() {
        assert_eq!(occurrences("Foo foo FOO", "foo"), [0..3, 4..7, 8..11]);
        assert_eq!(occurrences("aaaa", "aa"), [0..2, 2..4]);
        assert_eq!(occurrences("café Café", "café"), [0..5, 6..11]);
        assert!(occurrences("abc", "").is_empty());
    }

    #[test]
    fn keys_name_their_item() {
        assert_eq!(item_of("u12"), Some(12));
        assert_eq!(item_of("t7-0-3"), Some(7));
        assert_eq!(item_of("code-t7-2"), Some(7));
        assert_eq!(item_of("plan4-0"), None);
        assert_eq!(item_of("agent4-0"), None);
    }

    #[test]
    fn texts_follow_the_order_they_are_drawn_in() {
        let mut out = Vec::new();
        texts(
            &crate::markdown::parse("# Head\n\none\n\n- a\n- b\n\n```\ncode\n```\n"),
            &mut out,
        );
        assert_eq!(out, ["Head", "one", "a", "b", "code"]);
    }
}
