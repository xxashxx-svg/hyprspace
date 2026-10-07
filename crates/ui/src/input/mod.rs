// A text box: one line (search, rename) or many with wrapping (the composer). It takes typing
// and IME input through GPUI's input handler, and tells its owner about changes, Enter, Escape
// and pasted images through events.
//
// Adapted from Zed's `crates/gpui/examples/input.rs` (Apache-2.0, see THIRD_PARTY_NOTICES.md):
// the selection model, the UTF-16 bridging and the input handler are theirs; wrapping, several
// lines, vertical moves and the events are ours.

mod element;
mod handler;

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardEntry, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle,
    Focusable, Image, IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, SharedString, Window, WrappedLine, actions, div, point,
    prelude::*,
};
use unicode_segmentation::UnicodeSegmentation;

use element::TextElement;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Paste,
        Cut,
        Copy,
        Submit,
        Newline,
        Cancel,
    ]
);

const CONTEXT: &str = "TextInput";

/// Binds the editing keys. Call once at startup.
pub fn bind_keys(cx: &mut App) {
    let word = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new(&format!("{word}-backspace"), DeleteWordLeft, c),
        KeyBinding::new(&format!("{word}-delete"), DeleteWordRight, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new(&format!("{word}-left"), WordLeft, c),
        KeyBinding::new(&format!("{word}-right"), WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new(&format!("{word}-shift-left"), SelectWordLeft, c),
        KeyBinding::new(&format!("{word}-shift-right"), SelectWordRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("enter", Submit, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("escape", Cancel, c),
    ]);
}

pub enum InputEvent {
    Changed,
    /// Enter. The owner decides what it means and clears the box if it wants.
    Submit,
    /// Escape.
    Cancel,
    /// Images on the clipboard at a paste. Any text in the same paste is inserted as usual.
    Images(Vec<Image>),
}

pub struct TextInput {
    focus: FocusHandle,
    content: String,
    placeholder: SharedString,
    multiline: bool,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    selecting: bool,
    /// What the last paint laid out: each hard line's start offset and its wrapped layout.
    layout: Vec<(usize, WrappedLine)>,
    bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
}

impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    pub fn new(
        placeholder: impl Into<SharedString>,
        multiline: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus: cx.focus_handle(),
            content: String::new(),
            placeholder: placeholder.into(),
            multiline,
            selected: 0..0,
            reversed: false,
            marked: None,
            selecting: false,
            layout: Vec::new(),
            bounds: None,
            line_height: Pixels::ZERO,
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = text.into();
        let end = self.content.len();
        self.selected = end..end;
        self.reversed = false;
        self.marked = None;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    /// Types `text` over the selection, the way a keystroke would. An editor whose Enter means a
    /// new line answers `InputEvent::Submit` with this.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.replace(self.selected.clone(), text, cx);
    }

    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        if text != self.placeholder {
            self.placeholder = text;
            cx.notify();
        }
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.selected = 0..self.content.len();
        self.reversed = false;
        cx.notify();
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected = offset..offset;
        self.reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.reversed {
            self.selected.start = offset
        } else {
            self.selected.end = offset
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.content.len())
    }

    fn word_left(&self, offset: usize) -> usize {
        word_start(&self.content, offset)
    }

    fn word_right(&self, offset: usize) -> usize {
        word_end(&self.content, offset)
    }

    /// The offset one visual row above (`rows` = -1) or below (+1) the cursor, keeping its x.
    fn vertical(&self, rows: i32) -> Option<usize> {
        let at = self.position_of(self.cursor())?;
        let target = point(
            at.x,
            at.y + self.line_height * rows as f32 + self.line_height / 2.,
        );
        Some(self.offset_at(target))
    }

    /// Where `offset` sits, relative to the box's top left.
    fn position_of(&self, offset: usize) -> Option<Point<Pixels>> {
        let mut y = Pixels::ZERO;
        for (start, line) in &self.layout {
            let end = start + line.len();
            if offset <= end && offset >= *start {
                let p = line.position_for_index(offset - start, self.line_height)?;
                return Some(point(p.x, p.y + y));
            }
            y += self.line_height * (line.wrap_boundaries().len() + 1) as f32;
        }
        None
    }

    /// The offset nearest `pos`, relative to the box's top left.
    fn offset_at(&self, pos: Point<Pixels>) -> usize {
        if pos.y < Pixels::ZERO {
            return 0;
        }
        let mut y = Pixels::ZERO;
        for (start, line) in &self.layout {
            let height = self.line_height * (line.wrap_boundaries().len() + 1) as f32;
            if pos.y < y + height {
                let local = point(pos.x.max(Pixels::ZERO), pos.y - y);
                let ix = line
                    .closest_index_for_position(local, self.line_height)
                    .unwrap_or_else(|i| i);
                return start + ix.min(line.len());
            }
            y += height;
        }
        self.content.len()
    }

    fn mouse_offset(&self, pos: Point<Pixels>) -> usize {
        match self.bounds {
            Some(b) => self.offset_at(pos - b.origin),
            None => self.content.len(),
        }
    }

    fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let text = if self.multiline {
            text.replace("\r\n", "\n")
        } else {
            text.replace(['\r', '\n'], " ")
        };
        self.content.replace_range(range.clone(), &text);
        let at = range.start + text.len();
        self.selected = at..at;
        self.reversed = false;
        self.marked = None;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.move_to(self.previous_boundary(self.cursor()), cx);
        } else {
            self.move_to(self.selected.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.move_to(self.next_boundary(self.selected.end), cx);
        } else {
            self.move_to(self.selected.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(-1).unwrap_or(0);
        self.move_to(to, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(1).unwrap_or(self.content.len());
        self.move_to(to, cx);
    }

    fn word_left_action(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.word_left(self.cursor()), cx);
    }

    fn word_right_action(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.word_right(self.cursor()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.word_left(self.cursor()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.word_right(self.cursor()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical(-1).unwrap_or(0), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(1).unwrap_or(self.content.len());
        self.select_to(to, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }

    // Home and End go to the hard line's ends: good enough for a prompt box.
    fn line_start(&self) -> usize {
        self.content[..self.cursor()]
            .rfind('\n')
            .map_or(0, |i| i + 1)
    }

    fn line_end(&self) -> usize {
        let at = self.cursor();
        self.content[at..]
            .find('\n')
            .map_or(self.content.len(), |i| at + i)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_start(), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_end(), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_start(), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_end(), cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selected.is_empty() {
            self.previous_boundary(self.cursor())..self.cursor()
        } else {
            self.selected.clone()
        };
        self.replace(range, "", cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selected.is_empty() {
            self.cursor()..self.next_boundary(self.cursor())
        } else {
            self.selected.clone()
        };
        self.replace(range, "", cx);
    }

    /// Ctrl+Backspace (Option on macOS): the word before the cursor, or the selection.
    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selected.is_empty() {
            self.word_left(self.cursor())..self.cursor()
        } else {
            self.selected.clone()
        };
        self.replace(range, "", cx);
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selected.is_empty() {
            self.cursor()..self.word_right(self.cursor())
        } else {
            self.selected.clone()
        };
        self.replace(range, "", cx);
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let images: Vec<Image> = item
            .entries()
            .iter()
            .filter_map(|e| match e {
                ClipboardEntry::Image(img) => Some(img.clone()),
                _ => None,
            })
            .collect();
        if !images.is_empty() {
            cx.emit(InputEvent::Images(images));
        }
        let text = item.entries().iter().find_map(|e| match e {
            ClipboardEntry::String(s) => Some(s.text().to_string()),
            _ => None,
        });
        if let Some(text) = text {
            self.replace(self.selected.clone(), &text, cx);
        }
    }

    /// The box's own selection, or else text selected in a transcript above it.
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = if self.selected.is_empty() {
            crate::markdown::select::selected_text()
        } else {
            Some(self.content[self.selected.clone()].to_string())
        };
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            let text = self.content[self.selected.clone()].to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace(self.selected.clone(), "", cx);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submit);
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace(self.selected.clone(), "\n", cx);
        } else {
            cx.emit(InputEvent::Submit);
        }
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Cancel);
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.selecting = true;
        let at = self.mouse_offset(e.position);
        if e.modifiers.shift {
            self.select_to(at, cx);
        } else {
            self.move_to(at, cx);
        }
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.select_to(self.mouse_offset(e.position), cx);
        }
    }
}

