// The editor's side of the platform's text input: typing, IME composition and where the
// candidate window goes. The text box's bridge (Zed's Apache-2.0 input example, see
// THIRD_PARTY_NOTICES.md) over the whole file: the platform counts in UTF-16, the editor in
// UTF-8 bytes.

use std::ops::Range;

use gpui::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point, size,
};

use super::{Editor, PAD};
use crate::viewer::code::{ROW, shown_col};

impl Editor {
    fn offset_from_utf16(&self, offset: usize) -> usize {
        let (mut utf8, mut utf16) = (0, 0);
        for ch in self.buf.text().chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let (mut utf8, mut utf16) = (0, 0);
        for ch in self.buf.text().chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(r.start)..self.offset_to_utf16(r.end)
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end)
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range);
        actual.replace(self.range_to_utf16(&range));
        Some(self.buf.text()[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.range()),
            reversed: self.sel.1 < self.sel.0,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone());
        self.type_text(range, text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        new_selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.range());
        let start = range.start;
        self.type_text(Some(range), text, cx);
        self.marked = (!text.is_empty()).then(|| start..start + text.len());
        if let Some(r) = new_selected {
            let r = self.range_from_utf16(&r);
            self.sel = (start + r.start, start + r.end);
        }
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let at = self.range_from_utf16(&range).start;
        let l = self.layout.as_ref()?;
        let (line, col) = self.buf.pos(at);
        let (_, shaped) = l.lines.iter().find(|(i, _)| *i == line)?;
        let x = shaped.x_for_index(shown_col(self.buf.line(line), col));
        let top =
            l.bounds.top() + Pixels::from(PAD) + Pixels::from(ROW) * line as f32 - self.scroll.y;
        Some(Bounds::new(
            point(l.text_x - self.scroll.x + x, top),
            size(l.cell, Pixels::from(ROW)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.offset_at(p, window)))
    }
}
