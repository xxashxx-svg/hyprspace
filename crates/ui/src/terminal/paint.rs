// Grid painting. Backgrounds, selection, find highlights and the cursor are quads at
// `cell_w * col`; text is shaped per row. Sizes follow the Tauri app's xterm setup: line height
// 1.1 and the pane's 12/18/10 padding (styles/pane.css). The font and its size are the user's,
// from Settings, Appearance (13px JetBrains Mono unless changed).
// Approach from zeron's terminal view (MIT, see THIRD_PARTY_NOTICES.md).

use gpui::{
    App, Bounds, Font, FontFeatures, FontStyle, FontWeight, Hsla, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, TextAlign, TextRun, UnderlineStyle, Window, fill, font, point, px,
    size,
};

use super::emulator::{Cell, CellColor, Cursor, Mark, Shape};
use super::{cell_color, glyphs};
use crate::colors::{self, hsla, theme};

const LINE_HEIGHT: f32 = 1.1;
pub const PAD_X: f32 = 18.0;
pub const PAD_TOP: f32 = 12.0;
pub const PAD_BOTTOM: f32 = 10.0;
/// The scrollbar's gutter, which the grid leaves free like xterm's real scrollbar did.
pub const BAR_W: f32 = 10.0;

/// Where the grid sits this frame, in window coordinates. Pointer events map onto cells with
/// it, so it must be the one the frame was painted with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub bounds: Bounds<Pixels>,
    pub origin: Point<Pixels>,
    pub cell_w: Pixels,
    pub line_h: Pixels,
    pub cols: u16,
    pub rows: u16,
}

impl Grid {
    /// The cell under a window position, clamped into the grid, and whether the position is on
    /// the cell's right half (a selection anchors to a cell edge).
    pub fn cell(&self, p: Point<Pixels>) -> (usize, usize, bool) {
        let x = f32::from(p.x - self.origin.x);
        let y = f32::from(p.y - self.origin.y);
        let (w, h) = (f32::from(self.cell_w), f32::from(self.line_h));
        if w <= 0.0 || h <= 0.0 {
            return (0, 0, false);
        }
        let col = (x / w).floor().clamp(0.0, (self.cols - 1) as f32) as usize;
        let row = (y / h).floor().clamp(0.0, (self.rows - 1) as f32) as usize;
        let right = x > (col as f32 + 0.5) * w || x >= self.cols as f32 * w;
        (row, col, right)
    }

    /// A cell's box in window coordinates.
    pub fn cell_bounds(&self, row: usize, col: usize, cols: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(
                self.origin.x + self.cell_w * col as f32,
                self.origin.y + self.line_h * row as f32,
            ),
            size(self.cell_w * cols as f32, self.line_h),
        )
    }
}

pub fn mono() -> Font {
    let mut mono = font(crate::settings::terminal_font().0);
    // A terminal is a fixed grid: ligatures would fold several cells into one glyph and shift the
    // rest of the row off the columns the cursor and backgrounds use.
    mono.features = FontFeatures(std::sync::Arc::new(vec![
        ("liga".into(), 0),
        ("calt".into(), 0),
        ("dlig".into(), 0),
    ]));
    mono
}

fn font_size() -> f32 {
    crate::settings::terminal_font().1
}

pub fn measure(bounds: Bounds<Pixels>, window: &mut Window) -> Grid {
    let size = font_size();
    let font_size = px(size);
    let ts = window.text_system();
    let id = ts.resolve_font(&mono());
    let cell_w = ts.em_advance(id, font_size).unwrap_or(px(size * 0.6));
    // xterm's row: the font's own line box times the line height setting
    let natural = f32::from(ts.ascent(id, font_size)) + f32::from(ts.descent(id, font_size)).abs();
    let line_h = px((natural * LINE_HEIGHT).round().max(size));
    let w = f32::from(bounds.size.width) - 2.0 * PAD_X - BAR_W;
    let h = f32::from(bounds.size.height) - PAD_TOP - PAD_BOTTOM;
    Grid {
        bounds,
        origin: point(bounds.left() + px(PAD_X), bounds.top() + px(PAD_TOP)),
        cell_w,
        line_h,
        cols: (w / f32::from(cell_w)).floor().clamp(2.0, 500.0) as u16,
        rows: (h / f32::from(line_h)).floor().clamp(1.0, 500.0) as u16,
    }
}

