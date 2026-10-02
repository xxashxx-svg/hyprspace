// One file's working tree diff, as the Tauri app's DiffViewer drew it: hunk headers in the
// waiting blue, additions and deletions washed in the theme's diff colors. Each line also gets
// its old and new line numbers, read from the hunk headers.

use std::ops::Range;

use gpui::{
    AnyElement, Context, Hsla, IntoElement, ListHorizontalSizingBehavior, SharedString,
    UniformListScrollHandle, div, prelude::*, px, uniform_list,
};
use hyprspace_theme::MONO;

use super::code::ROW;
use super::{Body, Viewer};
use crate::colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Meta,
    Hunk,
    Add,
    Del,
    Same,
}

struct Row {
    kind: Kind,
    text: SharedString,
    old: Option<u32>,
    new: Option<u32>,
}

pub struct Diff {
    raw: String,
    rows: Vec<Row>,
    widest: usize,
}

/// `@@ -12,5 +12,7 @@`: where the hunk starts in the old and the new file.
fn hunk_start(line: &str) -> Option<(u32, u32)> {
    let mut parts = line.split_whitespace().skip(1);
    let num = |s: &str| s[1..].split(',').next()?.parse().ok();
    Some((num(parts.next()?)?, num(parts.next()?)?))
}

fn kind(line: &str) -> Kind {
    const META: &[&str] = &[
        "+++",
        "---",
        "diff ",
        "index ",
        "new file",
        "deleted file",
        "similarity",
        "rename ",
        "old mode",
        "new mode",
        "\\ No newline",
    ];
    if META.iter().any(|m| line.starts_with(m)) {
        Kind::Meta
    } else if line.starts_with("@@") {
        Kind::Hunk
    } else if line.starts_with('+') {
        Kind::Add
    } else if line.starts_with('-') {
        Kind::Del
    } else {
        Kind::Same
    }
}

impl Diff {
    pub fn new(text: &str) -> Self {
        let (mut old, mut new) = (0u32, 0u32);
        let rows: Vec<Row> = text
            .trim_end_matches('\n')
            .split('\n')
            .map(|l| {
                let l = l.strip_suffix('\r').unwrap_or(l);
                let kind = kind(l);
                let (o, n) = match kind {
                    Kind::Hunk => {
                        if let Some((a, b)) = hunk_start(l) {
                            (old, new) = (a, b);
                        }
                        (None, None)
                    }
                    Kind::Add => {
                        new += 1;
                        (None, Some(new - 1))
                    }
                    Kind::Del => {
                        old += 1;
                        (Some(old - 1), None)
                    }
                    Kind::Same => {
                        old += 1;
                        new += 1;
                        (Some(old - 1), Some(new - 1))
                    }
                    Kind::Meta => (None, None),
                };
                Row {
                    kind,
                    text: l.replace('\t', "    ").into(),
                    old: o,
                    new: n,
                }
            })
            .collect();
        let widest = rows
            .iter()
            .enumerate()
            .max_by_key(|(_, r)| r.text.len())
            .map_or(0, |(i, _)| i);
        Self {
            raw: text.to_string(),
            rows,
            widest,
        }
    }

    /// The same text as this diff, so a poll that changed nothing redraws nothing.
    pub fn is(&self, text: &str) -> bool {
        self.raw == text
    }

    pub fn counts(&self) -> (usize, usize) {
        let n = |k| self.rows.iter().filter(|r| r.kind == k).count();
        (n(Kind::Add), n(Kind::Del))
    }

    fn row(&self, ix: usize) -> AnyElement {
        let r = &self.rows[ix];
        let wash = |c: Hsla| c.opacity(0.12);
        let (bg, fg) = match r.kind {
            Kind::Meta => (None, colors::text3()),
            Kind::Hunk => (Some(colors::waiting().opacity(0.08)), colors::waiting()),
            Kind::Add => (Some(wash(colors::diff_add())), colors::text1()),
            Kind::Del => (Some(wash(colors::diff_del())), colors::text1()),
            Kind::Same => (None, colors::text2()),
        };
        let num = |n: Option<u32>| {
            div()
                .flex_none()
                .w(px(40.))
                .pr(px(8.))
                .text_right()
                .text_color(colors::text3().opacity(0.7))
                .child(SharedString::from(
                    n.map(|n| n.to_string()).unwrap_or_default(),
                ))
        };
        div()
            .flex()
            .min_w_full()
            .h(px(ROW))
            .when_some(bg, |d, c| d.bg(c))
            .child(num(r.old))
            .child(num(r.new))
            .child(
                div()
                    .flex_none()
                    .pl(px(6.))
                    .pr(px(24.))
                    .whitespace_nowrap()
                    .text_color(fg)
                    .child(if r.text.is_empty() {
                        SharedString::from(" ")
                    } else {
                        r.text.clone()
                    }),
            )
            .into_any_element()
    }

    pub fn render(&self, scroll: UniformListScrollHandle, cx: &mut Context<Viewer>) -> AnyElement {
        uniform_list(
            "diff",
            self.rows.len(),
            cx.processor(|v, range: Range<usize>, _, _| match &v.body {
                Body::Diff(d) => range.map(|ix| d.row(ix)).collect(),
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

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/a.rs b/a.rs\nindex 1..2 100644\n--- a/a.rs\n+++ b/a.rs\n@@ -3,3 +3,4 @@ fn x\n keep\n-old\n+new\n+more\n tail\n";

    #[test]
    fn numbers_follow_the_hunks() {
        let d = Diff::new(DIFF);
        let got: Vec<_> = d.rows.iter().map(|r| (r.kind, r.old, r.new)).collect();
        assert_eq!(got[4], (Kind::Hunk, None, None));
        assert_eq!(got[5], (Kind::Same, Some(3), Some(3)));
        assert_eq!(got[6], (Kind::Del, Some(4), None));
        assert_eq!(got[7], (Kind::Add, None, Some(4)));
        assert_eq!(got[8], (Kind::Add, None, Some(5)));
        assert_eq!(got[9], (Kind::Same, Some(5), Some(6)));
        assert_eq!(d.counts(), (2, 1));
        assert!(d.is(DIFF));
        assert_eq!(d.rows[2].kind, Kind::Meta);
    }

    #[test]
    fn hunk_headers_parse_with_or_without_counts() {
        assert_eq!(hunk_start("@@ -1 +1 @@"), Some((1, 1)));
        assert_eq!(hunk_start("@@ -0,0 +1,3 @@"), Some((0, 1)));
        assert_eq!(hunk_start("@@ junk"), None);
    }
}
