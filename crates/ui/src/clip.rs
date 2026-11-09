use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, Div, SharedString, Stateful, Window, canvas, div, prelude::*,
};

use crate::colors;
use crate::pace::Looping;
use crate::slide::{self, ease_out};
use crate::zoom::px;

const LINES: usize = 16;
const GLIDE: Duration = Duration::from_millis(200);

thread_local! {
    // the list drops rows that scroll away, and their state with them
    static OPEN: RefCell<HashSet<String>> = RefCell::default();
    static HEIGHTS: RefCell<HashMap<String, f32>> = RefCell::default();
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

/// The newest lines of `text`, as many as a cut block shows.
pub fn tail(text: &str) -> &str {
    let text = text.trim_end();
    text.rmatch_indices('\n')
        .nth(LINES - 1)
        .map_or(text, |(i, _)| &text[i + 1..])
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
    glide(key);
}

/// Starts `key`'s box sliding to its new height.
pub fn glide(key: &str) {
    slide::flip(format!("clip:{key}"));
}

/// Whether `key`'s box is still sliding, and so needs all of its text drawn.
pub fn gliding(key: &str) -> bool {
    slide::flipped(&format!("clip:{key}"), GLIDE).is_some()
}

pub fn lines(height: f32) -> f32 {
    LINES as f32 * height
}

pub fn cut(key: &str, height: f32, body: Div) -> AnyElement {
    clipped(key, open(key), height, body)
}

/// `body` cut to `height` while shut, sliding between that and its whole height after `glide`.
pub fn clipped(key: &str, open: bool, height: f32, body: impl IntoElement) -> AnyElement {
    let mark = key.to_string();
    let measure = canvas(
        move |b, _, _| {
            let h = f32::from(b.size.height);
            HEIGHTS.with(|m| m.borrow_mut().insert(mark, h));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    let outer = div().w_full().flex().flex_col().overflow_hidden().child(
        div()
            .relative()
            .w_full()
            .flex_none()
            .child(body)
            .child(measure),
    );
    let Some(at) = slide::flipped_at(&format!("clip:{key}"), GLIDE) else {
        return outer
            .when(!open, |d| d.max_h(px(height)))
            .into_any_element();
    };
    let shut = f32::from(px(height));
    let key = key.to_string();
    outer
        .looping(GLIDE.as_millis() as u64, move |d, _| {
            let t = ease_out((at.elapsed().as_secs_f32() / GLIDE.as_secs_f32()).min(1.));
            let s = if open { t } else { 1. - t };
            let whole = HEIGHTS
                .with(|m| m.borrow().get(&key).copied())
                .unwrap_or(shut)
                .max(shut);
            d.h(gpui::px(shut + (whole - shut) * s))
        })
        .into_any_element()
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

    #[test]
    fn a_running_log_shows_its_newest_lines() {
        let log: String = (0..40).map(|i| format!("{i}\n")).collect();
        let shown = tail(&log);
        assert_eq!(shown.lines().count(), LINES);
        assert!(shown.starts_with("24\n") && shown.ends_with("39"));
        assert!(long("k", shown).is_none());
        assert_eq!(tail("one\ntwo\n"), "one\ntwo");
    }
}