/// The scrollbar this frame: where its thumb is, if there is anything to scroll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub track: Bounds<Pixels>,
    pub thumb: Bounds<Pixels>,
}

/// `history` lines above the screen, `offset` of them scrolled back.
pub fn bar(grid: &Grid, history: usize, offset: usize) -> Option<Bar> {
    if history == 0 {
        return None;
    }
    let b = grid.bounds;
    let track = Bounds::new(
        point(b.right() - px(BAR_W), b.top()),
        size(px(BAR_W), b.size.height),
    );
    let total = (history + grid.rows as usize) as f32;
    let track_h = f32::from(track.size.height);
    let thumb_h = (grid.rows as f32 / total * track_h).max(24.0).min(track_h);
    // offset counts up from the live bottom; the thumb's top counts down from the top
    let from_top = (history - offset.min(history)) as f32 / history as f32;
    let y = from_top * (track_h - thumb_h);
    Some(Bar {
        track,
        thumb: Bounds::new(
            point(track.left() + px(2.), track.top() + px(y)),
            size(px(BAR_W - 4.), px(thumb_h)),
        ),
    })
}

/// The scroll offset for the thumb's top at `y` (window coordinates), from the live bottom.
pub fn offset_at(bar: &Bar, y: Pixels, history: usize) -> usize {
    let room = f32::from(bar.track.size.height - bar.thumb.size.height);
    if room <= 0.0 {
        return 0;
    }
    let from_top = (f32::from(y - bar.track.top()) / room).clamp(0.0, 1.0);
    ((1.0 - from_top) * history as f32).round() as usize
}

/// Typed text the IME has not committed yet, drawn at the cursor.
pub struct Preedit<'a> {
    pub text: &'a str,
    pub row: usize,
    pub col: usize,
}

/// Everything the frame shows besides the cells.
pub struct Extras<'a> {
    /// The cursor when it is visible this frame (focused, blink on).
    pub cursor: Option<Cursor>,
    pub preedit: Option<Preedit<'a>>,
    pub bar: Option<Bar>,
    pub bar_hot: bool,
}

pub struct Frame {
    quads: Vec<PaintQuad>,
    /// Per row: shaped segments and the column each starts at.
    rows: Vec<Vec<(usize, ShapedLine)>>,
    /// Drawn over everything else.
    top: Vec<PaintQuad>,
    preedit: Option<(Point<Pixels>, ShapedLine)>,
    origin: Point<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
}

/// One quad per run of cells that `pick` gives the same color.
fn runs(
    grid: &Grid,
    row: usize,
    line: &[Cell],
    pick: impl Fn(&Cell) -> Option<Hsla>,
    out: &mut Vec<PaintQuad>,
) {
    let mut run: Option<(usize, Hsla)> = None;
    for col in 0..=line.len() {
        let color = line.get(col).and_then(&pick);
        if run.map(|(_, c)| Some(c)) != Some(color) {
            if let Some((start, c)) = run {
                out.push(fill(grid.cell_bounds(row, start, col - start), c));
            }
            run = color.map(|c| (col, c));
        }
    }
}

