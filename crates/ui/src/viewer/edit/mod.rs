// The viewer's editor: a text file to change and save, after VS Code's basics. Typing, IME,
// selection by keys and by mouse (a word on double-click, a line on triple-click), cut, copy and
// paste (a whole line when nothing is selected), undo and redo, Enter keeping the indent, Tab
// and Shift+Tab over a selection's lines, and Ctrl+S. Syntax colors follow the edits a moment
// later. Nothing is written over a change made outside: a save only lands while the file still
// holds what was read (`FolderCommand::WriteFile`), and a file nobody has edited here follows
// the disk on its own. Our own, since Zed's editor is GPL (docs/REWRITE.md); the IME bridge is
// the text box's, from Zed's Apache-2.0 input example.

mod buffer;
mod element;
mod handler;
mod keys;
mod mouse;
mod notice;

use std::ops::Range;
use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle, Focusable,
    IntoElement, MouseButton, Pixels, Point, Render, ShapedLine, Task, Window, div, point,
    prelude::*, px,
};
use hyprspace_proto::folder::SaveError;
use hyprspace_proto::{Client, Command, FolderCommand};
use hyprspace_syntax::Line;

use super::code::{ROW, expand, raw_col, shown_col};
use crate::colors;
use buffer::{Buffer, Sel};
use element::EditorElement;

/// How often a file nobody has edited here is read again, to follow an agent's changes.
const FOLLOW: Duration = Duration::from_secs(2);
/// How long typing rests before the colors are worked out again.
const RECOLOR: Duration = Duration::from_millis(150);
/// Room above the first line and below the last.
pub(crate) const PAD: f32 = 8.;

pub enum EditorEvent {
    /// The unsaved state flipped.
    Dirty,
    /// Saved, or let go, after the user asked to close.
    Close,
}

/// A bar over the text, for what needs the user.
enum Notice {
    /// The file changed on disk under unsaved edits, or a save found it changed.
    Changed,
    Failed(String),
    /// Asked to close with unsaved edits.
    Unsaved,
}

/// What a mouse drag selects by.
#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Char,
    Word,
    Line,
}

/// What the last frame drew, for mapping the pointer and the IME onto the text.
pub(crate) struct Layout {
    pub bounds: Bounds<Pixels>,
    /// Where the text column starts, before scrolling.
    pub text_x: Pixels,
    /// The width of a column of the monospace font.
    pub cell: Pixels,
    /// The lines on screen, as drawn.
    pub lines: Vec<(usize, ShapedLine)>,
}

