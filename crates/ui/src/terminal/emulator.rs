// alacritty_terminal's Term plus the vte parser as a pure state machine: PTY bytes in, grid
// snapshots out, no I/O and no gpui, so escape handling, selection, scrollback and search can be
// tested with byte strings. Term owns the selection, so it stays on its text as output scrolls.
// Adapted from zeron's crates/ui/src/terminal/emulator.rs (MIT, see THIRD_PARTY_NOTICES.md).

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line};
use alacritty_terminal::selection::{Selection, SelectionRange};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{
    Color as AnsiColor, CursorShape, CursorStyle, NamedColor, Processor, Rgb,
};

pub use alacritty_terminal::index::{Point, Side};
pub use alacritty_terminal::selection::SelectionType;

const SCROLLBACK_LINES: usize = 10_000;
/// Past this many matches the find bar stops counting; a search for "e" in a full scrollback
/// would otherwise walk every line on each keystroke.
const MAX_MATCHES: usize = 1000;

#[derive(Clone, Copy)]
struct GridSize {
    cols: u16,
    rows: u16,
}

impl GridSize {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(2),
            rows: rows.max(1),
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// A cell color before it meets the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    Foreground,
    Background,
    /// 0-15 ANSI, 16-231 color cube, 232-255 grayscale ramp.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

fn map_color(color: AnsiColor) -> CellColor {
    match color {
        AnsiColor::Spec(Rgb { r, g, b }) => CellColor::Rgb(r, g, b),
        AnsiColor::Indexed(ix) => CellColor::Indexed(ix),
        AnsiColor::Named(named) => {
            let ix = named as usize;
            if ix < 16 {
                return CellColor::Indexed(ix as u8);
            }
            match named {
                NamedColor::Background => CellColor::Background,
                NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite => {
                    CellColor::Indexed((ix - NamedColor::DimBlack as usize) as u8)
                }
                _ => CellColor::Foreground,
            }
        }
    }
}

/// What a find or a hovered link marks on a cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    None,
    Found,
    /// The match the find bar is on.
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub wide: bool,
    /// The right half of a wide char: painted as background only.
    pub spacer: bool,
    pub selected: bool,
    pub mark: Mark,
    /// Under the hovered link.
    pub link: bool,
}

impl Cell {
    pub fn colors(&self) -> (CellColor, CellColor) {
        if self.inverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        }
    }
}

/// The cursor in viewport cells, and how the program asked for it to look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub shape: Shape,
    pub blink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Block,
    Bar,
    Underline,
}

/// What the view marks besides the selection.
#[derive(Default)]
pub struct Marks<'a> {
    pub found: &'a [Match],
    pub current: Option<usize>,
    pub link: Option<&'a Match>,
}

fn contains(m: &Match, p: Point) -> bool {
    *m.start() <= p && p <= *m.end()
}

/// A logical line (rows joined across soft wraps) as text, and the grid point of each char.
pub struct LineText {
    pub text: String,
    pub points: Vec<Point>,
}

// Term reports through `&self`, so events queue here and `feed` drains them.
#[derive(Default, Clone)]
struct Capture(Rc<RefCell<Vec<Event>>>);

impl EventListener for Capture {
    fn send_event(&self, event: Event) {
        self.0.borrow_mut().push(event);
    }
}

/// A color by xterm's numbering: 0-255 the palette, 256 the foreground, 257 the background,
/// 258 the cursor.
pub type Palette = fn(usize) -> (u8, u8, u8);

pub struct Emulator {
    term: Term<Capture>,
    parser: Processor,
    capture: Capture,
    palette: Palette,
}

