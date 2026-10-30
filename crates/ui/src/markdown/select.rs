// Selecting text in a transcript and copying it. GPUI has no selection for plain text, and a
// reply is many text elements (paragraphs, list items, table cells, code blocks), so each one
// registers itself as it paints, in paint order, which is document order. A drag anchored in one
// element resolves against that list into spans: partial in the end elements, whole in between.
// A press beside or between elements starts at the nearest one, and Shift+click extends. Ctrl+C
// in the reply box copies the spans joined by newlines.
//
// Adapted from zeron's crates/ui/src/markdown/selection.rs and the selection half of its
// render.rs (MIT, see THIRD_PARTY_NOTICES.md). The transcript is a virtual list that only paints
// the rows on screen, so a selection keeps every element it has passed over, and a drag near an
// edge scrolls the list along.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, CursorStyle, DispatchPhase, HitboxBehavior, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, TextAlign,
    TextLayout, TextRun, Window, canvas, div, fill, point, prelude::*, size,
};

use crate::colors;
use crate::zoom::px;

struct Selection {
    /// The transcript it lives in. Keys only need to be unique within one.
    surface: Arc<str>,
    anchor: Arc<str>,
    /// What the press picked in the anchor: a point, a word or the whole element.
    anchor_range: Range<usize>,
    /// The press's click count, so a drag after a double click grows by words.
    clicks: usize,
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
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, area, window, _| {
            PAINTED.with(|p| p.borrow_mut().retain(|e| e.surface != surface));
            SURFACE.with(|s| *s.borrow_mut() = surface.clone());
            super::find::reset();
            track(window, surface.clone(), area);
        },
    )
    .absolute()
    .size_full()
}

pub(super) fn surface() -> Arc<str> {
    SURFACE.with(|s| s.borrow().clone())
}

pub fn tail(
    surface: Arc<str>,
    cite: impl Fn(String, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let at = surface.clone();
    canvas(
        move |bounds, window, _| {
            let end = end_of(&at)?;
            let style = window.text_style();
            let run = TextRun {
                len: CITE.len(),
                font: style.font(),
                color: colors::text1(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(CITE.into(), px(12.), &[run], None);
            let chip = size(line.width + px(18.), px(24.));
            let x = end
                .x
                .min(bounds.right() - chip.width - px(8.))
                .max(bounds.left() + px(8.));
            let y = end.y + px(4.);
            if y < bounds.top() || y + chip.height > bounds.bottom() {
                return None;
            }
            let rect = Bounds::new(point(x, y), chip);
            Some((
                rect,
                line,
                window.insert_hitbox(rect, HitboxBehavior::BlockMouse),
            ))
        },
        move |_, chip, window, cx| {
            if drag_to(&surface) {
                window.refresh();
            }
            let Some((rect, line, hitbox)) = chip else {
                return;
            };
            let ground = match hitbox.is_hovered(window) {
                true => colors::surface3(),
                false => colors::surface2(),
            };
            window.paint_quad(
                fill(rect, ground)
                    .corner_radii(px(6.))
                    .border_widths(px(1.))
                    .border_color(colors::border2()),
            );
            let origin = point(
                rect.left() + px(9.),
                rect.top() + (rect.size.height - px(16.)) / 2.,
            );
            line.paint(origin, px(16.), TextAlign::Left, None, window, cx)
                .ok();
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
            let was = hitbox.is_hovered(window);
            let hover = hitbox.clone();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble && hover.is_hovered(window) != was {
                    window.refresh();
                }
            });
            let surface = surface.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture
                    || e.button != MouseButton::Left
                    || !hitbox.is_hovered(window)
                {
                    return;
                }
                cx.stop_propagation();
                if let Some(text) = take(&surface) {
                    cite(text, window, cx);
                }
                window.refresh();
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

const CITE: &str = "Cite";

fn end_of(surface: &Arc<str>) -> Option<Point<Pixels>> {
    let (key, range) = SELECTION.with(|s| {
        let s = s.borrow();
        let s = s
            .as_ref()
            .filter(|s| s.surface == *surface && !s.dragging)?;
        let (key, range, _) = s.spans.iter().rev().find(|(_, r, _)| !r.is_empty())?;
        Some((key.clone(), range.clone()))
    })?;
    PAINTED.with(|p| {
        let p = p.borrow();
        let e = p.iter().find(|e| e.surface == *surface && e.key == key)?;
        rects(&e.layout, &range).last().map(|r| r.bottom_right())
    })
}

fn take(surface: &Arc<str>) -> Option<String> {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let text = join(&s.as_ref().filter(|s| s.surface == *surface)?.spans);
        *s = None;
        (!text.is_empty()).then_some(text)
    })
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
            super::find::paint(&surface, &key, &text, &layout, window);
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
            let layout = layout.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble
                    && e.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                    && layout.bounds().contains(&e.position)
                    && press(&surface, e)
                {
                    window.refresh();
                }
            });
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

// elements hear a press on their text before this. The area's hitbox is under everything in the
// transcript, so what blocks the mouse (the find bar, the Cite chip) keeps its presses, and a press
// outside the area clears a settled selection
fn track(window: &mut gpui::Window, surface: Arc<str>, area: gpui::Hitbox) {
    {
        let surface = surface.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || held(&surface) {
                return;
            }
            let changed = match area.is_hovered(window) {
                true => press(&surface, e),
                false => clear(&surface),
            };
            if changed {
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
    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _| {
        if phase == DispatchPhase::Bubble && end(&surface) {
            window.refresh();
        }
    });
}

fn index(layout: &TextLayout, at: Point<Pixels>) -> usize {
    match layout.index_for_position(at) {
        Ok(ix) | Err(ix) => ix,
    }
}

/// A press in the transcript: Shift grows the selection to it, else one starts at the nearest
/// element, a word on a double click and the whole element on a triple.
fn press(surface: &Arc<str>, e: &MouseDownEvent) -> bool {
    if e.modifiers.shift && extend(surface, e.position) {
        return true;
    }
    let near = PAINTED.with(|p| {
        let p = p.borrow();
        let mine: Vec<&Painted> = p.iter().filter(|e| e.surface == *surface).collect();
        nearest(&mine, e.position).map(|n| {
            let ix = index(&n.layout, e.position);
            (n.key.clone(), n.text.clone(), ix)
        })
    });
    let Some((key, text, ix)) = near else {
        return clear(surface);
    };
    begin(
        surface,
        &key,
        &text,
        pick(&text, ix, e.click_count),
        e.click_count,
    );
    true
}

/// What a press `clicks` times at `ix` picks: a point, the word there, or all of `text`.
fn pick(text: &str, ix: usize, clicks: usize) -> Range<usize> {
    match clicks {
        0 | 1 => ix..ix,
        2 => word(text, ix),
        _ => 0..text.len(),
    }
}

fn held(surface: &Arc<str>) -> bool {
    SELECTION.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.surface == *surface && s.dragging)
    })
}

