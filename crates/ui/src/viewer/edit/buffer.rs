// The text being edited: one string with each line's start kept beside it, and the edits made to
// it, for undo. Offsets are UTF-8 bytes into the whole text. A line's own text leaves out its
// line break, so a file with Windows line ends keeps them: a typed or pasted break is written in
// the file's own style.

use std::ops::Range;
use std::time::{Duration, Instant};

use unicode_segmentation::UnicodeSegmentation;

/// Typing this close together undoes as one step.
const RUN: Duration = Duration::from_millis(1000);
const MAX_UNDO: usize = 500;

/// A selection: where it started and where the cursor is.
pub type Sel = (usize, usize);

#[derive(Debug, Clone)]
struct Edit {
    at: usize,
    removed: String,
    inserted: String,
    before: Sel,
    after: Sel,
    when: Instant,
    /// Closed to the next keystroke: an undo point was set after it.
    sealed: bool,
}

impl Edit {
    /// A run of typing, or of deleting, that the next keystroke can join.
    fn joins(&self, next: &Edit) -> bool {
        if self.sealed || next.when.duration_since(self.when) > RUN {
            return false;
        }
        let plain = |s: &str| !s.contains('\n');
        let typing = self.removed.is_empty()
            && next.removed.is_empty()
            && plain(&self.inserted)
            && plain(&next.inserted)
            && next.at == self.at + self.inserted.len();
        let deleting = self.inserted.is_empty()
            && next.inserted.is_empty()
            && plain(&self.removed)
            && plain(&next.removed)
            && next.at + next.removed.len() == self.at;
        typing || deleting
    }
}

pub struct Buffer {
    text: String,
    /// Where each line starts.
    starts: Vec<usize>,
    /// The file's line break.
    nl: &'static str,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl Buffer {
    pub fn new(text: String) -> Self {
        let nl = match text.find('\n') {
            Some(i) if text[..i].ends_with('\r') => "\r\n",
            _ => "\n",
        };
        let mut b = Self {
            text,
            starts: Vec::new(),
            nl,
            undo: Vec::new(),
            redo: Vec::new(),
        };
        b.index();
        b
    }

    fn index(&mut self) {
        self.starts = std::iter::once(0)
            .chain(self.text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn lines(&self) -> usize {
        self.starts.len()
    }

    /// Line `i`'s text, without its break.
    pub fn line_range(&self, i: usize) -> Range<usize> {
        let start = self.starts[i];
        let end = match self.starts.get(i + 1) {
            Some(next) => next - 1,
            None => self.text.len(),
        };
        let end = if self.text[start..end].ends_with('\r') {
            end - 1
        } else {
            end
        };
        start..end
    }

    pub fn line(&self, i: usize) -> &str {
        &self.text[self.line_range(i)]
    }

    pub fn line_of(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|&s| s <= offset)
            .saturating_sub(1)
    }

    /// `offset` as a line and a byte column in it.
    pub fn pos(&self, offset: usize) -> (usize, usize) {
        let line = self.line_of(offset);
        (line, offset - self.starts[line])
    }

    /// The offset at `col` bytes into `line`, kept inside the line and on a character.
    pub fn offset(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.lines() - 1);
        let r = self.line_range(line);
        let mut at = (r.start + col).min(r.end);
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// Writes `text` over `range` and remembers it for undo. Line breaks in `text` take the
    /// file's style. Returns where the inserted text ends.
    pub fn replace(&mut self, range: Range<usize>, text: &str, before: Sel) -> usize {
        let text = self.breaks(text);
        let end = range.start + text.len();
        let edit = Edit {
            at: range.start,
            removed: self.text[range.clone()].to_string(),
            inserted: text,
            before,
            after: (end, end),
            when: Instant::now(),
            sealed: false,
        };
        self.apply(range, &edit.inserted);
        self.redo.clear();
        match self.undo.last_mut() {
            Some(last) if last.joins(&edit) => {
                if edit.removed.is_empty() {
                    last.inserted.push_str(&edit.inserted);
                } else {
                    last.removed.insert_str(0, &edit.removed);
                    last.at = edit.at;
                }
                last.after = edit.after;
                last.when = edit.when;
            }
            _ => {
                self.undo.push(edit);
                if self.undo.len() > MAX_UNDO {
                    self.undo.remove(0);
                }
            }
        }
        end
    }

    /// Ends the current run of typing, so the next keystroke undoes on its own.
    pub fn seal(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.sealed = true;
        }
    }