impl Emulator {
    /// `blink` is the cursor's default before a program asks for a style of its own; `palette`
    /// answers programs that ask what the colors are.
    pub fn new(cols: u16, rows: u16, blink: bool, palette: Palette) -> Self {
        let capture = Capture::default();
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            default_cursor_style: CursorStyle {
                shape: CursorShape::Block,
                blinking: blink,
            },
            ..Config::default()
        };
        let term = Term::new(config, &GridSize::new(cols, rows), capture.clone());
        Self {
            term,
            parser: Processor::new(),
            capture,
            palette,
        }
    }

    /// Returns the bytes the terminal must write back (answers to cursor, device and color
    /// queries). A TUI like claude waits for those answers on startup, so dropping them hangs it,
    /// and the background color is how a CLI picks its light or dark look.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        let mut out = Vec::new();
        for event in self.capture.0.borrow_mut().drain(..) {
            match event {
                Event::PtyWrite(text) => out.extend_from_slice(text.as_bytes()),
                Event::ColorRequest(ix, answer) => {
                    let (r, g, b) = (self.palette)(ix);
                    out.extend_from_slice(answer(Rgb { r, g, b }).as_bytes());
                }
                _ => {}
            }
        }
        out
    }

    /// Reflows the grid. Scrollback rewraps to the new width.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize::new(cols, rows));
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    fn mode(&self, m: TermMode) -> bool {
        self.term.mode().contains(m)
    }

    /// DECCKM: arrows send SS3 instead of CSI.
    pub fn app_cursor(&self) -> bool {
        self.mode(TermMode::APP_CURSOR)
    }

    /// Pastes go between `ESC [200~` and `ESC [201~`.
    pub fn bracketed_paste(&self) -> bool {
        self.mode(TermMode::BRACKETED_PASTE)
    }

    /// A full-screen program (vim, less) has the alternate screen, which has no scrollback.
    pub fn alt_screen(&self) -> bool {
        self.mode(TermMode::ALT_SCREEN)
    }

    /// On the alternate screen, the wheel becomes arrow keys (xterm's alternate scroll).
    pub fn alternate_scroll(&self) -> bool {
        self.mode(TermMode::ALTERNATE_SCROLL)
    }

    /// The program asked for mouse reports, in SGR form when `sgr`.
    pub fn mouse_mode(&self) -> Option<bool> {
        self.term
            .mode()
            .intersects(TermMode::MOUSE_MODE)
            .then(|| self.mode(TermMode::SGR_MOUSE))
    }

    // ---- scrollback ----

    /// Lines scrolled back into history; 0 is the live bottom.
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    pub fn history(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Positive scrolls up into history, negative toward the live bottom.
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn scroll_to_offset(&mut self, offset: usize) {
        let delta = offset.min(self.history()) as i64 - self.display_offset() as i64;
        self.scroll(delta.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }

    /// Scrolls just enough to put grid line `line` on screen.
    fn reveal(&mut self, line: Line) {
        let rows = self.term.screen_lines() as i32;
        let top = -(self.display_offset() as i32);
        if line.0 < top {
            self.scroll(top - line.0);
        } else if line.0 >= top + rows {
            self.scroll(top + rows - 1 - line.0);
        }
    }

    // ---- selection ----

    /// The grid point under a viewport cell. Anchors live in grid space so a selection stays on
    /// its text while the view scrolls.
    pub fn grid_point(&self, row: usize, col: usize) -> Point {
        Point::new(
            Line(row as i32 - self.display_offset() as i32),
            Column(col.min(self.term.columns().saturating_sub(1))),
        )
    }

    /// The viewport row of a grid line, if it is on screen.
    pub fn viewport_row(&self, line: Line) -> Option<usize> {
        let row = line.0 + self.display_offset() as i32;
        (0..self.term.screen_lines() as i32)
            .contains(&row)
            .then_some(row as usize)
    }

    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    pub fn update_selection(&mut self, point: Point, side: Side) {
        if let Some(s) = self.term.selection.as_mut() {
            s.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// None when nothing is selected, including the empty selection a bare click leaves.
    pub fn selection_text(&self) -> Option<String> {
        self.term.selection_to_string().filter(|s| !s.is_empty())
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        self.term
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&self.term))
    }

    // ---- search ----

    /// Every match of `query` in the scrollback and screen, top to bottom, up to a cap. Smart
    /// case unless `case`: an uppercase letter makes the search case sensitive.
    pub fn find(&self, query: &str, case: bool) -> Vec<Match> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut pattern = escape(query);
        if case {
            pattern.insert_str(0, "(?-i)");
        }
        let Ok(mut regex) = RegexSearch::new(&pattern) else {
            return Vec::new();
        };
        let start = Point::new(self.term.topmost_line(), Column(0));
        let end = Point::new(self.term.bottommost_line(), self.term.last_column());
        RegexIter::new(start, end, Direction::Right, &self.term, &mut regex)
            .take(MAX_MATCHES)
            .collect()
    }

    /// Scrolls `m` into view.
    pub fn reveal_match(&mut self, m: &Match) {
        self.reveal(m.start().line);
    }

    // ---- snapshots ----

    pub fn lines(&self, marks: &Marks) -> Vec<Vec<Cell>> {
        let selection = self.selection_range();
        let offset = self.display_offset() as i32;
        let grid = self.term.grid();
        let cols = self.term.columns();
        (0..self.term.screen_lines())
            .map(|r| {
                let line = Line(r as i32 - offset);
                let row = &grid[line];
                // only the matches that touch this row, so each cell checks a handful
                let found: Vec<(usize, &Match)> = marks
                    .found
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.start().line <= line && line <= m.end().line)
                    .collect();
                (0..cols)
                    .map(|col| {
                        let c = &row[Column(col)];
                        let p = Point::new(line, Column(col));
                        let mark = found.iter().find(|(_, m)| contains(m, p)).map_or(
                            Mark::None,
                            |(i, _)| {
                                if Some(*i) == marks.current {
                                    Mark::Current
                                } else {
                                    Mark::Found
                                }
                            },
                        );
                        Cell {
                            ch: if c.flags.intersects(Flags::HIDDEN) {
                                ' '
                            } else {
                                c.c
                            },
                            fg: map_color(c.fg),
                            bg: map_color(c.bg),
                            bold: c.flags.intersects(Flags::BOLD),
                            dim: c.flags.intersects(Flags::DIM),
                            italic: c.flags.intersects(Flags::ITALIC),
                            underline: c.flags.intersects(Flags::ALL_UNDERLINES),
                            inverse: c.flags.intersects(Flags::INVERSE),
                            wide: c.flags.intersects(Flags::WIDE_CHAR),
                            spacer: c.flags.intersects(
                                Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER,
                            ),
                            selected: selection.is_some_and(|s| s.contains(p)),
                            mark,
                            link: marks.link.is_some_and(|m| contains(m, p)),
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// The cursor in viewport cells, or None when the program hid it or it is scrolled away.
    pub fn cursor(&self) -> Option<Cursor> {
        let content = self.term.renderable_content();
        let shape = match content.cursor.shape {
            CursorShape::Hidden => return None,
            CursorShape::Beam => Shape::Bar,
            CursorShape::Underline => Shape::Underline,
            CursorShape::Block | CursorShape::HollowBlock => Shape::Block,
        };
        let row = self.viewport_row(content.cursor.point.line)?;
        Some(Cursor {
            row,
            col: content.cursor.point.column.0,
            shape,
            blink: self.term.cursor_style().blinking,
        })
    }

    /// The logical line through viewport `row`: its rows joined across soft wraps, the way a long
    /// path or URL wraps, with the grid point of every char.
    pub fn line_text(&self, row: usize) -> LineText {
        let at = self.grid_point(row, 0);
        let first = self.term.line_search_left(at).line;
        let last = self.term.line_search_right(at).line;
        let grid = self.term.grid();
        let mut text = String::new();
        let mut points = Vec::new();
        for l in first.0..=last.0 {
            let row = &grid[Line(l)];
            for col in 0..self.term.columns() {
                let c = &row[Column(col)];
                if c.flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                text.push(c.c);
                points.push(Point::new(Line(l), Column(col)));
            }
        }
        LineText { text, points }
    }

    #[cfg(test)]
    pub fn row_text(&self, row: usize) -> String {
        let text: String = self.lines(&Marks::default())[row]
            .iter()
            .filter(|c| !c.spacer)
            .map(|c| c.ch)
            .collect();
        text.trim_end().to_string()
    }
}

/// `query` as a literal regex.
fn escape(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for c in query.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(ix: usize) -> (u8, u8, u8) {
        match ix {
            257 => (0x16, 0x16, 0x16),
            _ => (0xee, 0xee, 0xee),
        }
    }

    fn emu(cols: u16, rows: u16) -> Emulator {
        Emulator::new(cols, rows, true, palette)
    }

    #[test]
    fn prints_and_moves_the_cursor() {
        let mut e = emu(20, 4);
        e.feed(b"hello\r\nworld");
        assert_eq!(e.row_text(0), "hello");
        assert_eq!(e.row_text(1), "world");
        let c = e.cursor().unwrap();
        assert_eq!((c.row, c.col, c.shape, c.blink), (1, 5, Shape::Block, true));
    }

    #[test]
    fn answers_cursor_position_queries() {
        let mut e = emu(20, 4);
        assert_eq!(e.feed(b"ab\x1b[6n"), b"\x1b[1;3R");
    }

    #[test]
    fn answers_color_queries_from_the_palette() {
        let mut e = emu(20, 4);
        assert_eq!(e.feed(b"\x1b]11;?\x07"), b"\x1b]11;rgb:1616/1616/1616\x07");
    }

    #[test]
    fn reads_sgr_colors_and_attributes() {
        let mut e = emu(20, 4);
        e.feed(b"\x1b[31mx\x1b[38;2;1;2;3my\x1b[0;4;3mz");
        let row = &e.lines(&Marks::default())[0];
        assert_eq!(row[0].fg, CellColor::Indexed(1));
        assert_eq!(row[1].fg, CellColor::Rgb(1, 2, 3));
        assert!(row[2].underline && row[2].italic);
    }

    #[test]
    fn programs_pick_the_cursor_shape_and_blink() {
        let mut e = Emulator::new(10, 2, false, palette);
        assert!(!e.cursor().unwrap().blink);
        // DECSCUSR 5: blinking bar
        e.feed(b"\x1b[5 q");
        let c = e.cursor().unwrap();
        assert_eq!((c.shape, c.blink), (Shape::Bar, true));
        e.feed(b"\x1b[4 q");
        assert_eq!(e.cursor().unwrap().shape, Shape::Underline);
        e.feed(b"\x1b[?25l");
        assert_eq!(e.cursor(), None);
    }

    #[test]
    fn scrollback_scrolls_and_keeps_its_place_while_output_streams() {
        let mut e = emu(10, 3);
        for i in 1..=8 {
            e.feed(format!("line{i}\r\n").as_bytes());
        }
        assert_eq!(e.history(), 6);
        e.scroll(2);
        assert_eq!(e.row_text(0), "line5");
        assert_eq!(e.cursor(), None, "the cursor is below the view");
        // more output arrives; the view stays on the same text
        e.feed(b"line9\r\nline10\r\n");
        assert_eq!(e.row_text(0), "line5");
        e.scroll_to_bottom();
        assert_eq!(e.display_offset(), 0);
        e.scroll_to_offset(usize::MAX);
        assert_eq!(e.row_text(0), "line1");
    }

    #[test]
    fn the_alternate_screen_has_no_scrollback_and_restores_the_shell() {
        let mut e = emu(20, 3);
        e.feed(b"primary\r\n");
        assert!(!e.alt_screen() && e.alternate_scroll());
        e.feed(b"\x1b[?1049h\x1b[H");
        assert!(e.alt_screen());
        for i in 0..10 {
            e.feed(format!("alt{i}\r\n").as_bytes());
        }
        assert_eq!(e.history(), 0);
        e.feed(b"\x1b[?1049l");
        assert!(!e.alt_screen());
        assert_eq!(e.row_text(0), "primary");
    }

    #[test]
    fn modes_toggle() {
        let mut e = emu(10, 2);
        e.feed(b"\x1b[?1h\x1b[?2004h");
        assert!(e.app_cursor() && e.bracketed_paste());
        assert_eq!(e.mouse_mode(), None);
        e.feed(b"\x1b[?1000h\x1b[?1006h");
        assert_eq!(e.mouse_mode(), Some(true));
    }

    #[test]
    fn resize_reflows_long_lines() {
        let mut e = emu(10, 4);
        e.feed(b"abcdefghijKLM");
        assert_eq!(e.row_text(1), "KLM");
        e.resize(20, 4);
        assert_eq!(e.row_text(0), "abcdefghijKLM");
        e.resize(5, 4);
        assert_eq!(e.size(), (5, 4));
        // the cursor's row stays put, so the rows wrapped above it move into history
        assert_eq!(e.row_text(0), "KLM");
        e.scroll_to_offset(2);
        assert_eq!(e.row_text(1), "fghij");
        assert_eq!(e.line_text(0).text.trim_end(), "abcdefghijKLM");
    }

    #[test]
    fn drag_word_and_line_selections() {
        let mut e = emu(30, 3);
        e.feed(b"alpha beta gamma\r\nsecond row");
        e.start_selection(SelectionType::Simple, e.grid_point(0, 0), Side::Left);
        e.update_selection(e.grid_point(0, 4), Side::Right);
        assert_eq!(e.selection_text().as_deref(), Some("alpha"));
        let row = &e.lines(&Marks::default())[0];
        assert!(row[..5].iter().all(|c| c.selected) && !row[5].selected);
        e.start_selection(SelectionType::Semantic, e.grid_point(0, 7), Side::Left);
        assert_eq!(e.selection_text().as_deref(), Some("beta"));
        e.start_selection(SelectionType::Lines, e.grid_point(1, 3), Side::Left);
        assert_eq!(e.selection_text().as_deref(), Some("second row\n"));
        // a bare click selects nothing
        e.start_selection(SelectionType::Simple, e.grid_point(0, 2), Side::Left);
        assert_eq!(e.selection_text(), None);
        e.clear_selection();
        assert_eq!(e.selection_text(), None);
    }

    #[test]
    fn a_selection_follows_its_text_when_output_scrolls() {
        let mut e = emu(10, 3);
        e.feed(b"target\r\n");
        e.start_selection(SelectionType::Simple, e.grid_point(0, 0), Side::Left);
        e.update_selection(e.grid_point(0, 5), Side::Right);
        e.feed(b"a\r\nb\r\nc\r\n");
        assert_eq!(e.selection_text().as_deref(), Some("target"));
    }

    #[test]
    fn find_walks_the_scrollback_and_marks_matches() {
        let mut e = emu(20, 3);
        e.feed(b"one Foo\r\ntwo\r\nthree foo\r\nfour (x)\r\n");
        let all = e.find("foo", false);
        assert_eq!(all.len(), 2);
        assert_eq!(e.find("Foo", true).len(), 1);
        // regex characters are literal
        assert_eq!(e.find("(x)", false).len(), 1);
        assert!(e.find("", false).is_empty());
        // the first match is in history; revealing it scrolls up
        e.reveal_match(&all[0]);
        assert_eq!(e.row_text(0), "one Foo");
        let marks = Marks {
            found: &all,
            current: Some(0),
            link: None,
        };
        let row = &e.lines(&marks)[0];
        assert_eq!(row[4].mark, Mark::Current);
        assert_eq!(row[0].mark, Mark::None);
    }

    #[test]
    fn line_text_joins_soft_wraps_and_skips_wide_spacers() {
        let mut e = emu(10, 4);
        e.feed("宽 /a/b/c/d/e/f.rs\r\nnext".as_bytes());
        let lt = e.line_text(1);
        assert!(lt.text.starts_with("宽 /a/b/c/d/e/f.rs"), "{:?}", lt.text);
        assert_eq!(lt.text.chars().count(), lt.points.len());
        assert_eq!(lt.points[1], Point::new(Line(0), Column(2)));
        assert_eq!(e.line_text(2).text.trim_end(), "next");
    }

    #[test]
    fn utf8_split_across_feeds_reassembles() {
        let mut e = emu(10, 2);
        let bytes = "é".as_bytes();
        e.feed(&bytes[..1]);
        e.feed(&bytes[1..]);
        assert_eq!(e.row_text(0), "é");
    }
}
