// Selecting text in a transcript and copying it. GPUI has no selection for plain text, and a
// reply is many text elements (paragraphs, list items, table cells, code blocks), so each one
// registers itself as it paints, in paint order, which is document order. A drag anchored in one
// element resolves against that list into spans: partial in the end elements, whole in between.
// Ctrl+C in the reply box copies the spans joined by newlines.
//
// Adapted from zeron's crates/ui/src/markdown/selection.rs and the selection half of its
// render.rs (MIT, see THIRD_PARTY_NOTICES.md), without its handling for a virtualized list: our
// transcript paints every item.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, Bounds, CursorStyle, DispatchPhase, HitboxBehavior, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, TextLayout, canvas,
    div, fill, point, prelude::*, size,
};

use crate::colors;

struct Selection {
    /// The transcript it lives in. Keys only need to be unique within one.
    surface: Arc<str>,
    anchor: Arc<str>,
    anchor_ix: usize,
    dragging: bool,
    /// Each selected element's key, the byte range picked out of it, and its text.
    spans: Vec<(Arc<str>, Range<usize>, SharedString)>,
}

struct Painted {
    surface: Arc<str>,
    key: Arc<str>,
    text: SharedString,
    layout: TextLayout,
}

thread_local! {
    static SELECTION: RefCell<Option<Selection>> = const { RefCell::new(None) };
    /// This frame's text elements, in paint order.
    static PAINTED: RefCell<Vec<Painted>> = const { RefCell::new(Vec::new()) };
    /// The transcript painting right now, set by its `reset`.
    static SURFACE: RefCell<Arc<str>> = RefCell::new(Arc::from(""));
}

/// Paint first in a transcript, before any of its text: forgets the transcript's elements from
/// the last frame and marks the text painted after it as its own.
pub fn reset(surface: Arc<str>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, _, _| {
            PAINTED.with(|p| p.borrow_mut().retain(|e| e.surface != surface));
            SURFACE.with(|s| *s.borrow_mut() = surface.clone());
        },
    )
    .absolute()
    .size_0()
}

/// `body`, the element drawing `text` through `layout`, made selectable. `key` must be unique in
/// its transcript and the same from frame to frame.
pub fn wrap(key: &str, text: SharedString, layout: TextLayout, body: AnyElement) -> AnyElement {
    let key: Arc<str> = Arc::from(key);
    let under = canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            let surface = SURFACE.with(|s| s.borrow().clone());
            window.set_cursor_style(CursorStyle::IBeam, &hitbox);
            if let Some(range) = wash(&surface, &key) {
                for rect in rects(&layout, &range) {
                    window.paint_quad(fill(rect, colors::selection()));
                }
            }
            PAINTED.with(|p| {
                p.borrow_mut().push(Painted {
                    surface: surface.clone(),
                    key: key.clone(),
                    text: text.clone(),
                    layout: layout.clone(),
                })
            });
            listen(
                window,
                hitbox,
                surface,
                key.clone(),
                text.clone(),
                layout.clone(),
            );
        },
    )
    .absolute()
    .size_full();
    div().relative().child(under).child(body).into_any_element()
}

/// The selected text, if any.
pub fn selected_text() -> Option<String> {
    SELECTION.with(|s| {
        let s = s.borrow();
        let s = s.as_ref()?;
        let text = join(&s.spans);
        (!text.is_empty()).then_some(text)
    })
}

/// Window-wide listeners for one element, registered each frame as it paints. Window-wide so a
/// drag keeps tracking outside the element; only the anchor's listeners move the drag.
fn listen(
    window: &mut gpui::Window,
    hitbox: gpui::Hitbox,
    surface: Arc<str>,
    key: Arc<str>,
    text: SharedString,
    layout: TextLayout,
) {
    {
        let (surface, key) = (surface.clone(), key.clone());
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                return;
            }
            if hitbox.is_hovered(window) && layout.bounds().contains(&e.position) {
                let ix = index(&layout, e.position);
                let range = match e.click_count {
                    1 => ix..ix,
                    2 => word(&text, ix),
                    _ => 0..text.len(),
                };
                begin(&surface, &key, &text, range);
                window.refresh();
            } else if clear_if_owner(&surface, &key) {
                window.refresh();
            }
        });
    }
    {
        let (surface, key) = (surface.clone(), key.clone());
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || !e.dragging() || !anchored(&surface, &key, true) {
                return;
            }
            if drag_to(&surface, e.position) {
                window.refresh();
            }
        });
    }
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, _| {
        if phase == DispatchPhase::Bubble && anchored(&surface, &key, true) {
            end();
        }
    });
}

