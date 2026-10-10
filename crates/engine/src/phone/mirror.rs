// A terminal session's screen for the phone. The desktop's own emulator lives in the UI, which
// the engine can't reach, so the engine keeps a second one for each terminal a phone watches,
// fed the same PTY bytes. It starts from the last bytes the session printed (`Ring`), which is
// enough for an agent's recent output; older lines are on the desktop.
//
// The phone holds a copy of every line, scrollback included. A frame sends only what changed:
// lines are numbered from the oldest line kept, so a line keeps its number as output scrolls
// it up into history, until history is full and old lines fall off the top (`drop`).

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};
use hyprspace_proto::phone::{Span, TermFrame};

/// Scrollback the phone gets. A long agent session scrolls past this, and the rest stays on the
/// desktop.
const HISTORY: usize = 2000;
/// What a session keeps of its output for a mirror that starts late.
const RING: usize = 512 * 1024;

/// The last bytes a terminal session printed.
#[derive(Default)]
pub struct Ring(VecDeque<u8>);

impl Ring {
    pub fn push(&mut self, bytes: &[u8]) {
        if bytes.len() >= RING {
            self.0.clear();
            self.0.extend(&bytes[bytes.len() - RING..]);
            return;
        }
        let over = (self.0.len() + bytes.len()).saturating_sub(RING);
        self.0.drain(..over);
        self.0.extend(bytes);
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.0.iter().copied().collect()
    }
}

struct Size {
    cols: u16,
    rows: u16,
}

impl Dimensions for Size {
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

pub struct Mirror {
    term: Term<VoidListener>,
    parser: Processor,
    /// A hash of each line, oldest first, as of the last `refresh`.
    hashes: Vec<u64>,
    /// History's length at the last `refresh`.
    history: usize,
    /// Fed or resized since the last `refresh`.
    dirty: bool,
}

/// What one phone was last sent of a mirror.
#[derive(Default)]
pub struct Sent {
    hashes: Vec<u64>,
    started: bool,
    size: (u16, u16),
    cursor: Option<(u32, u16)>,
    fit: bool,
    paste: bool,
}

impl Mirror {
    /// A mirror at the session's size, caught up on what the session printed so far.
    pub fn new(cols: u16, rows: u16, ring: &Ring) -> Self {
        let config = Config {
            scrolling_history: HISTORY,
            ..Config::default()
        };
        let size = Size {
            cols: cols.max(2),
            rows: rows.max(1),
        };
        let mut m = Self {
            term: Term::new(config, &size, VoidListener),
            parser: Processor::new(),
            hashes: Vec::new(),
            history: 0,
            dirty: true,
        };
        m.feed(&ring.bytes());
        m
    }

