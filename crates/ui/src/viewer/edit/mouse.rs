// The editor's mouse: a click places the cursor (Shift+click selects to it), a double-click
// takes a word and a triple-click a line, a drag selects by whichever it started with and
// scrolls when held past an edge, the wheel scrolls, sideways with Shift, and the scrollbars drag.

use std::ops::Range;

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollWheelEvent, Window,
};

use super::{Editor, Unit};
use crate::viewer::code::ROW;
use crate::zoom::px;

impl Editor {
    pub(super) fn on_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if self.take_bar(e.position) {
            cx.notify();
            return;
        }
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
        if let Some((down, from)) = self.grab {
            if e.pressed_button == Some(MouseButton::Left) {
                self.slide(down, from, e.position);
            } else {
                self.grab = None;
            }
            cx.notify();
            return;
        }
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

    pub(super) fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.drag = None;
        if self.grab.take().is_some() {
            cx.notify();
        }
    }

    /// A press on a scrollbar takes its thumb: where it was pressed on the thumb, or by the
    /// middle after a jump to a press on the track.
    fn take_bar(&mut self, p: Point<Pixels>) -> bool {
        let Some(l) = &self.layout else {
            return false;
        };
        for (b, down) in [(l.down, true), (l.across, false)] {
            let Some(b) = b.filter(|b| b.track.contains(&p)) else {
                continue;
            };
            let (at, start, len) = if down {
                (p.y, b.thumb.top(), b.thumb.size.height)
            } else {
                (p.x, b.thumb.left(), b.thumb.size.width)
            };
            let from = if at >= start && at <= start + len {
                at - start
            } else {
                len / 2.
            };
            self.grab = Some((down, from));
            self.slide(down, from, p);
            return true;
        }
        false
    }

    /// Scrolls so the held thumb sits under the pointer.
    fn slide(&mut self, down: bool, from: Pixels, p: Point<Pixels>) {
        let Some(l) = &self.layout else {
            return;
        };
        let Some(b) = (if down { l.down } else { l.across }) else {
            return;
        };
        let (at, start, long, len) = if down {
            (p.y, b.track.top(), b.track.size.height, b.thumb.size.height)
        } else {
            (p.x, b.track.left(), b.track.size.width, b.thumb.size.width)
        };
        let free = long - len;
        if free <= px(0.) {
            return;
        }
        let to = b.max * ((at - from - start) / free).clamp(0., 1.);
        if down {
            self.scroll.y = to;
        } else {
            self.scroll.x = to;
        }
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