pub fn prepare(grid: &Grid, lines: &[Vec<Cell>], extras: Extras, window: &Window) -> Frame {
    let t = theme();
    let selection = hsla(t.selection);
    let found = colors::busy().opacity(0.28);
    let current = colors::busy().opacity(0.6);
    let mut quads = Vec::new();
    let mut rows = Vec::with_capacity(lines.len());
    let cursor = extras.cursor.filter(|c| c.row < lines.len());
    for (r, line) in lines.iter().enumerate() {
        runs(
            grid,
            r,
            line,
            |c| {
                let bg = c.colors().1;
                (bg != CellColor::Background).then(|| cell_color(bg))
            },
            &mut quads,
        );
        runs(
            grid,
            r,
            line,
            |c| c.selected.then_some(selection),
            &mut quads,
        );
        runs(
            grid,
            r,
            line,
            |c| match c.mark {
                Mark::None => None,
                Mark::Found => Some(found),
                Mark::Current => Some(current),
            },
            &mut quads,
        );
        // a block cursor reverses the glyph under it, like xterm's cursorAccent
        let under = cursor
            .filter(|c| c.row == r && c.shape == Shape::Block)
            .map(|c| c.col);
        drawn_glyphs(grid, r, line, under, &mut quads);
        rows.push(shape_row(line, under, window));
    }
    let mut top = Vec::new();
    if let Some(c) = cursor {
        let cell = grid.cell_bounds(c.row, c.col, 1);
        let color = hsla(t.cursor);
        let thin = px(2.);
        let b = match c.shape {
            Shape::Block => cell,
            Shape::Bar => Bounds::new(cell.origin, size(thin, cell.size.height)),
            Shape::Underline => Bounds::new(
                point(cell.left(), cell.bottom() - thin),
                size(cell.size.width, thin),
            ),
        };
        // the block goes under the text so the reversed glyph shows; the thin ones go on top
        if c.shape == Shape::Block {
            quads.push(fill(b, color));
        } else {
            top.push(fill(b, color));
        }
    }
    let preedit = extras.preedit.filter(|p| !p.text.is_empty()).map(|p| {
        let at = grid.cell_bounds(p.row, p.col, 1).origin;
        let shaped = window.text_system().shape_line(
            SharedString::from(p.text.to_string()),
            px(font_size()),
            &[TextRun {
                len: p.text.len(),
                font: mono(),
                color: hsla(t.term_fg),
                background_color: Some(hsla(t.term_bg)),
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(hsla(t.term_fg)),
                    wavy: false,
                }),
                strikethrough: None,
            }],
            None,
        );
        (at, shaped)
    });
    if let Some(bar) = extras.bar {
        let a = if extras.bar_hot { 0.5 } else { 0.28 };
        top.push(fill(bar.thumb, colors::ink(a)).corner_radii(px(3.)));
    }
    Frame {
        quads,
        rows,
        top,
        preedit,
        origin: grid.origin,
        cell_w: grid.cell_w,
        line_h: grid.line_h,
    }
}

pub fn paint(bounds: Bounds<Pixels>, mut frame: Frame, window: &mut Window, cx: &mut App) {
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        for q in frame.quads.drain(..) {
            window.paint_quad(q);
        }
        for (r, segments) in frame.rows.iter().enumerate() {
            let y = frame.origin.y + frame.line_h * r as f32;
            for (col, line) in segments {
                let at = point(frame.origin.x + frame.cell_w * *col as f32, y);
                let _ = line.paint(at, frame.line_h, TextAlign::Left, None, window, cx);
            }
        }
        for q in frame.top.drain(..) {
            window.paint_quad(q);
        }
        if let Some((at, line)) = &frame.preedit {
            let _ = line.paint(*at, frame.line_h, TextAlign::Left, None, window, cx);
        }
    });
}

/// The foreground a cell's text is drawn in.
fn text_color(cell: &Cell, under_cursor: bool) -> Hsla {
    if under_cursor {
        return hsla(theme().term_bg);
    }
    let mut color = cell_color(cell.colors().0);
    if cell.dim {
        color.a *= 0.6;
    }
    color
}

/// Block and line characters as rectangles (see glyphs.rs).
fn drawn_glyphs(
    grid: &Grid,
    row: usize,
    line: &[Cell],
    cursor: Option<usize>,
    out: &mut Vec<PaintQuad>,
) {
    let (w, h) = (f32::from(grid.cell_w), f32::from(grid.line_h));
    for (col, cell) in line.iter().enumerate() {
        let Some(parts) = glyphs::rects(cell.ch, w, h) else {
            continue;
        };
        let color = text_color(cell, cursor == Some(col));
        let at = grid.cell_bounds(row, col, 1).origin;
        for p in parts {
            out.push(fill(
                Bounds::new(
                    point(at.x + px(p.x * w), at.y + px(p.y * h)),
                    size(px(p.w * w), px(p.h * h)),
                ),
                color.opacity(color.a * p.alpha),
            ));
        }
    }
}