    /// Cheap: the lines are looked at when the next frame is made.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        self.dirty = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        if (cols, rows) == self.size() {
            return;
        }
        self.term.resize(Size {
            cols: cols.max(2),
            rows: rows.max(1),
        });
        // a reflow rewrites every line
        self.hashes.clear();
        self.history = 0;
        self.dirty = true;
    }

    pub fn size(&self) -> (u16, u16) {
        (self.term.columns() as u16, self.term.screen_lines() as u16)
    }

    pub fn bracketed_paste(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Brings the line hashes up to date. History lines never change once there, so only the
    /// screen and the lines that just scrolled into history are hashed again; once history is
    /// full, the lines that fell off its top are found by where the old ones sit now.
    fn refresh(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let grid = self.term.grid();
        let history = grid.history_size();
        let total = history + grid.screen_lines();
        let old = self.history;
        let keep = if history < old {
            // history was cleared
            0
        } else if history == HISTORY && !self.hashes.is_empty() {
            match self.shift_at_cap() {
                Some(shift) => {
                    self.hashes.drain(..shift);
                    old.saturating_sub(shift)
                }
                None => 0,
            }
        } else {
            old
        };
        self.hashes.truncate(keep.min(history));
        for i in self.hashes.len()..total {
            self.hashes.push(self.hash_line(i));
        }
        self.history = history;
    }

    /// How many lines fell off the top of a full history since the last refresh: the smallest
    /// shift that lines up the old line hashes with the lines there now, checked at a few more
    /// places so a run of blank lines can't fool it. None when nothing lines up, which costs
    /// hashing every line again.
    fn shift_at_cap(&self) -> Option<usize> {
        let sample = 16.min(self.history);
        let now: Vec<u64> = (0..sample).map(|i| self.hash_line(i)).collect();
        let spots = |s: usize| {
            let kept = self.history.min(self.hashes.len().saturating_sub(s));
            [kept / 4, kept / 2, kept * 3 / 4, kept.saturating_sub(1)]
        };
        (0..=self.hashes.len().saturating_sub(sample)).find(|&s| {
            self.hashes[s..s + sample] == now[..]
                && spots(s)
                    .iter()
                    .all(|&i| self.hashes[s + i] == self.hash_line(i))
        })
    }

    fn grid_line(&self, i: usize) -> Line {
        Line(i as i32 - self.term.grid().history_size() as i32)
    }

    fn hash_line(&self, i: usize) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for s in self.spans(i) {
            s.t.hash(&mut h);
            s.fg.hash(&mut h);
            s.bg.hash(&mut h);
            s.s.hash(&mut h);
        }
        h.finish()
    }

    /// Line `i` from the oldest kept, as runs of cells drawn alike, trailing blanks left off.
    fn spans(&self, i: usize) -> Vec<Span> {
        let grid = self.term.grid();
        let row = &grid[self.grid_line(i)];
        let cols = grid.columns();
        let mut spans: Vec<Span> = Vec::new();
        for c in 0..cols {
            let cell = &row[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let fg = color(cell.fg);
            let bg = color(cell.bg);
            let s = style(cell.flags, cell.fg);
            let mut text = String::new();
            text.push(cell.c);
            if let Some(zw) = cell.zerowidth() {
                text.extend(zw);
            }
            match spans.last_mut() {
                Some(last) if last.fg == fg && last.bg == bg && last.s == s => {
                    last.t.push_str(&text)
                }
                _ => spans.push(Span { t: text, fg, bg, s }),
            }
        }
        // the blank tail of a line is only noise
        while let Some(last) = spans.last_mut() {
            if last.bg.is_some() || last.s & Span::INVERSE != 0 {
                break;
            }
            let trimmed = last.t.trim_end_matches(' ').len();
            if trimmed == 0 {
                spans.pop();
            } else {
                last.t.truncate(trimmed);
                break;
            }
        }
        spans
    }

    /// The frame that brings `sent` up to date, or None when nothing changed.
    pub fn frame(&mut self, thread: u64, sent: &mut Sent, fit: bool) -> Option<TermFrame> {
        self.refresh();
        let size = self.size();
        let cursor = self.cursor();
        let paste = self.bracketed_paste();
        let reset = !sent.started;
        if reset {
            *sent = Sent {
                started: true,
                ..Sent::default()
            };
        }
        // lines that fell off the top of a full history fall off the phone's copy too
        let mut drop = 0;
        if !reset && self.history == HISTORY && sent.hashes.len() >= self.hashes.len() {
            let sample = 16.min(self.hashes.len());
            let max = sent.hashes.len().saturating_sub(sample);
            if let Some(s) =
                (0..=max).find(|&s| sent.hashes[s..s + sample] == self.hashes[..sample])
            {
                drop = s;
                sent.hashes.drain(..s);
            }
        }
        let len = self.hashes.len();
        sent.hashes.resize(len, u64::MAX);
        let mut lines = Vec::new();
        for (i, h) in self.hashes.iter().enumerate() {
            if sent.hashes[i] != *h {
                sent.hashes[i] = *h;
                lines.push((i as u32, self.spans(i)));
            }
        }
        let same = !reset
            && drop == 0
            && lines.is_empty()
            && sent.size == size
            && sent.cursor == cursor
            && sent.fit == fit
            && sent.paste == paste;
        sent.size = size;
        sent.cursor = cursor;
        sent.fit = fit;
        sent.paste = paste;
        (!same).then_some(TermFrame {
            thread,
            cols: size.0,
            rows: size.1,
            reset,
            drop: drop as u32,
            len: len as u32,
            lines,
            cursor,
            fit,
            paste,
        })
    }

    fn cursor(&self) -> Option<(u32, u16)> {
        if !self.term.mode().contains(TermMode::SHOW_CURSOR) {
            return None;
        }
        let p = self.term.grid().cursor.point;
        let line = self.history as i32 + p.line.0;
        Some((line.max(0) as u32, p.column.0 as u16))
    }
}

fn color(c: Color) -> Option<u32> {
    match c {
        Color::Spec(Rgb { r, g, b }) => {
            Some(Span::RGB | (r as u32) << 16 | (g as u32) << 8 | b as u32)
        }
        Color::Indexed(i) => Some(i as u32),
        Color::Named(n) => {
            let i = n as usize;
            if i < 16 {
                return Some(i as u32);
            }
            match n {
                NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite => Some((i - NamedColor::DimBlack as usize) as u32),
                _ => None,
            }
        }
    }
}

