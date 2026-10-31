// A unified diff, drawn like zeron's: git's file lines dropped (the card's header names the
// file and counts its lines), quiet hunk headers, and additions and deletions washed in the
// theme's diff colors with a bar on the left edge and the sign in its own column. Each line also
// gets its old and new line numbers, read from the hunk headers. The viewer, the transcript's
// edits and the approval dock all draw rows through `Diff::draw`.

use std::ops::Range;
use std::path::Path;

use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ListHorizontalSizingBehavior, SharedString,
    StyledText, UniformListScrollHandle, div, prelude::*, uniform_list,
};
use hyprspace_syntax::Line;
use hyprspace_theme::MONO;

use super::code::{self, ROW};
use super::{Body, Viewer};
use crate::colors;
use crate::zoom::px;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
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

pub struct Fit {
    row: f32,
    num: f32,
    wash: f32,
    wrap: bool,
}

pub const FULL: Fit = Fit {
    row: ROW,
    num: 40.,
    wash: 0.1,
    wrap: false,
};

pub const COMPACT: Fit = Fit {
    row: 17.,
    num: 30.,
    wash: 0.08,
    wrap: true,
};

pub struct Diff {
    raw: String,
    rows: Vec<Row>,
    widest: usize,
    colors: Vec<Line>,
}

/// A row before its tabs become spaces, which the highlighter needs.
struct Read {
    kind: Kind,
    body: String,
    old: Option<u32>,
    new: Option<u32>,
    file: usize,
}

const META: &[&str] = &[
    "+++",
    "---",
    "index ",
    "new file",
    "deleted file",
    "similarity",
    "rename ",
    "old mode",
    "new mode",
];

/// `@@ -12,5 +12,7 @@`: where the hunk starts in the old and the new file.
fn hunk_start(line: &str) -> Option<(u32, u32)> {
    let mut parts = line.split_whitespace().skip(1);
    let num = |s: &str| s.get(1..)?.split(',').next()?.parse().ok();
    Some((num(parts.next()?)?, num(parts.next()?)?))
}

/// The file a `diff --git a/x b/x` line names. Renames are off, so both halves match.
fn git_path(rest: &str) -> String {
    let rest = rest.replace('"', "");
    let half = rest.len().saturating_sub(1) / 2;
    match (rest.get(..half), rest.get(half..)) {
        (Some(a), Some(b)) if a.starts_with("a/") && b.starts_with(" b/") && a[2..] == b[3..] => {
            a[2..].to_string()
        }
        _ => rest
            .rsplit_once(" b/")
            .map_or(rest.clone(), |(_, p)| p.to_string()),
    }
}

fn read(text: &str, path: &str) -> (Vec<Read>, Vec<String>) {
    let mut files = vec![path.to_string()];
    let mut rows: Vec<Read> = Vec::new();
    let (mut old, mut new, mut known, mut header) = (0u32, 0u32, false, true);
    let text = text.trim_end_matches('\n');
    for l in text.split('\n').filter(|_| !text.is_empty()) {
        let l = l.strip_suffix('\r').unwrap_or(l);
        let file = files.len() - 1;
        let row = |kind: Kind, body: &str, old: Option<u32>, new: Option<u32>| Read {
            kind,
            body: body.to_string(),
            old,
            new,
            file,
        };
        if let Some(rest) = l.strip_prefix("diff --git ") {
            let name = git_path(rest);
            rows.push(Read {
                file: files.len(),
                ..row(Kind::File, &name, None, None)
            });
            files.push(name);
            header = true;
            continue;
        }
        if l.starts_with("\\ ") || header && META.iter().any(|m| l.starts_with(m)) {
            continue;
        }
        header = false;
        if l.starts_with("@@") {
            let first = rows.last().is_none_or(|r| r.kind == Kind::File);
            match hunk_start(l) {
                Some((a, b)) => (old, new, known) = (a, b, true),
                None => known = false,
            }
            // Claude's writes, and edits it couldn't place, come with a bare `@@`
            if !(first && l.trim() == "@@") {
                rows.push(row(Kind::Hunk, l, None, None));
            }
            continue;
        }
        let num = |n: u32| known.then_some(n);
        let r = match l.as_bytes().first() {
            Some(b'+') => {
                new += 1;
                row(Kind::Add, &l[1..], None, num(new - 1))
            }
            Some(b'-') => {
                old += 1;
                row(Kind::Del, &l[1..], num(old - 1), None)
            }
            Some(b' ') | None => {
                old += 1;
                new += 1;
                row(
                    Kind::Same,
                    l.get(1..).unwrap_or(""),
                    num(old - 1),
                    num(new - 1),
                )
            }
            _ => row(Kind::Hunk, l, None, None),
        };
        rows.push(r);
    }
    if !path.is_empty() && rows.iter().filter(|r| r.kind == Kind::File).count() < 2 {
        rows.retain(|r| r.kind != Kind::File);
    }
    (rows, files)
}