// ASCII runs shape together because a mono font keeps them on the grid. Any other glyph may come
// from a fallback font with its own advance (box drawing, emoji, CJK), so it gets its own segment
// pinned at its column; otherwise the rest of the row drifts off the grid. `cursor` is the column
// a block cursor covers, whose glyph is drawn in the background color.
fn shape_row(line: &[Cell], cursor: Option<usize>, window: &Window) -> Vec<(usize, ShapedLine)> {
    let base = mono();
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut start = 0;
    let flush = |text: &mut String,
                 runs: &mut Vec<TextRun>,
                 start: usize,
                 out: &mut Vec<(usize, ShapedLine)>| {
        // trailing blanks with no underline paint nothing, so skip shaping them
        if text.trim_end().is_empty() && runs.iter().all(|r| r.underline.is_none()) {
            text.clear();
            runs.clear();
            return;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(std::mem::take(text)),
            px(font_size()),
            runs,
            None,
        );
        runs.clear();
        out.push((start, shaped));
    };
    for (col, cell) in line.iter().enumerate() {
        if cell.spacer {
            continue;
        }
        let pinned = !cell.ch.is_ascii() || cell.wide;
        // drawn as rectangles, so it shapes nothing; flushing keeps the rest on its columns
        if pinned && glyphs::rects(cell.ch, 1.0, 1.0).is_some() {
            flush(&mut text, &mut runs, start, &mut segments);
            continue;
        }
        if pinned {
            flush(&mut text, &mut runs, start, &mut segments);
        }
        if text.is_empty() {
            start = col;
        }
        let color = text_color(cell, cursor == Some(col));
        let mut f = base.clone();
        if cell.bold {
            f.weight = FontWeight::BOLD;
        }
        if cell.italic {
            f.style = FontStyle::Italic;
        }
        let underline = (cell.underline || cell.link).then_some(UnderlineStyle {
            thickness: px(1.),
            color: Some(color),
            wavy: false,
        });
        text.push(cell.ch);
        let len = cell.ch.len_utf8();
        match runs.last_mut() {
            Some(last) if last.color == color && last.font == f && last.underline == underline => {
                last.len += len
            }
            _ => runs.push(TextRun {
                len,
                font: f,
                color,
                background_color: None,
                underline,
                strikethrough: None,
            }),
        }
        if pinned {
            flush(&mut text, &mut runs, start, &mut segments);
        }
    }
    flush(&mut text, &mut runs, start, &mut segments);
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid {
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(400.), px(300.))),
            origin: point(px(18.), px(12.)),
            cell_w: px(8.),
            line_h: px(18.),
            cols: 40,
            rows: 10,
        }
    }

    #[test]
    fn pointer_positions_land_on_cells_and_clamp_at_the_edges() {
        let g = grid();
        assert_eq!(g.cell(point(px(18.), px(12.))), (0, 0, false));
        assert_eq!(
            g.cell(point(px(18. + 8. * 3. + 6.), px(12. + 18.))),
            (1, 3, true)
        );
        // outside the grid: the nearest cell
        assert_eq!(g.cell(point(px(0.), px(0.))), (0, 0, false));
        assert_eq!(g.cell(point(px(999.), px(999.))), (9, 39, true));
    }

    #[test]
    fn the_thumb_tracks_the_scroll_offset() {
        let g = grid();
        assert_eq!(bar(&g, 0, 0), None);
        let live = bar(&g, 90, 0).unwrap();
        let top = bar(&g, 90, 90).unwrap();
        assert!(live.thumb.top() > top.thumb.top());
        assert_eq!(top.thumb.top(), px(0.));
        assert_eq!(offset_at(&live, live.track.top(), 90), 90);
        assert_eq!(offset_at(&live, px(10_000.), 90), 0);
    }
}
