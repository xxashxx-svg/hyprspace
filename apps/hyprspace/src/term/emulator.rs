// alacritty_terminal's Term plus the vte parser as a pure state machine: PTY bytes in, grid
// snapshots out, no I/O and no gpui, so escape handling can be tested with byte strings.
// Adapted from zeron's crates/ui/src/terminal/emulator.rs (MIT, see THIRD_PARTY_NOTICES.md).

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb};

const SCROLLBACK_LINES: usize = 10_000;

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

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub dim: bool,
    pub inverse: bool,
    pub wide: bool,
    /// The right half of a wide char: painted as background only.
    pub spacer: bool,
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

// Term reports through `&self`, so events queue here and `feed` drains them.
#[derive(Default, Clone)]
struct Capture(Rc<RefCell<Vec<Event>>>);

impl EventListener for Capture {
    fn send_event(&self, event: Event) {
        self.0.borrow_mut().push(event);
    }
}

pub struct Emulator {
    term: Term<Capture>,
    parser: Processor,
    capture: Capture,
}

impl Emulator {
    pub fn new(cols: u16, rows: u16) -> Self {
        let capture = Capture::default();
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            ..Config::default()
        };
        let term = Term::new(config, &GridSize::new(cols, rows), capture.clone());
        Self {
            term,
            parser: Processor::new(),
            capture,
        }
    }

    /// Returns the bytes the terminal must write back (answers to cursor and device queries).
    /// A TUI like claude waits for those answers on startup, so dropping them hangs it.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        let mut out = Vec::new();
        for event in self.capture.0.borrow_mut().drain(..) {
            if let Event::PtyWrite(text) = event {
                out.extend_from_slice(text.as_bytes());
            }
        }
        out
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize::new(cols, rows));
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    /// DECCKM: arrows send SS3 instead of CSI.
    pub fn app_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    pub fn lines(&self) -> Vec<Vec<Cell>> {
        let grid = self.term.grid();
        (0..self.term.screen_lines())
            .map(|row| {
                let line = &grid[Line(row as i32)];
                (0..self.term.columns())
                    .map(|col| {
                        let c = &line[Column(col)];
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
                            inverse: c.flags.intersects(Flags::INVERSE),
                            wide: c.flags.intersects(Flags::WIDE_CHAR),
                            spacer: c.flags.intersects(
                                Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER,
                            ),
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// (row, col) of the cursor, or None when the program hid it.
    pub fn cursor(&self) -> Option<(usize, usize)> {
        let content = self.term.renderable_content();
        if content.cursor.shape == CursorShape::Hidden {
            return None;
        }
        let Point { line, column } = content.cursor.point;
        (line.0 >= 0).then_some((line.0 as usize, column.0))
    }

    #[cfg(test)]
    fn row_text(&self, row: usize) -> String {
        let text: String = self.lines()[row]
            .iter()
            .filter(|c| !c.spacer)
            .map(|c| c.ch)
            .collect();
        text.trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_and_moves_the_cursor() {
        let mut emu = Emulator::new(20, 4);
        emu.feed(b"hello\r\nworld");
        assert_eq!(emu.row_text(0), "hello");
        assert_eq!(emu.row_text(1), "world");
        assert_eq!(emu.cursor(), Some((1, 5)));
    }

    #[test]
    fn answers_cursor_position_queries() {
        let mut emu = Emulator::new(20, 4);
        assert_eq!(emu.feed(b"ab\x1b[6n"), b"\x1b[1;3R");
    }

    #[test]
    fn reads_sgr_colors() {
        let mut emu = Emulator::new(20, 4);
        emu.feed(b"\x1b[31mx\x1b[38;2;1;2;3my");
        let row = &emu.lines()[0];
        assert_eq!(row[0].fg, CellColor::Indexed(1));
        assert_eq!(row[1].fg, CellColor::Rgb(1, 2, 3));
    }
}