    /// Sets the selection the last edit leaves behind, when it isn't just after its text.
    pub fn set_after(&mut self, sel: Sel) {
        if let Some(last) = self.undo.last_mut() {
            last.after = sel;
        }
    }

    fn breaks(&self, text: &str) -> String {
        let plain = text.replace("\r\n", "\n");
        if self.nl == "\n" {
            plain
        } else {
            plain.replace('\n', self.nl)
        }
    }

    fn apply(&mut self, range: Range<usize>, text: &str) {
        self.text.replace_range(range, text);
        self.index();
    }

    /// Takes back the last edit. Returns the selection to restore.
    pub fn undo(&mut self) -> Option<Sel> {
        let e = self.undo.pop()?;
        self.apply(e.at..e.at + e.inserted.len(), &e.removed);
        let sel = e.before;
        self.redo.push(e);
        Some(sel)
    }

    pub fn redo(&mut self) -> Option<Sel> {
        let mut e = self.redo.pop()?;
        self.apply(e.at..e.at + e.removed.len(), &e.inserted);
        let sel = e.after;
        // a redone edit never joins the next keystroke
        e.sealed = true;
        self.undo.push(e);
        Some(sel)
    }

    /// The start of the character before `offset`; a line's start steps back over the break.
    pub fn left(&self, offset: usize) -> usize {
        let (line, _) = self.pos(offset);
        let r = self.line_range(line);
        if offset <= r.start {
            return if line == 0 {
                0
            } else {
                self.line_range(line - 1).end
            };
        }
        self.text[r.start..offset]
            .grapheme_indices(true)
            .next_back()
            .map_or(r.start, |(i, _)| r.start + i)
    }

    /// The end of the character after `offset`; a line's end steps over the break.
    pub fn right(&self, offset: usize) -> usize {
        let (line, _) = self.pos(offset);
        let r = self.line_range(line);
        if offset >= r.end {
            return if line + 1 < self.lines() {
                self.starts[line + 1]
            } else {
                self.text.len()
            };
        }
        self.text[offset..r.end]
            .grapheme_indices(true)
            .nth(1)
            .map_or(r.end, |(i, _)| offset + i)
    }

    /// The start of the word before `offset`, as editors take it: spaces, then a run of word
    /// characters or of punctuation.
    pub fn word_left(&self, offset: usize) -> usize {
        let (line, _) = self.pos(offset);
        let r = self.line_range(line);
        if offset <= r.start {
            return self.left(offset);
        }
        let before = &self.text[r.start..offset];
        let trimmed = before.trim_end();
        let Some(last) = trimmed.chars().next_back() else {
            return r.start;
        };
        let kind = class(last);
        let start = trimmed
            .char_indices()
            .rev()
            .find(|(_, c)| class(*c) != kind)
            .map_or(0, |(i, c)| i + c.len_utf8());
        r.start + start
    }

    pub fn word_right(&self, offset: usize) -> usize {
        let (line, _) = self.pos(offset);
        let r = self.line_range(line);
        if offset >= r.end {
            return self.right(offset);
        }
        let after = &self.text[offset..r.end];
        let skipped = after.len() - after.trim_start().len();
        let rest = &after[skipped..];
        let Some(first) = rest.chars().next() else {
            return r.end;
        };
        let kind = class(first);
        let len = rest
            .char_indices()
            .find(|(_, c)| class(*c) != kind)
            .map_or(rest.len(), |(i, _)| i);
        offset + skipped + len
    }