/// The start of the word before `offset`, skipping spaces first.
fn word_start(text: &str, offset: usize) -> usize {
    let before = &text[..offset];
    let trimmed = before.trim_end();
    trimmed.rfind(|c: char| c.is_whitespace()).map_or(0, |i| {
        i + trimmed[i..].chars().next().map_or(1, char::len_utf8)
    })
}

/// The end of the word after `offset`, skipping spaces first.
fn word_end(text: &str, offset: usize) -> usize {
    let after = &text[offset..];
    let skipped = after.len() - after.trim_start().len();
    let rest = &after[skipped..];
    offset + skipped + rest.find(char::is_whitespace).unwrap_or(rest.len())
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_left_action))
            .on_action(cx.listener(Self::word_right_action))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .child(TextElement::new(cx.entity()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_skip_spaces_then_stop_at_the_next_gap() {
        let t = "fix the  bug";
        assert_eq!(word_start(t, t.len()), 9);
        assert_eq!(word_start(t, 9), 4);
        assert_eq!(word_start(t, 2), 0);
        assert_eq!(word_end(t, 0), 3);
        assert_eq!(word_end(t, 3), 7);
        assert_eq!(word_end(t, 7), t.len());
        assert_eq!(word_start("héllo wörld", 13), 7);
    }
}
