// Selecting text in a transcript and copying it. GPUI has no selection for plain text, and a
// reply is many text elements (paragraphs, list items, table cells, code blocks), so each one
// registers itself as it paints, in paint order, which is document order. A drag anchored in one
// element resolves against that list into spans: partial in the end elements, whole in between.
// Ctrl+C in the reply box copies the spans joined by newlines.
//
// Adapted from zeron's crates/ui/src/markdown/selection.rs and the selection half of its
// render.rs (MIT, see THIRD_PARTY_NOTICES.md). The transcript is a virtual list that only paints
// the rows on screen, so a selection keeps every element it has passed over, and a drag near an
// edge scrolls the list along.

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
    seen: Vec<(Arc<str>, SharedString)>,
    at: Option<Point<Pixels>>,
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
        move |_, _, window, _| {
            PAINTED.with(|p| p.borrow_mut().retain(|e| e.surface != surface));
            SURFACE.with(|s| *s.borrow_mut() = surface.clone());
            track(window, surface.clone());
        },
    )
    .absolute()
    .size_0()
}

pub fn tail(surface: Arc<str>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            if drag_to(&surface) {
                window.refresh();
            }
        },
    )
    .absolute()
    .size_0()
}

pub fn dragging(surface: &Arc<str>) -> Option<Point<Pixels>> {
    SELECTION.with(|s| {
        s.borrow()
            .as_ref()
            .filter(|s| s.surface == *surface && s.dragging)
            .and_then(|s| s.at)
    })
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

fn listen(
    window: &mut gpui::Window,
    hitbox: gpui::Hitbox,
    surface: Arc<str>,
    key: Arc<str>,
    text: SharedString,
    layout: TextLayout,
) {
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
        }
    });
}

// elements hear a press before this, so a press that started no selection clears a settled one
fn track(window: &mut gpui::Window, surface: Arc<str>) {
    {
        let surface = surface.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && clear(&surface) {
                window.refresh();
            }
        });
    }
    {
        let surface = surface.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble
                && e.dragging()
                && aim(&surface, e.position)
                && drag_to(&surface)
            {
                window.refresh();
            }
        });
    }
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, _| {
        if phase == DispatchPhase::Bubble {
            end(&surface);
        }
    });
}

fn index(layout: &TextLayout, at: Point<Pixels>) -> usize {
    match layout.index_for_position(at) {
        Ok(ix) | Err(ix) => ix,
    }
}

fn begin(surface: &Arc<str>, key: &Arc<str>, text: &SharedString, range: Range<usize>) {
    let seen = PAINTED.with(|p| {
        p.borrow()
            .iter()
            .filter(|e| e.surface == *surface)
            .map(|e| (e.key.clone(), e.text.clone()))
            .collect()
    });
    SELECTION.with(|s| {
        *s.borrow_mut() = Some(Selection {
            surface: surface.clone(),
            anchor: key.clone(),
            anchor_ix: range.start,
            dragging: true,
            spans: vec![(key.clone(), range, text.clone())],
            seen,
            at: None,
        })
    });
}

fn clear(surface: &Arc<str>) -> bool {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let owns = s
            .as_ref()
            .is_some_and(|s| s.surface == *surface && !s.dragging);
        if owns {
            *s = None;
        }
        owns
    })
}

fn aim(surface: &Arc<str>, at: Point<Pixels>) -> bool {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        match s.as_mut().filter(|s| s.surface == *surface && s.dragging) {
            Some(sel) => {
                sel.at = Some(at);
                true
            }
            None => false,
        }
    })
}

fn end(surface: &Arc<str>) {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(sel) = s.as_mut().filter(|s| s.surface == *surface && s.dragging) {
            sel.dragging = false;
            sel.seen.clear();
            if sel.spans.iter().all(|(_, r, _)| r.is_empty()) {
                *s = None;
            }
        }
    });
}

fn item(key: &str) -> Option<usize> {
    key.trim_start_matches(|c: char| !c.is_ascii_digit())
        .split('-')
        .next()?
        .parse()
        .ok()
}

fn merge(seen: &mut Vec<(Arc<str>, SharedString)>, painted: &[(Arc<str>, SharedString)]) {
    if painted.iter().any(|(k, _)| item(k).is_none()) {
        *seen = painted.to_vec();
        return;
    }
    let mut rest = painted;
    while let Some((first, _)) = rest.first() {
        let n = item(first);
        let len = rest.iter().take_while(|(k, _)| item(k) == n).count();
        seen.retain(|(k, _)| item(k) != n);
        let at = seen
            .iter()
            .position(|(k, _)| item(k) > n)
            .unwrap_or(seen.len());
        seen.splice(at..at, rest[..len].iter().cloned());
        rest = &rest[len..];
    }
}

/// Moves the drag's head to where the pointer is: the transcript's element under it, or the
/// nearest one, vertical distance first so dragging past the end of a short line keeps that line.
/// True if the selection changed.
fn drag_to(surface: &Arc<str>) -> bool {
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
        SELECTION.with(|s| {
            let mut s = s.borrow_mut();
            let Some(sel) = s.as_mut().filter(|s| s.surface == *surface && s.dragging) else {
                return false;
            };
            let Some(at) = sel.at else {
                return false;
            };
            let Some(head) = mine.iter().min_by(|a, b| {
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
            let now: Vec<_> = mine
                .iter()
                .map(|e| (e.key.clone(), e.text.clone()))
                .collect();
            merge(&mut sel.seen, &now);
            let find = |key: &Arc<str>| sel.seen.iter().position(|(k, _)| k == key);
            let (Some(anchor), Some(to)) = (find(&sel.anchor), find(&head.key)) else {
                return false;
            };
            let texts: Vec<&str> = sel.seen.iter().map(|(_, t)| t.as_ref()).collect();
            let spans: Vec<_> = resolve(
                &texts,
                (anchor, sel.anchor_ix),
                (to, index(&head.layout, at)),
            )
            .into_iter()
            .map(|(i, r)| (sel.seen[i].0.clone(), r, sel.seen[i].1.clone()))
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
    fn rows_that_scroll_away_stay_in_the_selection_in_order() {
        let el = |k: &str| (Arc::<str>::from(k), SharedString::from(k.to_string()));
        let mut seen = vec![el("t3-0"), el("t3-1"), el("u4")];
        merge(&mut seen, &[el("u4"), el("t5-0"), el("t6-0")]);
        merge(&mut seen, &[el("t1-0"), el("t2-0")]);
        let keys: Vec<&str> = seen.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(keys, ["t1-0", "t2-0", "t3-0", "t3-1", "u4", "t5-0", "t6-0"]);
        merge(&mut seen, &[el("t5-0"), el("t5-1")]);
        assert_eq!(seen.len(), 8);
        assert_eq!(item("plan12-0"), Some(12));
        assert_eq!(item("agent3"), Some(3));
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