pub struct Editor {
    path: PathBuf,
    client: Client,
    focus: FocusHandle,
    buf: Buffer,
    sel: Sel,
    /// The x kept across up and down, so a short line doesn't pull the cursor left for good.
    goal: Option<Pixels>,
    marked: Option<Range<usize>>,
    /// What the file held when it was read or last saved.
    saved: String,
    /// The text of a save on its way.
    sending: Option<String>,
    notice: Option<Notice>,
    /// Close once the save lands.
    closing: bool,
    /// Take the next read of the file whatever is unsaved: the user chose Reload.
    force: bool,
    spans: Vec<Line>,
    /// The widest line, in columns, for horizontal scrolling.
    widest: usize,
    /// The line a terminal pointed at, lit until the cursor moves off it.
    lit: Option<usize>,
    scroll: Point<Pixels>,
    /// Bring the cursor into view at the next frame: to the middle, or just inside the edge.
    reveal: Option<bool>,
    drag: Option<(Unit, Range<usize>)>,
    rev: u64,
    pub(crate) layout: Option<Layout>,
    _recolor: Option<Task<()>>,
    _follow: Task<()>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Widest line of `text` in columns, tabs counted as spaces.
fn widest(buf: &Buffer) -> usize {
    (0..buf.lines())
        .map(|i| expand(buf.line(i), &[]).0.chars().count())
        .max()
        .unwrap_or(0)
}

impl Editor {
    pub fn new(
        path: PathBuf,
        text: String,
        client: Client,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let follow = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FOLLOW).await;
                let alive = this.update(cx, |e, _| {
                    if e.sending.is_none() {
                        e.client.send(Command::Folder(FolderCommand::ReadFile {
                            path: e.path.clone(),
                        }));
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let buf = Buffer::new(text.clone());
        let mut e = Self {
            path,
            client,
            focus,
            widest: widest(&buf),
            buf,
            sel: (0, 0),
            goal: None,
            marked: None,
            saved: text,
            sending: None,
            notice: None,
            closing: false,
            force: false,
            spans: Vec::new(),
            lit: None,
            scroll: point(px(0.), px(0.)),
            reveal: None,
            drag: None,
            rev: 0,
            layout: None,
            _recolor: None,
            _follow: follow,
        };
        e.recolor(Duration::ZERO, cx);
        e
    }

    pub fn dirty(&self) -> bool {
        self.buf.text() != self.saved
    }

    /// Puts the cursor on a 1-based line and column, lit and in the middle of the view.
    pub fn target(&mut self, line: Option<u32>, col: Option<u32>, cx: &mut Context<Self>) {
        let Some(line) = line.filter(|l| *l >= 1) else {
            return;
        };
        let line = (line as usize - 1).min(self.buf.lines() - 1);
        let text = self.buf.line(line);
        let col = col
            .map(|c| {
                text.char_indices()
                    .nth(c.saturating_sub(1) as usize)
                    .map_or(text.len(), |(i, _)| i)
            })
            .unwrap_or(0);
        let at = self.buf.offset(line, col);
        self.sel = (at, at);
        self.lit = Some(line);
        self.reveal = Some(true);
        cx.notify();
    }

    /// The file as read again: taken as it is when nothing here is unsaved, else the user
    /// hears that it changed.
    pub fn disk(&mut self, text: &str, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.force) {
            self.reload(text.to_string(), cx);
            return;
        }
        if self.sending.is_some() || text == self.saved {
            return;
        }
        if self.dirty() {
            if self.notice.is_none() {
                self.notice = Some(Notice::Changed);
                cx.notify();
            }
            return;
        }
        self.reload(text.to_string(), cx);
    }

    /// Takes `text` as the file, keeping the cursor's line and column.
    fn reload(&mut self, text: String, cx: &mut Context<Self>) {
        let (line, col) = self.buf.pos(self.head());
        self.buf = Buffer::new(text.clone());
        self.saved = text;
        self.widest = widest(&self.buf);
        let at = self.buf.offset(line, col);
        self.sel = (at, at);
        self.marked = None;
        self.notice = None;
        self.rev += 1;
        self.recolor(Duration::ZERO, cx);
        cx.emit(EditorEvent::Dirty);
        cx.notify();
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        self.write(Some(self.saved.clone()), cx);
    }

    fn write(&mut self, expect: Option<String>, cx: &mut Context<Self>) {
        if self.sending.is_some() {
            return;
        }
        if !self.dirty() && expect.is_some() {
            if self.closing {
                cx.emit(EditorEvent::Close);
            }
            return;
        }
        let text = self.buf.text().to_string();
        self.client.send(Command::Folder(FolderCommand::WriteFile {
            path: self.path.clone(),
            text: text.clone(),
            expect,
        }));
        self.sending = Some(text);
        self.buf.seal();
        cx.notify();
    }

    pub fn saved(&mut self, result: &Result<(), SaveError>, cx: &mut Context<Self>) {
        let Some(text) = self.sending.take() else {
            return;
        };
        match result {
            Ok(()) => {
                self.saved = text;
                self.notice = None;
                cx.emit(EditorEvent::Dirty);
                if self.closing && !self.dirty() {
                    cx.emit(EditorEvent::Close);
                }
            }
            Err(SaveError::Changed) => self.notice = Some(Notice::Changed),
            Err(SaveError::Failed(why)) => self.notice = Some(Notice::Failed(why.clone())),
        }
        if result.is_err() {
            self.closing = false;
        }
        cx.notify();
    }

    /// The card's close: asks first when there are unsaved edits. Returns whether it may close
    /// now.
    pub fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.dirty() {
            return true;
        }
        self.notice = Some(Notice::Unsaved);
        cx.notify();
        false
    }

