// Lays out and paints a `TextInput`: the text wrapped to the box's width, the selection, the
// IME's marked text underlined, and the cursor. The layout is kept on the input so clicks and
// arrow keys can map between offsets and positions.

use gpui::{
    App, AvailableSpace, Bounds, ElementId, ElementInputHandler, Entity, GlobalElementId, LayoutId,
    PaintQuad, Pixels, SharedString, Style, TextRun, UnderlineStyle, Window, WrappedLine, fill,
    point, prelude::*, px, relative, size,
};

use super::TextInput;
use crate::colors;

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
    width: Option<Pixels>,
    window: &mut Window,
) -> Vec<(usize, WrappedLine)> {
    let style = window.text_style();
    let (text, color): (SharedString, _) = if input.content.is_empty() {
        (input.placeholder.clone(), colors::text3())
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
        None => vec![run],
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
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let lines = shape(input.read(cx), width, window);
            let height = window.line_height() * rows(&lines) as f32;
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
        let lines = shape(input, Some(bounds.size.width), window);
        let lh = window.line_height();
        let empty = input.content.is_empty();
        let (sel, cursor) = (input.selected.clone(), input.cursor());

        // where an offset lands, in window coordinates, using the fresh layout
        let pos = |offset: usize| {
            let mut y = bounds.top();
            for (start, line) in &lines {
                if offset >= *start && offset <= start + line.len() {
                    let p = line.position_for_index(offset - start, lh)?;
                    return Some(point(bounds.left() + p.x, y + p.y));
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
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        let lh = window.line_height();
        let mut y = bounds.top();
        for (_, line) in &prepaint.lines {
            let _ = line.paint(
                point(bounds.left(), y),
                lh,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            y += lh * (line.wrap_boundaries().len() + 1) as f32;
        }
        if focus.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        let lines = std::mem::take(&mut prepaint.lines);
        self.input.update(cx, |input, _| {
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