fn extend(surface: &Arc<str>, at: Point<Pixels>) -> bool {
    let held = SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let Some(sel) = s.as_mut().filter(|s| s.surface == *surface && !s.dragging) else {
            return false;
        };
        sel.dragging = true;
        sel.at = Some(at);
        true
    });
    if held {
        drag_to(surface);
    }
    held
}

fn begin(
    surface: &Arc<str>,
    key: &Arc<str>,
    text: &SharedString,
    range: Range<usize>,
    clicks: usize,
) {
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
            anchor_range: range.clone(),
            clicks,
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

fn end(surface: &Arc<str>) -> bool {
    SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let Some(sel) = s.as_mut().filter(|s| s.surface == *surface && s.dragging) else {
            return false;
        };
        sel.dragging = false;
        if sel.spans.iter().all(|(_, r, _)| r.is_empty()) {
            *s = None;
        }
        true
    })
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
        SELECTION.with(|s| {
            let mut s = s.borrow_mut();
            let Some(sel) = s.as_mut().filter(|s| s.surface == *surface && s.dragging) else {
                return false;
            };
            let Some(at) = sel.at else {
                return false;
            };
            let Some(head) = nearest(&mine, at) else {
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
            let picked = pick(&head.text, index(&head.layout, at), sel.clicks);
            let (a, b) = ends((anchor, sel.anchor_range.clone()), (to, picked));
            let spans: Vec<_> = resolve(&texts, a, b)
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

/// The element under `at`, or the nearest one, vertical distance first so a point past the end of
/// a short line keeps that line.
fn nearest<'a>(mine: &[&'a Painted], at: Point<Pixels>) -> Option<&'a Painted> {
    let gap = |lo: Pixels, hi: Pixels, v: Pixels| -> f32 {
        if v < lo {
            (lo - v).into()
        } else if v > hi {
            (v - hi).into()
        } else {
            0.
        }
    };
    let d = |e: &Painted| {
        let b = e.layout.bounds();
        (
            gap(b.top(), b.bottom(), at.y),
            gap(b.left(), b.right(), at.x),
        )
    };
    mine.iter()
        .copied()
        .min_by(|a, b| d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal))
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

/// The two ends of a drag from what the press picked to what the pointer is on, each an (element,
/// byte offset): whichever comes first gives its start and the other its end, so the words or
/// elements at both ends stay whole.
fn ends(
    anchor: (usize, Range<usize>),
    head: (usize, Range<usize>),
) -> ((usize, usize), (usize, usize)) {
    if (head.0, head.1.start) >= (anchor.0, anchor.1.start) {
        ((anchor.0, anchor.1.start), (head.0, head.1.end))
    } else {
        ((head.0, head.1.start), (anchor.0, anchor.1.end))
    }
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
pub(super) fn rects(layout: &TextLayout, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
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
    fn a_drag_after_a_double_click_takes_whole_words_either_way() {
        let t = "first paragraph";
        let forward = ends((0, word(t, 1)), (2, word(TEXTS[2], 7)));
        assert_eq!(
            picked(&resolve(&TEXTS, forward.0, forward.1)),
            ["first paragraph", "second", "third one"]
        );
        let back = ends((2, word(TEXTS[2], 7)), (0, word(t, 9)));
        assert_eq!(
            picked(&resolve(&TEXTS, back.0, back.1)),
            ["paragraph", "second", "third one"]
        );
        let inside = ends((0, word(t, 9)), (0, word(t, 2)));
        assert_eq!(
            picked(&resolve(&TEXTS, inside.0, inside.1)),
            ["first paragraph"]
        );
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
