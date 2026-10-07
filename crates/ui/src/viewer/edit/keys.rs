// The editor's keys: moves (by character, word, line, page and file, Shift to select), the
// deletes, Enter and Tab, and the Ctrl shortcuts (Cmd on macOS). Typed text comes through the
// input handler instead, IME and all; a key handled here stops there.

use gpui::{Context, KeyDownEvent, Window, px};

use super::Editor;
use crate::viewer::code::ROW;

impl Editor {
    pub(super) fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // the close dialog holds the keyboard: Enter saves, Esc goes back to the text
        if self.asking() {
            match e.keystroke.key.as_str() {
                "enter" => self.save_and_close(cx),
                "escape" => self.keep_editing(cx),
                _ => {}
            }
            cx.stop_propagation();
            return;
        }
        let m = &e.keystroke.modifiers;
        let primary = if cfg!(target_os = "macos") {
            m.platform
        } else {
            m.control
        };
        let shift = m.shift;
        let head = self.head();
        let keep_goal = matches!(
            e.keystroke.key.as_str(),
            "up" | "down" | "pageup" | "pagedown"
        );
        let handled = match (e.keystroke.key.as_str(), primary) {
            ("left", false) if !shift && !self.range().is_empty() => {
                self.move_to(self.range().start, false);
                true
            }
            ("right", false) if !shift && !self.range().is_empty() => {
                self.move_to(self.range().end, false);
                true
            }
            ("left", word) => {
                let to = if word {
                    self.buf.word_left(head)
                } else {
                    self.buf.left(head)
                };
                self.move_to(to, shift);
                true
            }
            ("right", word) => {
                let to = if word {
                    self.buf.word_right(head)
                } else {
                    self.buf.right(head)
                };
                self.move_to(to, shift);
                true
            }
            ("up", false) => {
                self.vertical(-1, shift, window);
                true
            }
            ("down", false) => {
                self.vertical(1, shift, window);
                true
            }
            ("pageup" | "pagedown", false) => {
                let rows = self
                    .layout
                    .as_ref()
                    .map_or(30, |l| (l.bounds.size.height / px(ROW)) as isize - 2)
                    .max(1);
                let rows = if e.keystroke.key == "pageup" {
                    -rows
                } else {
                    rows
                };
                self.scroll.y = (self.scroll.y + px(ROW) * rows as f32).max(px(0.));
                self.vertical(rows, shift, window);
                true
            }
            ("home", false) => {
                // the first character, then the line's very start, as editors toggle
                let (line, col) = self.buf.pos(head);
                let text = self.buf.line(line);
                let first = text.len() - text.trim_start().len();
                let to = if col == first { 0 } else { first };
                self.move_to(self.buf.line_start(line) + to, shift);
                true
            }
            ("end", false) => {
                let (line, _) = self.buf.pos(head);
                self.move_to(self.buf.line_range(line).end, shift);
                true
            }
            ("home", true) => {
                self.move_to(0, shift);
                true
            }
            ("end", true) => {
                self.move_to(self.buf.len(), shift);
                true
            }
            ("backspace", word) => {
                if self.range().is_empty() {
                    let to = if word {
                        self.buf.word_left(head)
                    } else {
                        self.buf.left(head)
                    };
                    self.type_text(Some(to..head), "", cx);
                } else {
                    self.type_text(None, "", cx);
                }
                true
            }
            ("delete", word) => {
                if self.range().is_empty() {
                    let to = if word {
                        self.buf.word_right(head)
                    } else {
                        self.buf.right(head)
                    };
                    self.type_text(Some(head..to), "", cx);
                } else {
                    self.type_text(None, "", cx);
                }
                true
            }
            ("enter", false) if !shift => {
                self.newline(cx);
                true
            }
            ("tab", false) => {
                self.indent(shift, cx);
                true
            }
            ("escape", false) if !self.range().is_empty() => {
                self.move_to(head, false);
                true
            }
            ("a", true) => {
                self.sel = (0, self.buf.len());
                true
            }
            ("c", true) => {
                self.copy(false, cx);
                true
            }
            ("x", true) => {
                self.copy(true, cx);
                true
            }
            ("v", true) => {
                self.paste(cx);
                true
            }
            ("z", true) => {
                self.undo(shift, cx);
                true
            }
            ("y", true) => {
                self.undo(true, cx);
                true
            }
            ("s", true) => {
                self.save(cx);
                true
            }
            _ => false,
        };
        if handled {
            if !keep_goal {
                self.goal = None;
            }
            cx.stop_propagation();
            cx.notify();
        }
    }
}
