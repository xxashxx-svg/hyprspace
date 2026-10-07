// The editor's mouse: a click places the cursor (Shift+click selects to it), a double-click
// takes a word and a triple-click a line, a drag selects by whichever it started with and
// scrolls when held past an edge, and the wheel scrolls, sideways with Shift.

use std::ops::Range;

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window,
    px,
};

use super::{Editor, Unit};
use crate::viewer::code::ROW;

impl Editor {
    pub(super) fn on_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let at = self.offset_at(e.position, window);
        let unit = match e.click_count {
            1 => Unit::Char,
            2 => Unit::Word,
            _ => Unit::Line,
        };
        let pick = self.unit_range(unit, at);
        if e.modifiers.shift && unit == Unit::Char {
            self.move_to(at, true);
        } else {
            self.sel = (pick.start, pick.end);
            self.marked = None;
            self.lit = None;
        }
        self.goal = None;
        self.drag = Some((unit, pick));
        self.buf.seal();
        cx.notify();
    }

    fn unit_range(&self, unit: Unit, at: usize) -> Range<usize> {
        match unit {
            Unit::Char => at..at,
            Unit::Word => self.buf.word_at(at),
            Unit::Line => self.buf.line_with_break(self.buf.line_of(at)),
        }
    }

    pub(super) fn on_move(
        &mut self,
        e: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((unit, first)) = self.drag.clone() else {
            return;
        };
        if e.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        // held past the top or bottom, the text scrolls under the pointer
        if let Some(l) = &self.layout {
            if e.position.y < l.bounds.top() {
                self.scroll.y = (self.scroll.y - px(ROW)).max(px(0.));
            } else if e.position.y > l.bounds.bottom() {
                self.scroll.y += px(ROW);
            }
        }
        let at = self.offset_at(e.position, window);
        let now = self.unit_range(unit, at);
        self.sel = if now.start < first.start {
            (first.end, now.start)
        } else {
            (first.start, now.end.max(first.end))
        };
        cx.notify();
    }

    pub(super) fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.drag = None;
    }

    pub(super) fn on_wheel(
        &mut self,
        e: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let d = e.delta.pixel_delta(px(ROW));
        let (dx, dy) = if e.modifiers.shift && d.x == px(0.) {
            (d.y, px(0.))
        } else {
            (d.x, d.y)
        };
        self.scroll.x = (self.scroll.x - dx).max(px(0.));
        self.scroll.y = (self.scroll.y - dy).max(px(0.));
        cx.stop_propagation();
        cx.notify();
    }
}