fn index(layout: &TextLayout, at: Point<Pixels>) -> usize {
    match layout.index_for_position(at) {
        Ok(ix) | Err(ix) => ix,
    }
}

fn begin(surface: &Arc<str>, key: &Arc<str>, text: &SharedString, range: Range<usize>) {
    SELECTION.with(|s| {
        *s.borrow_mut() = Some(Selection {
            surface: surface.clone(),
            anchor: key.clone(),
            anchor_ix: range.start,
            dragging: true,
            spans: vec![(key.clone(), range, text.clone())],
        })
    });
}

/// Whether `key` in `surface` holds the selection, and is still dragging it when `dragging`.
fn anchored(surface: &Arc<str>, key: &Arc<str>, dragging: bool) -> bool {
    SELECTION.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.surface == *surface && s.anchor == *key && (s.dragging || !dragging))
    })
}

/// A press outside a settled selection's anchor clears it. True if it did.
fn clear_if_owner(surface: &Arc<str>, key: &Arc<str>) -> bool {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let owns = s
            .as_ref()
            .is_some_and(|s| s.surface == *surface && s.anchor == *key && !s.dragging);
        if owns {
            *s = None;
        }
        owns
    })
}

fn end() {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(sel) = s.as_mut() {
            sel.dragging = false;
            if sel.spans.iter().all(|(_, r, _)| r.is_empty()) {
                *s = None;
            }
        }
    });
}

/// Moves the drag's head to `at`: the transcript's element under it, or the nearest one, vertical
/// distance first so dragging past the end of a short line keeps that line. True if the
/// selection changed.
fn drag_to(surface: &Arc<str>, at: Point<Pixels>) -> bool {
    PAINTED.with(|p| {
        let painted = p.borrow();
        let mine: Vec<&Painted> = painted.iter().filter(|e| e.surface == *surface).collect();
        let gap = |lo: Pixels, hi: Pixels, v: Pixels| -> f32 {
            if v < lo {
                (lo - v).into()
            } else if v > hi {
                (v - hi).into()
            } else {
                0.
            }
        };
        let Some((head, _)) = mine.iter().enumerate().min_by(|(_, a), (_, b)| {
            let d = |e: &Painted| {
                let b = e.layout.bounds();
                (
                    gap(b.top(), b.bottom(), at.y),
                    gap(b.left(), b.right(), at.x),
                )
            };
            d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return false;
        };
        SELECTION.with(|s| {
            let mut s = s.borrow_mut();
            let Some(sel) = s.as_mut() else {
                return false;
            };
            let Some(anchor) = mine.iter().position(|e| e.key == sel.anchor) else {
                return false;
            };
            let texts: Vec<&str> = mine.iter().map(|e| e.text.as_ref()).collect();
            let head_ix = index(&mine[head].layout, at);
            let spans: Vec<_> = resolve(&texts, (anchor, sel.anchor_ix), (head, head_ix))
                .into_iter()
                .map(|(i, r)| (mine[i].key.clone(), r, mine[i].text.clone()))
                .collect();
            if spans == sel.spans {
                return false;
            }
            sel.spans = spans;
            true
        })
    })
}

/// The range of `key` to wash in `surface`, if it is selected.
fn wash(surface: &Arc<str>, key: &Arc<str>) -> Option<Range<usize>> {
    SELECTION.with(|s| {
        let s = s.borrow();
        let s = s.as_ref().filter(|s| s.surface == *surface)?;
        s.spans
            .iter()
            .find(|(k, r, _)| k == key && !r.is_empty())
            .map(|(_, r, _)| r.clone())
    })
}

/// The spans between `a` and `b`, each an (element, byte offset) into `texts`, in either order:
/// the element and the range picked out of it. Elements strictly between keep even an empty
/// range, so a blank line still gives its newline.
fn resolve(texts: &[&str], a: (usize, usize), b: (usize, usize)) -> Vec<(usize, Range<usize>)> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let mut out = Vec::new();
    for (i, text) in texts.iter().enumerate().take(end.0 + 1).skip(start.0) {
        let from = if i == start.0 { start.1 } else { 0 }.min(text.len());
        let to = if i == end.0 { end.1 } else { text.len() }.min(text.len());
        if from < to || (i > start.0 && i < end.0) {
            out.push((i, from..to));
        }
    }
    out
}

