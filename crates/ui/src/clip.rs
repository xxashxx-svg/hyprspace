use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};

use gpui::{App, ClickEvent, Div, SharedString, Stateful, Window, div, prelude::*};

use crate::colors;
use crate::zoom::px;

const LINES: usize = 16;

thread_local! {
    // the list drops rows that scroll away, and their state with them
    static OPEN: RefCell<HashSet<String>> = RefCell::default();
}

pub fn long(key: &str, text: &str) -> Option<(String, usize)> {
    let n = text.trim_end().lines().count();
    if n <= LINES {
        return None;
    }
    // row keys repeat across threads, and output grows while it streams; its head doesn't
    let mut h = DefaultHasher::new();
    text.lines().take(LINES).for_each(|l| l.hash(&mut h));
    Some((format!("{key}-{:x}", h.finish()), n - LINES))
}

// a cut block lays out only what shows; a log of thousands of lines would be shaped every frame
pub fn head(text: &str) -> &str {
    text.match_indices('\n')
        .nth(LINES - 1)
        .map_or(text, |(i, _)| &text[..i])
}

pub fn open(key: &str) -> bool {
    OPEN.with(|o| o.borrow().contains(key))
}

pub fn flip(key: &str) {
    OPEN.with(|o| {
        let mut o = o.borrow_mut();
        if !o.remove(key) {
            o.insert(key.to_string());
        }
    });
}

pub fn lines(height: f32) -> f32 {
    LINES as f32 * height
}

pub fn cut(key: &str, height: f32, body: Div) -> Div {
    body.when(!open(key), |d| d.max_h(px(height)).overflow_hidden())
}

pub fn more(
    key: &str,
    more: usize,
    toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("clip-{key}")))
        .text_size(px(12.))
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text1()))
        .child(if open(key) {
            "Show less".to_string()
        } else if more == 1 {
            "Show 1 more line".to_string()
        } else {
            format!("Show {more} more lines")
        })
        .on_click(toggle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_block_shows_its_first_lines_and_counts_the_rest() {
        let text: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let (_, more) = long("k", &text).unwrap();
        assert_eq!(more, 24);
        assert_eq!(head(&text).lines().count(), LINES);
        assert!(head(&text).ends_with("line 15"));
        let short: String = (0..LINES).map(|i| format!("line {i}\n")).collect();
        assert!(long("k", &short).is_none());
        assert_eq!(head("one\ntwo"), "one\ntwo");
    }

    #[test]
    fn a_streaming_block_keeps_its_key() {
        let a: String = (0..20).map(|i| format!("{i}\n")).collect();
        let b = format!("{a}more\nand more\n");
        assert_eq!(long("k", &a).unwrap().0, long("k", &b).unwrap().0);
    }
}
