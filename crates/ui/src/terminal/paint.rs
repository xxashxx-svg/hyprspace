// Grid painting. Backgrounds and the cursor are quads at `cell_w * col`; text is shaped per row.
// Approach from zeron's terminal view (MIT, see THIRD_PARTY_NOTICES.md).

use gpui::{
    App, Bounds, Font, FontFeatures, FontWeight, Hsla, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, TextAlign, TextRun, Window, fill, font, point, px, size,
};

use super::cell_color;
use super::emulator::{Cell, CellColor};
use crate::colors::{THEME, hsla};

const FONT_SIZE: f32 = 13.0;
const LINE_HEIGHT: f32 = 1.3;
const PAD: f32 = 6.0;

pub struct Grid {
    origin: Point<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
    font: Font,
    pub cols: u16,
    pub rows: u16,
}

pub struct Frame {
    quads: Vec<PaintQuad>,
    /// Per row: shaped segments and the column each starts at.
    rows: Vec<Vec<(usize, ShapedLine)>>,
    origin: Point<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
    cursor: Option<PaintQuad>,
}

pub fn measure(bounds: Bounds<Pixels>, window: &mut Window) -> Grid {
    let mut mono = font(hyprspace_theme::MONO);
    // A terminal is a fixed grid: ligatures would fold several cells into one glyph and shift the
    // rest of the row off the columns the cursor and backgrounds use.
    mono.features = FontFeatures(std::sync::Arc::new(vec![
        ("liga".into(), 0),
        ("calt".into(), 0),
    ]));
    let font_size = px(FONT_SIZE);
    let id = window.text_system().resolve_font(&mono);
    let cell_w = window
        .text_system()
        .em_advance(id, font_size)
        .unwrap_or(px(FONT_SIZE * 0.6));
    let line_h = px(FONT_SIZE * LINE_HEIGHT);
    let w = f32::from(bounds.size.width) - 2.0 * PAD;
    let h = f32::from(bounds.size.height) - 2.0 * PAD;
    Grid {
        origin: point(bounds.left() + px(PAD), bounds.top() + px(PAD)),
        cell_w,
        line_h,
        font: mono,
        cols: (w / f32::from(cell_w)).floor().clamp(2.0, 500.0) as u16,
        rows: (h / f32::from(line_h)).floor().clamp(1.0, 500.0) as u16,
    }
}

pub fn prepare(
    grid: &Grid,
    lines: &[Vec<Cell>],
    cursor: Option<(usize, usize)>,
    window: &Window,
) -> Frame {
    let mut quads = Vec::new();
    let mut rows = Vec::with_capacity(lines.len());
    for (r, line) in lines.iter().enumerate() {
        let y = grid.origin.y + grid.line_h * r as f32;
        // one quad per run of the same non-default background
        let mut run: Option<(usize, Hsla)> = None;
        let bgs = line
            .iter()
            .map(|c| c.colors().1)
            .chain(std::iter::once(CellColor::Background));
        for (col, bg) in bgs.enumerate() {
            let color = (bg != CellColor::Background).then(|| cell_color(bg));
            if run.map(|(_, c)| Some(c)) != Some(color) {
                if let Some((start, c)) = run {
                    let at = point(grid.origin.x + grid.cell_w * start as f32, y);
                    quads.push(fill(
                        Bounds::new(at, size(grid.cell_w * (col - start) as f32, grid.line_h)),
                        c,
                    ));
                }
                run = color.map(|c| (col, c));
            }
        }
        rows.push(shape_row(line, grid, window));
    }
    let cursor = cursor.filter(|&(r, _)| r < lines.len()).map(|(r, c)| {
        let at = point(
            grid.origin.x + grid.cell_w * c as f32,
            grid.origin.y + grid.line_h * r as f32,
        );
        fill(
            Bounds::new(at, size(grid.cell_w, grid.line_h)),
            hsla(THEME.cursor),
        )
    });
    Frame {
        quads,
        rows,
        origin: grid.origin,
        cell_w: grid.cell_w,
        line_h: grid.line_h,
        cursor,
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
        if let Some(c) = frame.cursor.take() {
            window.paint_quad(c);
        }
    });
}

// ASCII runs shape together because a mono font keeps them on the grid. Any other glyph may come
// from a fallback font with its own advance (box drawing, emoji, CJK), so it gets its own segment
// pinned at its column; otherwise the rest of the row drifts off the grid.
fn shape_row(line: &[Cell], grid: &Grid, window: &Window) -> Vec<(usize, ShapedLine)> {
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut start = 0;
    let flush = |text: &mut String,
                 runs: &mut Vec<TextRun>,
                 start: usize,
                 out: &mut Vec<(usize, ShapedLine)>| {
        if text.trim_end().is_empty() {
            text.clear();
            runs.clear();
            return;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(std::mem::take(text)),
            px(FONT_SIZE),
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
        if pinned {
            flush(&mut text, &mut runs, start, &mut segments);
        }
        if text.is_empty() {
            start = col;
        }
        let mut color = cell_color(cell.colors().0);
        if cell.dim {
            color.a *= 0.6;
        }
        let mut f = grid.font.clone();
        f.weight = if cell.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        text.push(cell.ch);
        let len = cell.ch.len_utf8();
        match runs.last_mut() {
            Some(last) if last.color == color && last.font == f => last.len += len,
            _ => runs.push(TextRun {
                len,
                font: f,
                color,
                background_color: None,
                underline: None,
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
