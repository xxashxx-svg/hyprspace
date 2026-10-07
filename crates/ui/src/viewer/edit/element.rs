// Draws the editor: only the lines on screen are shaped, so a long file costs what a screenful
// does. Each frame keeps the cursor in view when a key or an edit asked for it, then paints the
// current line, the selection, the text with its colors and the IME's underline, the cursor,
// and the line numbers in a gutter the text scrolls under.

use std::ops::Range;

use gpui::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, GlobalElementId,
    HighlightStyle, Hsla, LayoutId, PaintQuad, Pixels, Point, ShapedLine, Style, TextAlign,
    TextRun, UnderlineStyle, Window, fill, font, point, prelude::*, px, relative, size,
};
use hyprspace_theme::MONO;

use super::{Editor, Layout, PAD, shape_plain};
use crate::colors;
use crate::viewer::code::{ROW, color, expand, shown_col};

/// The editor's font size.
pub const SIZE: f32 = 12.;

/// A run of the editor's font, colored and styled by `style` where it says.
pub fn run(len: usize, base: Hsla, style: Option<HighlightStyle>) -> TextRun {
    let mut f = font(MONO);
    let mut color = base;
    let (mut background, mut underline) = (None, None);
    if let Some(s) = style {
        if let Some(w) = s.font_weight {
            f.weight = w;
        }
        if let Some(st) = s.font_style {
            f.style = st;
        }
        if let Some(c) = s.color {
            color = c;
        }
        background = s.background_color;
        underline = s.underline;
    }
    TextRun {
        len,
        font: f,
        color,
        background_color: background,
        underline,
        strikethrough: None,
    }
}

/// Runs covering all `len` bytes: the styled ranges, and plain text between them.
fn runs(len: usize, styles: &[(Range<usize>, HighlightStyle)]) -> Vec<TextRun> {
    let base = colors::text1();
    let mut out = Vec::new();
    let mut at = 0;
    for (r, s) in styles {
        if r.start < at || r.end > len || r.start >= r.end {
            continue;
        }
        if r.start > at {
            out.push(run(r.start - at, base, None));
        }
        out.push(run(r.end - r.start, base, Some(*s)));
        at = r.end;
    }
    if at < len || out.is_empty() {
        out.push(run(len - at, base, None));
    }
    out
}

pub struct EditorElement {
    editor: Entity<Editor>,
}

impl EditorElement {
    pub fn new(editor: Entity<Editor>) -> Self {
        Self { editor }
    }
}