    // ---- selection ----

    fn head(&self) -> usize {
        self.sel.1
    }

    fn range(&self) -> Range<usize> {
        let (a, b) = self.sel;
        a.min(b)..a.max(b)
    }

    fn move_to(&mut self, at: usize, select: bool) {
        self.sel = if select { (self.sel.0, at) } else { (at, at) };
        self.marked = None;
        self.reveal = Some(false);
        if self.lit.is_some_and(|l| l != self.buf.line_of(at)) {
            self.lit = None;
        }
    }

    /// The x the cursor is drawn at on its line, from the last frame.
    fn head_x(&self, window: &mut Window) -> Pixels {
        let (line, col) = self.buf.pos(self.head());
        let text = self.buf.line(line);
        let shown = expand(text, &[]).0;
        let at = shown_col(text, col);
        let shaped = shape_plain(&shown, window);
        shaped.x_for_index(at)
    }

    /// The offset on `line` nearest `x` in the text column.
    fn at_x(&self, line: usize, x: Pixels, window: &mut Window) -> usize {
        let text = self.buf.line(line);
        let shown = expand(text, &[]).0;
        let shaped = shape_plain(&shown, window);
        let ix = shaped.closest_index_for_x(x.max(px(0.)));
        self.buf.offset(line, raw_col(text, ix))
    }

    fn vertical(&mut self, rows: isize, select: bool, window: &mut Window) {
        let x = match self.goal {
            Some(x) => x,
            None => self.head_x(window),
        };
        let (line, _) = self.buf.pos(self.head());
        let to = line as isize + rows;
        let at = if to < 0 {
            0
        } else if to as usize >= self.buf.lines() {
            self.buf.len()
        } else {
            self.at_x(to as usize, x, window)
        };
        self.move_to(at, select);
        self.goal = Some(x);
    }

    /// The offset under a window position, from the last frame's layout.
    fn offset_at(&self, p: Point<Pixels>, window: &mut Window) -> usize {
        let Some(l) = &self.layout else {
            return self.head();
        };
        let y = p.y - l.bounds.top() - px(PAD) + self.scroll.y;
        let line = (y / px(ROW)).floor().max(0.) as usize;
        if line >= self.buf.lines() {
            return self.buf.len();
        }
        self.at_x(line, p.x - l.text_x + self.scroll.x, window)
    }

    // ---- edits ----

    /// Writes `text` over the selection, or over `range`.
    fn type_text(&mut self, range: Option<Range<usize>>, text: &str, cx: &mut Context<Self>) {
        let range = range.unwrap_or_else(|| self.range());
        let was = self.dirty();
        let (l0, _) = self.buf.pos(range.start);
        let (l1, _) = self.buf.pos(range.end);
        let end = self.buf.replace(range, text, self.sel);
        let (l2, _) = self.buf.pos(end);
        // the colors of the lines touched wait for the next pass; the rest keep theirs
        if l0 < self.spans.len() {
            let to = (l1 + 1).min(self.spans.len());
            self.spans
                .splice(l0..to, std::iter::repeat_n(Vec::new(), l2 - l0 + 1));
        }
        for l in l0..=l2 {
            self.widest = self
                .widest
                .max(expand(self.buf.line(l), &[]).0.chars().count());
        }
        self.sel = (end, end);
        self.marked = None;
        self.goal = None;
        self.lit = None;
        self.reveal = Some(false);
        self.rev += 1;
        self.recolor(RECOLOR, cx);
        if was != self.dirty() {
            cx.emit(EditorEvent::Dirty);
        }
        cx.notify();
    }

    fn recolor(&mut self, after: Duration, cx: &mut Context<Self>) {
        let (path, text, rev) = (self.path.clone(), self.buf.text().to_string(), self.rev);
        self._recolor = Some(cx.spawn(async move |this, cx| {
            if !after.is_zero() {
                cx.background_executor().timer(after).await;
            }
            let job = cx
                .background_executor()
                .spawn(async move { hyprspace_syntax::highlight(&path, &text) });
            let spans = job.await;
            let _ = this.update(cx, |e, cx| {
                if e.rev == rev {
                    e.spans = spans.unwrap_or_default();
                    e.widest = widest(&e.buf);
                    cx.notify();
                }
            });
        }));
    }

