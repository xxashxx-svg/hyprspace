// How code is drawn in the viewer, shared by the editor and the diff: the row height, tabs as
// spaces (GPUI draws a tab as nothing) with the colored ranges and columns moved to match, and
// syntax colors from the theme's terminal palette, so each theme and side keeps its own.

use std::ops::Range;

use gpui::{FontStyle, FontWeight, HighlightStyle, Hsla};
use hyprspace_syntax::{Kind, Line};

use crate::colors;

pub const ROW: f32 = 19.;
const TAB: usize = 4;

/// `line` with its tabs as spaces, and `spans` moved to fit.
pub fn expand(line: &str, spans: &[(Range<usize>, Kind)]) -> (String, Line) {
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

/// Where byte `col` of `line` lands once its tabs are spaces.
pub fn shown_col(line: &str, col: usize) -> usize {
    if !line.contains('\t') {
        return col;
    }
    let mut out = 0;
    let mut chars = 0;
    for (i, ch) in line.char_indices() {
        if i >= col {
            break;
        }
        if ch == '\t' {
            let n = TAB - chars % TAB;
            out += n;
            chars += n;
        } else {
            out += ch.len_utf8();
            chars += 1;
        }
    }
    out
}

/// The byte of `line` drawn at `shown` once its tabs are spaces: a point inside a tab's spaces
/// goes to the nearer side of the tab.
pub fn raw_col(line: &str, shown: usize) -> usize {
    if !line.contains('\t') {
        return shown.min(line.len());
    }
    let mut out = 0;
    let mut chars = 0;
    for (i, ch) in line.char_indices() {
        let width = if ch == '\t' {
            TAB - chars % TAB
        } else {
            ch.len_utf8()
        };
        if shown < out + width {
            return if ch == '\t' && shown - out > width / 2 {
                i + 1
            } else {
                i
            };
        }
        out += width;
        chars += if ch == '\t' { width } else { 1 };
    }
    line.len()
}

pub fn color(kind: Kind) -> HighlightStyle {
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
    fn columns_map_both_ways_across_tabs() {
        let line = "\tfn\ta";
        // drawn "    fn  a"
        assert_eq!(shown_col(line, 0), 0);
        assert_eq!(shown_col(line, 1), 4);
        assert_eq!(shown_col(line, 3), 6);
        assert_eq!(shown_col(line, 4), 8);
        assert_eq!(raw_col(line, 1), 0);
        assert_eq!(raw_col(line, 3), 1);
        assert_eq!(raw_col(line, 5), 2);
        assert_eq!(raw_col(line, 8), 4);
        assert_eq!(raw_col(line, 40), line.len());
        assert_eq!(raw_col("abc", 9), 3);
    }
}