pub struct Frame {
    gutter: Bounds<Pixels>,
    numbers: Vec<(Point<Pixels>, ShapedLine)>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    back: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl Editor {
    fn frame(&mut self, bounds: Bounds<Pixels>, window: &mut Window) -> Frame {
        let lh = px(ROW);
        let cell = shape_plain("0", window).width();
        let count = self.buf.lines();
        let digits = count.to_string().len().max(3);
        let gutter_w = cell * digits as f32 + px(28.);
        let text_x = bounds.left() + gutter_w;
        let text_w = bounds.size.width - gutter_w;
        let view_h = bounds.size.height - px(2. * PAD);

        if let Some(center) = self.reveal.take() {
            let (line, _) = self.buf.pos(self.head());
            let y = lh * line as f32;
            if center {
                self.scroll.y = (y - view_h / 2.).max(px(0.));
            } else if y < self.scroll.y {
                self.scroll.y = y;
            } else if y + lh > self.scroll.y + view_h {
                self.scroll.y = y + lh - view_h;
            }
            let x = self.head_x(window);
            let margin = cell * 4.;
            if x < self.scroll.x {
                self.scroll.x = (x - margin).max(px(0.));
            } else if x > self.scroll.x + text_w - margin {
                self.scroll.x = x - text_w + margin * 2.;
            }
        }
        // the last line can come up to the middle, as editors let it
        let total = lh * count as f32;
        let max_y = if total > view_h {
            total - view_h / 2.
        } else {
            px(0.)
        };
        let max_x = (cell * (self.widest + 4) as f32 - text_w).max(px(0.));
        self.scroll.y = self.scroll.y.clamp(px(0.), max_y);
        self.scroll.x = self.scroll.x.clamp(px(0.), max_x);

        let first = (self.scroll.y / lh).floor().max(0.) as usize;
        let last = (((self.scroll.y + view_h) / lh).ceil() as usize + 1).min(count);
        let sel = self.range();
        let (head_line, head_col) = self.buf.pos(self.head());
        let mut frame = Frame {
            gutter: Bounds::new(bounds.origin, size(gutter_w, bounds.size.height)),
            numbers: Vec::new(),
            lines: Vec::new(),
            back: Vec::new(),
            cursor: None,
        };
        let mut shaped_lines = Vec::new();
        for i in first..last {
            let raw = self.buf.line(i);
            let lr = self.buf.line_range(i);
            let top = bounds.top() + px(PAD) + lh * i as f32 - self.scroll.y;
            let origin = point(text_x - self.scroll.x, top);
            // colors from the last pass, unless the line has changed shape since
            let spans: &[_] = match self.spans.get(i) {
                Some(s)
                    if s.iter().all(|(r, _)| {
                        r.end <= raw.len()
                            && raw.is_char_boundary(r.start)
                            && raw.is_char_boundary(r.end)
                    }) =>
                {
                    s
                }
                _ => &[],
            };
            let (shown, spans) = expand(raw, spans);
            let mut styles: Vec<(Range<usize>, HighlightStyle)> =
                spans.iter().map(|(r, k)| (r.clone(), color(*k))).collect();
            if let Some(m) = &self.marked
                && m.start >= lr.start
                && m.end <= lr.end
            {
                let at = shown_col(raw, m.start - lr.start)..shown_col(raw, m.end - lr.start);
                styles.retain(|(r, _)| r.end <= at.start || r.start >= at.end);
                styles.push((
                    at,
                    HighlightStyle {
                        underline: Some(UnderlineStyle {
                            color: Some(colors::text1()),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..Default::default()
                    },
                ));
                styles.sort_by_key(|(r, _)| r.start);
            }
            let shaped = window.text_system().shape_line(
                shown.clone().into(),
                px(SIZE),
                &runs(shown.len(), &styles),
                None,
            );
            let row =
                Bounds::from_corners(point(bounds.left(), top), point(bounds.right(), top + lh));
            if self.lit == Some(i) {
                frame.back.push(fill(row, colors::accent().opacity(0.12)));
            } else if i == head_line && sel.is_empty() {
                frame.back.push(fill(row, colors::ink(0.035)));
            }
            // the selection's part of this line, and a sliver for its break when that's in too
            if !sel.is_empty() && sel.start <= lr.end && sel.end > lr.start {
                let a = sel.start.max(lr.start) - lr.start;
                let b = sel.end.min(lr.end) - lr.start;
                let x0 = shaped.x_for_index(shown_col(raw, a));
                let mut x1 = shaped.x_for_index(shown_col(raw, b));
                if sel.end > lr.end {
                    x1 += cell * 0.6;
                }
                frame.back.push(fill(
                    Bounds::from_corners(point(origin.x + x0, top), point(origin.x + x1, top + lh)),
                    colors::selection(),
                ));
            }
            if i == head_line {
                let x = shaped.x_for_index(shown_col(raw, head_col));
                frame.cursor = Some(fill(
                    Bounds::new(point(origin.x + x, top), size(px(2.), lh)),
                    colors::text1(),
                ));
            }
            let number = shape_number(i + 1, i == head_line, window);
            let nx = bounds.left() + gutter_w - px(14.) - number.width();
            frame.numbers.push((point(nx, top), number));
            shaped_lines.push((i, shaped.clone()));
            frame.lines.push((origin, shaped));
        }
        self.layout = Some(Layout {
            bounds,
            text_x,
            cell,
            lines: shaped_lines,
        });
        frame
    }
}

fn shape_number(n: usize, current: bool, window: &mut Window) -> ShapedLine {
    let text = n.to_string();
    let tone = if current {
        colors::text2()
    } else {
        colors::text3().opacity(0.7)
    };
    window.text_system().shape_line(
        text.clone().into(),
        px(SIZE),
        &[run(text.len(), tone, None)],
        None,
    )
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Frame;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Frame {
        self.editor.update(cx, |e, _| e.frame(bounds, window))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        frame: &mut Frame,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let lh = px(ROW);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in frame.back.drain(..) {
                window.paint_quad(quad);
            }
            for (origin, line) in &frame.lines {
                let _ = line.paint(*origin, lh, TextAlign::Left, None, window, cx);
            }
            if focus.is_focused(window)
                && let Some(cursor) = frame.cursor.take()
            {
                window.paint_quad(cursor);
            }
            // the text scrolls under the numbers
            window.paint_quad(fill(frame.gutter, colors::bg()));
            for (origin, number) in &frame.numbers {
                let _ = number.paint(*origin, lh, TextAlign::Left, None, window, cx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_cover_the_line_and_skip_bad_ranges() {
        let s = HighlightStyle::default();
        let lens = |r: Vec<TextRun>| r.into_iter().map(|r| r.len).collect::<Vec<_>>();
        assert_eq!(lens(runs(10, &[(2..4, s)])), [2, 2, 6]);
        assert_eq!(lens(runs(4, &[(0..4, s)])), [4]);
        assert_eq!(lens(runs(0, &[])), [0]);
        // a range past the end, or overlapping the last, is left out
        assert_eq!(lens(runs(5, &[(0..2, s), (1..3, s), (4..9, s)])), [2, 3]);
    }
}