fn style(f: Flags, fg: Color) -> u8 {
    let mut s = 0;
    for (flag, bit) in [
        (Flags::BOLD, Span::BOLD),
        (Flags::ITALIC, Span::ITALIC),
        (Flags::INVERSE, Span::INVERSE),
        (Flags::DIM, Span::DIM),
        (Flags::STRIKEOUT, Span::STRIKE),
        (Flags::HIDDEN, Span::HIDDEN),
    ] {
        if f.contains(flag) {
            s |= bit;
        }
    }
    if f.intersects(Flags::ALL_UNDERLINES) {
        s |= Span::UNDERLINE;
    }
    if matches!(
        fg,
        Color::Named(
            NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite
                | NamedColor::DimForeground
        )
    ) {
        s |= Span::DIM;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays frames onto a copy the way the phone does.
    #[derive(Default)]
    struct Phone(Vec<String>);

    impl Phone {
        fn apply(&mut self, f: &TermFrame) {
            if f.reset {
                self.0.clear();
            }
            self.0.drain(..(f.drop as usize).min(self.0.len()));
            self.0.resize(f.len as usize, String::new());
            for (i, spans) in &f.lines {
                self.0[*i as usize] = spans.iter().map(|s| s.t.as_str()).collect();
            }
        }
    }

    fn expect(m: &mut Mirror) -> Vec<String> {
        m.refresh();
        (0..m.hashes.len())
            .map(|i| m.spans(i).iter().map(|s| s.t.as_str()).collect())
            .collect()
    }

    #[test]
    fn scrolling_output_sends_only_new_lines() {
        let mut m = Mirror::new(20, 4, &Ring::default());
        let mut sent = Sent::default();
        let mut phone = Phone::default();
        m.feed(b"one\r\ntwo\r\n");
        phone.apply(&m.frame(1, &mut sent, false).unwrap());
        assert_eq!(phone.0[..2], ["one", "two"]);
        m.feed(b"three\r\nfour\r\nfive\r\nsix\r\n");
        let f = m.frame(1, &mut sent, false).unwrap();
        // one and two only moved up; their numbers held
        assert!(f.lines.iter().all(|(i, _)| *i >= 2), "{:?}", f.lines);
        phone.apply(&f);
        assert_eq!(phone.0, expect(&mut m));
        assert!(m.frame(1, &mut sent, false).is_none());
    }

    #[test]
    fn a_full_history_drops_lines_off_the_top() {
        let mut m = Mirror::new(10, 3, &Ring::default());
        let mut sent = Sent::default();
        let mut phone = Phone::default();
        for i in 0..HISTORY + 10 {
            m.feed(format!("l{i}\r\n").as_bytes());
        }
        phone.apply(&m.frame(1, &mut sent, false).unwrap());
        assert_eq!(phone.0, expect(&mut m));
        for i in 0..50 {
            m.feed(format!("m{i}\r\n").as_bytes());
        }
        let f = m.frame(1, &mut sent, false).unwrap();
        assert_eq!(f.drop, 50);
        assert!(f.lines.len() < 60, "{}", f.lines.len());
        phone.apply(&f);
        assert_eq!(phone.0, expect(&mut m));
    }

    #[test]
    fn a_resize_and_a_redraw_still_end_up_the_same() {
        let mut m = Mirror::new(30, 5, &Ring::default());
        let mut sent = Sent::default();
        let mut phone = Phone::default();
        m.feed(b"\x1b[31mred\x1b[0m and a long line that wraps once narrower\r\n");
        phone.apply(&m.frame(1, &mut sent, false).unwrap());
        m.resize(12, 5);
        m.feed(b"\x1b[2J\x1b[Hfresh");
        phone.apply(&m.frame(1, &mut sent, true).unwrap());
        assert_eq!(phone.0, expect(&mut m));
        let f = m.frame(1, &mut sent, true);
        assert!(f.is_none());
    }

    #[test]
    fn colors_and_styles_come_through() {
        let mut m = Mirror::new(20, 2, &Ring::default());
        m.feed(b"\x1b[1;32mok\x1b[0m \x1b[38;2;1;2;3mrgb\x1b[0m");
        m.refresh();
        let spans = m.spans(m.hashes.len() - 2);
        assert_eq!(spans[0].t, "ok");
        assert_eq!(spans[0].fg, Some(2));
        assert_eq!(spans[0].s, Span::BOLD);
        assert_eq!(spans[2].fg, Some(Span::RGB | 0x010203));
    }

    #[test]
    fn the_ring_keeps_the_newest_bytes() {
        let mut r = Ring::default();
        r.push(&vec![b'a'; RING]);
        r.push(b"bc");
        let b = r.bytes();
        assert_eq!(b.len(), RING);
        assert!(b.ends_with(b"abc"));
    }
}
