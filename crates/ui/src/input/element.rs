// Lays out and paints a `TextInput`: the text wrapped to the box's width, the selection, the
// IME's marked text underlined, and the cursor. The layout is kept on the input so clicks and
// arrow keys can map between offsets and positions.

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, ElementId, ElementInputHandler, Entity,
    GlobalElementId, LayoutId, PaintQuad, Pixels, ScrollHandle, SharedString, Style, TextRun,
    TextStyle, UnderlineStyle, Window, WrappedLine, fill, point, prelude::*, relative, size,
};

use super::TextInput;
use crate::colors;
use crate::zoom::px;

pub struct TextElement {
    input: Entity<TextInput>,
}

impl TextElement {
    pub fn new(input: Entity<TextInput>) -> Self {
        Self { input }
    }
}

pub struct Prepaint {
    lines: Vec<(usize, WrappedLine)>,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

/// Shapes `input`'s text (or its placeholder) wrapped at `width`, with each hard line's start.
fn shape(
    input: &TextInput,
    style: &TextStyle,
    width: Option<Pixels>,
    window: &mut Window,
) -> Vec<(usize, WrappedLine)> {
    let (text, color): (SharedString, _) = if input.content.is_empty() {
        (input.placeholder.clone(), colors::text2())
    } else {
        (input.content.clone().into(), style.color)
    };
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let runs = match input.marked.as_ref().filter(|_| !input.content.is_empty()) {
        Some(m) => vec![
            TextRun {
                len: m.start,
                ..run.clone()
            },
            TextRun {
                len: m.end - m.start,
                underline: Some(UnderlineStyle {
                    color: Some(color),
                    thickness: px(1.),
                    wavy: false,
                }),
                ..run.clone()
            },
            TextRun {
                len: text.len() - m.end,
                ..run
            },
        ],
        None if input.content.is_empty() => vec![run],
        None => {
            let mut runs = Vec::new();
            let mut last = 0;
            for range in input.atoms() {
                runs.push(TextRun {
                    len: range.start - last,
                    ..run.clone()
                });
                runs.push(TextRun {
                    len: range.len(),
                    color: colors::text1(),
                    background_color: Some(colors::ink(0.1)),
                    ..run.clone()
                });
                last = range.end;
            }
            runs.push(TextRun {
                len: text.len() - last,
                ..run
            });
            runs.retain(|r| r.len > 0);
            runs
        }
    };
    let font_size = style.font_size.to_pixels(window.rem_size());
    let wrap = if input.multiline { width } else { None };
    let lines = window
        .text_system()
        .shape_text(text.clone(), font_size, &runs, wrap, None)
        .unwrap_or_default();
    let mut start = 0;
    lines
        .into_iter()
        .map(|line| {
            let at = start;
            start += line.len() + 1;
            (at, line)
        })
        .collect()
}

fn thumb(scroll: &ScrollHandle) -> Option<Bounds<Pixels>> {
    let view = scroll.bounds();
    let max = scroll.max_offset().y;
    if max <= Pixels::ZERO {
        return None;
    }
    let track = view.size.height - px(4.);
    let h = (track * (view.size.height / (view.size.height + max))).max(px(24.));
    let y = (track - h) * (-scroll.offset().y / max);
    Some(Bounds::new(
        point(view.right() - px(6.), view.top() + px(2.) + y),
        size(px(4.), h),
    ))
}

fn rows(lines: &[(usize, WrappedLine)]) -> usize {
    lines
        .iter()
        .map(|(_, l)| l.wrap_boundaries().len() + 1)
        .sum::<usize>()
        .max(1)
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

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
        _: &mut App,
    ) -> (LayoutId, ()) {
        let input = self.input.clone();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        // layout runs after the parents' text style is popped, so it travels with the closure
        let text = window.text_style();
        let lh = window.line_height();
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let lines = shape(input.read(cx), &text, width, window);
            let height = lh * rows(&lines) as f32;
            size(width.unwrap_or(Pixels::ZERO), height)
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let input = self.input.read(cx);
        let lines = shape(input, &window.text_style(), Some(bounds.size.width), window);
        let lh = window.line_height();
        let empty = input.content.is_empty();
        let (sel, cursor) = (input.selected.clone(), input.cursor());
        let shift = match lines.first() {
            Some((_, line)) if !input.multiline && !empty => {
                let width = bounds.size.width;
                let at = line
                    .position_for_index(cursor, lh)
                    .map_or(Pixels::ZERO, |p| p.x);
                let mut shift = input.shift;
                if at < shift {
                    shift = at;
                } else if at + px(2.) > shift + width {
                    shift = at + px(2.) - width;
                }
                shift.min((line.width() + px(2.) - width).max(Pixels::ZERO))
            }
            _ => Pixels::ZERO,
        };
        if shift != input.shift {
            self.input.update(cx, |input, _| input.shift = shift);
        }
        let input = self.input.read(cx);

        // where an offset lands, in window coordinates, using the fresh layout
        let pos = |offset: usize| {
            let mut y = bounds.top();
            for (start, line) in &lines {
                if offset >= *start && offset <= start + line.len() {
                    let p = line.position_for_index(offset - start, lh)?;
                    return Some(point(bounds.left() + p.x - shift, y + p.y));
                }
                y += lh * (line.wrap_boundaries().len() + 1) as f32;
            }
            None
        };

        let mut selection = Vec::new();
        if !sel.is_empty()
            && !empty
            && let (Some(a), Some(b)) = (pos(sel.start), pos(sel.end))
        {
            let wash = colors::selection();
            if a.y == b.y {
                selection.push(fill(Bounds::from_corners(a, point(b.x, b.y + lh)), wash));
            } else {
                selection.push(fill(
                    Bounds::from_corners(a, point(bounds.right(), a.y + lh)),
                    wash,
                ));
                if b.y > a.y + lh {
                    selection.push(fill(
                        Bounds::from_corners(
                            point(bounds.left(), a.y + lh),
                            point(bounds.right(), b.y),
                        ),
                        wash,
                    ));
                }
                selection.push(fill(
                    Bounds::from_corners(point(bounds.left(), b.y), point(b.x, b.y + lh)),
                    wash,
                ));
            }
        }
        let at = if empty {
            Some(bounds.origin)
        } else {
            pos(cursor)
        };
        let key = (input.cursor(), input.content.len());
        if input.multiline
            && input.revealed != key
            && let Some(p) = at
        {
            let scroll = input.scroll.clone();
            let view = scroll.bounds();
            if view.size.height > Pixels::ZERO {
                let top = p.y - view.top();
                let mut off = scroll.offset();
                if top < Pixels::ZERO {
                    off.y -= top;
                } else if top + lh > view.size.height {
                    off.y -= top + lh - view.size.height;
                }
                if off != scroll.offset() {
                    scroll.set_offset(off);
                    window.refresh();
                }
                self.input.update(cx, |input, _| input.revealed = key);
            }
        }
        let cursor = sel
            .is_empty()
            .then_some(at)
            .flatten()
            .map(|p| fill(Bounds::new(p, size(px(1.5), lh)), colors::text1()));
        Prepaint {
            lines,
            selection,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let (multiline, shift) = {
            let input = self.input.read(cx);
            (input.multiline, input.shift)
        };
        // a slid single line clips to the box; a multiline one sits in its own scroll area
        let mask = (!multiline).then_some(ContentMask { bounds });
        window.with_content_mask(mask, |window| {
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            let lh = window.line_height();
            let seen = window.content_mask().bounds;
            let mut y = bounds.top();
            for (_, line) in &prepaint.lines {
                let h = lh * (line.wrap_boundaries().len() + 1) as f32;
                if y + h >= seen.top() && y <= seen.bottom() {
                    let origin = point(bounds.left() - shift, y);
                    let left = gpui::TextAlign::Left;
                    let _ = line.paint_background(origin, lh, left, None, window, cx);
                    let _ = line.paint(origin, lh, left, None, window, cx);
                }
                y += h;
            }
            if focus.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });
        let lh = window.line_height();
        let input = self.input.read(cx);
        let thumb = input.multiline.then(|| thumb(&input.scroll)).flatten();
        if let Some(t) = thumb {
            let alpha = if input.grab.is_some() { 0.3 } else { 0.16 };
            window.paint_quad(fill(t, colors::ink(alpha)).corner_radii(px(2.)));
        }
        let lines = std::mem::take(&mut prepaint.lines);
        self.input.update(cx, |input, _| {
            input.thumb = thumb;
            // the placeholder's layout must not answer for an empty box's offsets
            input.layout = if input.content.is_empty() {
                Vec::new()
            } else {
                lines
            };
            input.bounds = Some(bounds);
            input.line_height = lh;
        });
    }
}