/// Slow on a big diff, so it runs off the UI thread.
pub fn colors(text: &str, path: &str) -> Vec<Line> {
    let (rows, files) = read(text, path);
    let mut out = vec![Line::new(); rows.len()];
    for (f, file) in files.iter().enumerate() {
        let file = Path::new(file);
        if hyprspace_syntax::language(file).is_none() {
            continue;
        }
        for (side, take) in [(Kind::Add, Kind::Same), (Kind::Del, Kind::Del)] {
            let ixs: Vec<usize> = (0..rows.len())
                .filter(|&i| rows[i].file == f && [side, Kind::Same].contains(&rows[i].kind))
                .collect();
            if !ixs.iter().any(|&i| [side, take].contains(&rows[i].kind)) {
                continue;
            }
            let source: Vec<&str> = ixs.iter().map(|&i| rows[i].body.as_str()).collect();
            let Some(lines) = hyprspace_syntax::highlight(file, &source.join("\n")) else {
                continue;
            };
            for (&i, spans) in ixs.iter().zip(lines) {
                if [side, take].contains(&rows[i].kind) {
                    out[i] = code::expand(&rows[i].body, &spans).1;
                }
            }
        }
    }
    out
}

impl Diff {
    pub fn new(text: &str, path: &str) -> Self {
        let rows: Vec<Row> = read(text, path)
            .0
            .into_iter()
            .map(|r| Row {
                kind: r.kind,
                text: code::expand(&r.body, &[]).0.into(),
                old: r.old,
                new: r.new,
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
            colors: Vec::new(),
        }
    }

    /// The same text as this diff, so a poll that changed nothing redraws nothing.
    pub fn is(&self, text: &str) -> bool {
        self.raw == text
    }

    pub fn set_colors(&mut self, colors: Vec<Line>) {
        self.colors = colors;
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn counts(&self) -> (usize, usize) {
        let n = |k| self.rows.iter().filter(|r| r.kind == k).count();
        (n(Kind::Add), n(Kind::Del))
    }

    pub fn draw(&self, ix: usize, fit: &Fit) -> AnyElement {
        let r = &self.rows[ix];
        if r.kind == Kind::File {
            return div()
                .flex()
                .items_center()
                .min_w_full()
                .h(px(fit.row))
                .px(px(10.))
                .bg(colors::ink(0.06))
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(r.text.clone())
                .into_any_element();
        }
        // the line's color, its sign, and the color its text is drawn in
        let (tint, sign, fg): (Option<Hsla>, &str, Hsla) = match r.kind {
            Kind::Add => (Some(colors::diff_add()), "+", colors::text1()),
            Kind::Del => (Some(colors::diff_del()), "-", colors::text1()),
            Kind::Hunk | Kind::File => (None, "", colors::text3()),
            Kind::Same => (None, "", colors::text2()),
        };
        let num = |n: Option<u32>| {
            div()
                .flex_none()
                .w(px(fit.num))
                .pr(px(8.))
                .text_right()
                .text_color(colors::text3().opacity(0.7))
                .child(SharedString::from(
                    n.map(|n| n.to_string()).unwrap_or_default(),
                ))
        };
        let text: SharedString = if r.text.is_empty() {
            " ".into()
        } else {
            r.text.clone()
        };
        let body = StyledText::new(text).with_highlights(
            self.colors
                .get(ix)
                .into_iter()
                .flatten()
                .map(|(range, k)| (range.clone(), code::color(*k))),
        );
        div()
            .flex()
            .min_w_full()
            .map(|d| {
                if fit.wrap {
                    d.min_h(px(fit.row))
                } else {
                    d.h(px(fit.row))
                }
            })
            .when_some(tint, |d, c| d.bg(c.opacity(fit.wash)))
            .when(r.kind == Kind::Hunk, |d| d.bg(colors::ink(0.04)))
            .child(div().flex_none().w(px(2.)).when_some(tint, |d, c| d.bg(c)))
            .child(num(r.old))
            .child(num(r.new))
            .child(
                div()
                    .flex_none()
                    .w(px(14.))
                    .text_center()
                    .text_color(tint.unwrap_or(colors::text3()))
                    .child(sign),
            )
            .child(
                div()
                    .pl(px(4.))
                    .pr(px(if fit.wrap { 10. } else { 24. }))
                    .text_color(fg)
                    .map(|d| {
                        if fit.wrap {
                            d.flex_1().min_w_0()
                        } else {
                            d.flex_none().whitespace_nowrap()
                        }
                    })
                    .child(body),
            )
            .into_any_element()
    }

    pub fn render(&self, scroll: UniformListScrollHandle, cx: &mut Context<Viewer>) -> AnyElement {
        uniform_list(
            "diff",
            self.rows.len(),
            cx.processor(|v, range: Range<usize>, _, _| match &v.body {
                Body::Diff(d) => range.map(|ix| d.draw(ix, &FULL)).collect(),
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

    fn shape(d: &Diff) -> Vec<(Kind, Option<u32>, Option<u32>)> {
        d.rows.iter().map(|r| (r.kind, r.old, r.new)).collect()
    }

    #[test]
    fn numbers_follow_the_hunks() {
        let d = Diff::new(DIFF, "a.rs");
        let got = shape(&d);
        // git's four file lines are dropped
        assert_eq!(got[0], (Kind::Hunk, None, None));
        assert_eq!(got[1], (Kind::Same, Some(3), Some(3)));
        assert_eq!(got[2], (Kind::Del, Some(4), None));
        assert_eq!(got[3], (Kind::Add, None, Some(4)));
        assert_eq!(got[4], (Kind::Add, None, Some(5)));
        assert_eq!(got[5], (Kind::Same, Some(5), Some(6)));
        assert_eq!(d.counts(), (2, 1));
        assert!(d.is(DIFF));
        // the sign is drawn in its own column
        assert_eq!(d.rows[2].text.as_ref(), "old");
        assert_eq!(d.rows[1].text.as_ref(), "keep");
    }

    #[test]
    fn hunk_headers_parse_with_or_without_counts() {
        assert_eq!(hunk_start("@@ -1 +1 @@"), Some((1, 1)));
        assert_eq!(hunk_start("@@ -0,0 +1,3 @@"), Some((0, 1)));
        assert_eq!(hunk_start("@@ junk"), None);
    }

    #[test]
    fn several_files_get_a_row_each() {
        let two = format!(
            "{DIFF}diff --git a/b c.md b/b c.md\nnew file mode 100644\n--- /dev/null\n+++ b/b c.md\n@@ -0,0 +1 @@\n+++ hi\n"
        );
        let d = Diff::new(&two, "");
        let files: Vec<&str> = d
            .rows
            .iter()
            .filter(|r| r.kind == Kind::File)
            .map(|r| r.text.as_ref())
            .collect();
        assert_eq!(files, ["a.rs", "b c.md"]);
        // an added line that starts like a file line is still a line
        assert_eq!(d.counts(), (3, 1));
    }

    #[test]
    fn a_bare_hunk_has_no_numbers() {
        let d = Diff::new("@@\n-a\n+b\n", "x.rs");
        assert_eq!(
            shape(&d),
            [(Kind::Del, None, None), (Kind::Add, None, None)]
        );
    }

    #[test]
    fn colors_land_on_their_rows() {
        let c = colors(DIFF, "a.rs");
        assert_eq!(c.len(), Diff::new(DIFF, "a.rs").len());
        let d = colors("@@ -1 +1 @@\n-let a = 1;\n+fn b() {}\n", "x.rs");
        assert!(
            d[1].contains(&(0..3, hyprspace_syntax::Kind::Keyword)),
            "{d:?}"
        );
        assert!(
            d[2].contains(&(0..2, hyprspace_syntax::Kind::Keyword)),
            "{d:?}"
        );
        assert!(colors("@@\n+plain\n", "notes.unknown")[0].is_empty());
    }
}