    fn newline(&mut self, cx: &mut Context<Self>) {
        // the new line starts where the indent of this one does
        let (line, col) = self.buf.pos(self.range().start);
        let text = self.buf.line(line);
        let indent: String = text[..col]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.buf.seal();
        self.type_text(None, &format!("\n{indent}"), cx);
        self.buf.seal();
    }

    /// Tab: a step of indent at the cursor, or before each line of a selection that spans
    /// lines. Shift+Tab takes a step off each line.
    fn indent(&mut self, out: bool, cx: &mut Context<Self>) {
        let unit = self.buf.indent_unit();
        let r = self.range();
        let (first, _) = self.buf.pos(r.start);
        let (last, _) = self.buf.pos(r.end);
        if !out && first == last {
            self.type_text(None, unit, cx);
            return;
        }
        let start = self.buf.line_start(first);
        let end = self.buf.line_range(last).end;
        let mut lines = Vec::new();
        for i in first..=last {
            let text = self.buf.line(i);
            lines.push(if out {
                let step = if unit == "\t" { 4 } else { unit.len() };
                let cut = if text.starts_with('\t') {
                    1
                } else {
                    (text.len() - text.trim_start_matches(' ').len()).min(step)
                };
                text[cut..].to_string()
            } else if text.is_empty() {
                String::new()
            } else {
                format!("{unit}{}", text)
            });
        }
        let before = self.sel;
        self.buf.seal();
        // breaks come out in the file's style
        self.type_text(Some(start..end), &lines.join("\n"), cx);
        // the lines stay selected
        let new_end = self.head();
        self.sel = if before.0 <= before.1 {
            (start, new_end)
        } else {
            (new_end, start)
        };
        self.buf.set_after(self.sel);
        self.buf.seal();
    }

    fn copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let mut r = self.range();
        // nothing selected: the whole line, as VS Code does
        if r.is_empty() {
            let (line, _) = self.buf.pos(self.head());
            r = self.buf.line_with_break(line);
        }
        let mut text = self.buf.text()[r.clone()].to_string();
        if self.range().is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        if cut {
            self.buf.seal();
            self.type_text(Some(r), "", cx);
            self.buf.seal();
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            self.buf.seal();
            self.type_text(None, &text, cx);
            self.buf.seal();
        }
    }

    fn undo(&mut self, redo: bool, cx: &mut Context<Self>) {
        let was = self.dirty();
        let sel = if redo {
            self.buf.redo()
        } else {
            self.buf.undo()
        };
        if let Some(sel) = sel {
            self.sel = sel;
            self.marked = None;
            self.reveal = Some(false);
            self.rev += 1;
            self.spans.clear();
            self.recolor(Duration::ZERO, cx);
            if was != self.dirty() {
                cx.emit(EditorEvent::Dirty);
            }
            cx.notify();
        }
    }
}

/// `text` shaped in the editor's font, uncolored: for measuring.
fn shape_plain(text: &str, window: &mut Window) -> ShapedLine {
    let style = element::run(text.len(), colors::text1(), None);
    window
        .text_system()
        .shape_line(text.to_string().into(), px(element::SIZE), &[style], None)
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("editor")
            .size_full()
            .flex()
            .flex_col()
            .children(self.notice_bar(cx))
            .child(
                div()
                    .id("editor-text")
                    .flex_1()
                    .min_h_0()
                    .track_focus(&self.focus)
                    .cursor(CursorStyle::IBeam)
                    .on_key_down(cx.listener(Self::on_key))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                    .on_mouse_move(cx.listener(Self::on_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up))
                    .on_scroll_wheel(cx.listener(Self::on_wheel))
                    .child(EditorElement::new(cx.entity())),
            )
    }
}