fn join(spans: &[(Arc<str>, Range<usize>, SharedString)]) -> String {
    spans
        .iter()
        .map(|(_, r, t)| t.get(r.clone()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The word around `ix` for a double click: a run of letters, digits and `_`, or the one
/// character there, or nothing at a space.
fn word(text: &str, ix: usize) -> Range<usize> {
    let mut ix = ix.min(text.len());
    while ix > 0 && !text.is_char_boundary(ix) {
        ix -= 1;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let before = text[..ix].chars().next_back();
    let at = text[ix..].chars().next();
    if !at.is_some_and(is_word) && !before.is_some_and(is_word) {
        return match at {
            Some(c) if !c.is_whitespace() => ix..ix + c.len_utf8(),
            _ => ix..ix,
        };
    }
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(i, _)| i);
    let end = text[ix..]
        .char_indices()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(i, c)| ix + i + c.len_utf8());
    start..end
}

/// The boxes to wash for `range`: one per visual line it covers, since soft wraps split it.
fn rects(layout: &TextLayout, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
    let bounds = layout.bounds();
    let line = layout.line_height();
    let at = |ix| layout.position_for_index(ix);
    let mut out = Vec::new();
    let mut cur = range.start;
    // a wrap can't make more rows than this; it stops a bad layout from spinning
    for _ in 0..256 {
        if cur >= range.end {
            break;
        }
        let Some(mut p1) = at(cur) else {
            break;
        };
        // GPUI puts a wrap boundary at the end of the row before it; start the box on the row
        // the next character is on
        if let Some(after) = at(cur + 1)
            && after.y > p1.y
        {
            p1 = point(bounds.left(), after.y);
        }
        let row_end = match at(range.end) {
            Some(pe) if pe.y == p1.y => range.end,
            _ => {
                // the last index still on this row
                let (mut lo, mut hi) = (cur, range.end);
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    match at(mid) {
                        Some(pm) if pm.y == p1.y => lo = mid,
                        _ => hi = mid,
                    }
                }
                lo
            }
        };
        if let Some(p2) = at(row_end)
            && p2.x > p1.x
        {
            out.push(Bounds::new(p1, size(p2.x - p1.x, line)));
        }
        if row_end <= cur {
            break;
        }
        cur = row_end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXTS: [&str; 3] = ["first paragraph", "second", "third one"];

    fn picked(spans: &[(usize, Range<usize>)]) -> Vec<&str> {
        spans.iter().map(|(i, r)| &TEXTS[*i][r.clone()]).collect()
    }

    #[test]
    fn a_drag_inside_one_element_picks_its_slice_either_way() {
        let spans = resolve(&TEXTS, (0, 6), (0, 15));
        assert_eq!(picked(&spans), ["paragraph"]);
        assert_eq!(resolve(&TEXTS, (0, 15), (0, 6)), spans);
    }

    #[test]
    fn a_drag_across_elements_takes_the_middle_whole() {
        let spans = resolve(&TEXTS, (2, 5), (0, 6));
        assert_eq!(picked(&spans), ["paragraph", "second", "third"]);
    }

    #[test]
    fn a_blank_line_between_keeps_its_newline() {
        let texts = ["first", "", "third"];
        let spans: Vec<_> = resolve(&texts, (0, 0), (2, 5))
            .into_iter()
            .map(|(i, r)| (Arc::from(""), r, SharedString::from(texts[i])))
            .collect();
        assert_eq!(join(&spans), "first\n\nthird");
    }

    #[test]
    fn a_double_click_takes_the_word() {
        let t = "let foo_bar = 12;";
        assert_eq!(&t[word(t, 5)], "foo_bar");
        assert_eq!(&t[word(t, 11)], "foo_bar");
        assert_eq!(&t[word(t, 12)], "=");
        assert_eq!(word(t, 3), 0..3);
        let u = "héllo wörld";
        assert_eq!(&u[word(u, 2)], "héllo");
    }
}