    /// The word around `offset`, for a double-click.
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        let (line, _) = self.pos(offset);
        let r = self.line_range(line);
        let text = &self.text[r.clone()];
        let at = offset - r.start;
        let kind = text[at..]
            .chars()
            .next()
            .or_else(|| text[..at].chars().next_back())
            .map(class);
        let Some(kind) = kind else {
            return offset..offset;
        };
        let start = text[..at]
            .char_indices()
            .rev()
            .find(|(_, c)| class(*c) != kind)
            .map_or(0, |(i, c)| i + c.len_utf8());
        let end = text[at..]
            .char_indices()
            .find(|(_, c)| class(*c) != kind)
            .map_or(text.len(), |(i, _)| at + i);
        r.start + start..r.start + end
    }

    /// The line around `offset` with its break, for a triple-click.
    pub fn line_with_break(&self, line: usize) -> Range<usize> {
        let start = self.starts[line];
        let end = self
            .starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        start..end
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.starts[line]
    }

    /// One step of indent: a tab in a file indented with tabs, else the narrowest run of
    /// leading spaces the file uses (two or four), four when it has none.
    pub fn indent_unit(&self) -> &'static str {
        let (mut tabs, mut spaces, mut narrowest) = (0, 0, usize::MAX);
        for i in 0..self.lines().min(2000) {
            let line = self.line(i);
            if line.starts_with('\t') {
                tabs += 1;
            } else if line.starts_with(' ') {
                spaces += 1;
                let n = line.len() - line.trim_start_matches(' ').len();
                if line.trim().is_empty() {
                    continue;
                }
                narrowest = narrowest.min(n);
            }
        }
        if tabs > spaces {
            "\t"
        } else if narrowest == 2 {
            "  "
        } else {
            "    "
        }
    }
}

/// Words, spaces and punctuation, for word moves.
fn class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_leave_out_their_breaks() {
        let b = Buffer::new("one\r\ntwo\r\n".into());
        assert_eq!(b.lines(), 3);
        assert_eq!(b.line(0), "one");
        assert_eq!(b.line(1), "two");
        assert_eq!(b.line(2), "");
        assert_eq!(b.pos(5), (1, 0));
        // a column past the end lands on the end, not between \r and \n
        assert_eq!(b.offset(0, 9), 3);
    }

    #[test]
    fn breaks_keep_the_files_style() {
        let mut b = Buffer::new("a\r\nb".into());
        b.replace(1..1, "\nx", (1, 1));
        assert_eq!(b.text(), "a\r\nx\r\nb");
        let mut u = Buffer::new("a\nb".into());
        u.replace(1..1, "\r\nx", (1, 1));
        assert_eq!(u.text(), "a\nx\nb");
    }

    #[test]
    fn typing_undoes_as_one_step_and_redoes() {
        let mut b = Buffer::new("".into());
        for (i, c) in ["h", "i", "!"].iter().enumerate() {
            b.replace(i..i, c, (i, i));
        }
        assert_eq!(b.text(), "hi!");
        assert_eq!(b.undo(), Some((0, 0)));
        assert_eq!(b.text(), "");
        assert_eq!(b.redo(), Some((3, 3)));
        assert_eq!(b.text(), "hi!");
        // a break ends the run
        b.replace(3..3, "\n", (3, 3));
        b.replace(4..4, "x", (4, 4));
        b.undo();
        assert_eq!(b.text(), "hi!\n");
    }

    #[test]
    fn backspaces_join_and_a_new_edit_drops_redo() {
        let mut b = Buffer::new("abcd".into());
        b.replace(3..4, "", (4, 4));
        b.replace(2..3, "", (3, 3));
        assert_eq!(b.text(), "ab");
        b.undo();
        assert_eq!(b.text(), "abcd");
        b.replace(0..0, "x", (0, 0));
        assert_eq!(b.redo(), None);
    }

    #[test]
    fn moves_cross_line_breaks_and_whole_characters() {
        let b = Buffer::new("ab\r\nçd".into());
        assert_eq!(b.right(2), 4);
        assert_eq!(b.left(4), 2);
        assert_eq!(b.right(4), 6);
        assert_eq!(b.left(6), 4);
        assert_eq!(b.left(0), 0);
        assert_eq!(b.right(b.len()), b.len());
    }

    #[test]
    fn words_stop_at_punctuation() {
        let b = Buffer::new("let foo_bar = x.y;".into());
        assert_eq!(b.word_right(0), 3);
        assert_eq!(b.word_right(3), 11);
        assert_eq!(b.word_right(11), 13);
        assert_eq!(b.word_left(11), 4);
        assert_eq!(b.word_left(18), 17);
        assert_eq!(b.word_at(6), 4..11);
    }

    #[test]
    fn indent_follows_the_file() {
        assert_eq!(Buffer::new("a\n\tb\n\tc".into()).indent_unit(), "\t");
        assert_eq!(Buffer::new("a\n  b\n    c".into()).indent_unit(), "  ");
        assert_eq!(Buffer::new("a\n    b".into()).indent_unit(), "    ");
        assert_eq!(Buffer::new("a".into()).indent_unit(), "    ");
    }
}
