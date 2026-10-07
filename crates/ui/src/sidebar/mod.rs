// The sidebar: a search box and a New thread button on top, then every thread at work as one
// list, most recently active first, after T3 Code's: each row carries its project as a colored
// tag with the project's name, so a space needs no header of its own. Snoozed, Settled and
// Archived sit below as shelves behind a rule, and Settings at the foot. Right-click a thread for
// its menu and its project's; drag the right edge to resize. The drag handle follows zeron's
// shell (MIT, see THIRD_PARTY_NOTICES.md).
//
// It is a view of its own, cached, so a frame that only changes a terminal reuses its layout. Its
// rows are a virtual list: only the ones on screen are laid out, which is what keeps scrolling
// smooth with dozens of threads. The wheel eases the list along instead of jumping a notch at a
// time, the way the Tauri app's webview scrolled. A thread that joins the list grows in from
// nothing as it fades in, after T3 Code's, so the rows under it glide down instead of jumping.

mod card;
mod drag;
mod row;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, ClickEvent, Context, DispatchPhase, DragMoveEvent, Entity, ExternalPaths,
    Focusable, FontWeight, HitboxBehavior, IntoElement, ListAlignment, ListOffset, ListState,
    MouseButton, MouseDownEvent, Pixels, ScrollWheelEvent, Subscription, Transformation,
    WeakEntity, Window, canvas, div, list, percentage, prelude::*, px,
};
use hyprspace_proto::{Space, Thread};
use hyprspace_theme::MONO;

use crate::assets::icon;
use crate::colors;
use crate::palette::TogglePalette;
use crate::root::{MenuItems, Root, Screen, SidebarDrag};
use crate::slide::{animations, ease_out};
use crate::time::now_ms;
use crate::workbench::PaneDrag;

pub const MIN_WIDTH: f32 = 200.;
pub const MAX_WIDTH: f32 = 480.;
/// Rows laid out past each edge of the list, so a quick scroll doesn't show them arriving.
const OVERDRAW: f32 = 240.;
/// Rows sit this far in from the sidebar's edges.
const EDGE: f32 = 8.;
/// How fast an eased scroll settles: each frame covers this share of what is left per second's
/// worth of time constant. About 60 ms, the feel of a browser's smooth scrolling.
const EASE: f32 = 0.06;
/// A wheel line's worth of travel.
const LINE: f32 = 20.;
/// While a row is dragged, the list scrolls when the pointer is this close to its top or bottom,
/// up to this many pixels a frame, faster nearer the edge.
const EDGE_ZONE: f32 = 56.;
const EDGE_SPEED: f32 = 14.;
/// How long a row that joins the list takes to grow in, T3 Code's 150 ms.
const ARRIVE: Duration = Duration::from_millis(150);
/// More rows than this joining at once, like a search cleared, just appear.
const MAX_ARRIVALS: usize = 40;

/// How far the list scrolls this frame for a drag at `y` in a list spanning `top..bottom`:
/// negative up, positive down, nothing away from the edges.
fn edge_scroll(y: f32, top: f32, bottom: f32) -> f32 {
    if y < top + EDGE_ZONE {
        -EDGE_SPEED * (1. - ((y - top) / EDGE_ZONE).clamp(0., 1.))
    } else if y > bottom - EDGE_ZONE {
        EDGE_SPEED * (1. - ((bottom - y) / EDGE_ZONE).clamp(0., 1.))
    } else {
        0.
    }
}

/// One line of the sidebar's list.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// A thread at work.
    Thread(u64),
    /// A thread on the Snoozed or Settled shelf.
    Shelved(u64),
    /// Room above the list, between shelves, or at the end.
    Gap(u8),
    /// The snoozed threads of every space.
    Snoozed { count: usize, open: bool },
    /// The settled threads of every space.
    Settled { count: usize, open: bool },
    /// Nothing to list: no threads yet, or a search with no hits.
    Nothing { searching: bool },
}

impl Item {
    /// The same row, whatever it shows: a shelf that opens or shuts stays where it is, and only
    /// the rows that come or go are spliced.
    fn same(&self, other: &Item) -> bool {
        match (self, other) {
            (Item::Snoozed { .. }, Item::Snoozed { .. }) => true,
            (Item::Settled { .. }, Item::Settled { .. }) => true,
            (Item::Nothing { .. }, Item::Nothing { .. }) => true,
            (a, b) => a == b,
        }
    }
}

/// The range of `old` that differs from `new`, and how many items replace it. Splicing only that
/// keeps the scroll where it was when a space folds or a thread arrives.
fn changed(old: &[Item], new: &[Item]) -> (Range<usize>, usize) {
    let head = old.iter().zip(new).take_while(|(a, b)| a.same(b)).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a.same(b))
        .count();
    (head..old.len() - tail, new.len() - head - tail)
}

/// The threads in `new` that `old` didn't list.
fn arrived(old: &[Item], new: &[Item]) -> Vec<u64> {
    new.iter()
        .filter(|i| !old.contains(i))
        .filter_map(|i| match i {
            Item::Thread(id) | Item::Shelved(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// A row growing into the list: when it joined, and its full height once it has been laid out.
#[derive(Clone)]
struct Arrival {
    since: Instant,
    height: Rc<Cell<Option<Pixels>>>,
}

impl Arrival {
    /// `row` as far as it has grown in. Its own height is only known after a first layout, so
    /// the first frame draws it at nothing, which is where it starts anyway.
    fn draw(&self, row: AnyElement, window: &mut Window) -> AnyElement {
        let t = self.since.elapsed().as_secs_f32() / ARRIVE.as_secs_f32();
        if t >= 1. {
            return row;
        }
        window.request_animation_frame();
        let t = ease_out(t);
        let height = self.height.clone();
        div()
            .w_full()
            .h(self.height.get().map_or(px(0.), |h| h * t))
            .overflow_hidden()
            .opacity(t)
            // pinned to the bottom, so it slides down into place as the room opens
            .flex()
            .flex_col()
            .justify_end()
            .child(div().w_full().flex_none().child(row))
            .on_children_prepainted(move |b, _, _| {
                if let Some(b) = b.first() {
                    height.set(Some(b.size.height));
                }
            })
            .into_any_element()
    }
}

/// The sidebar as a view of its own. It draws from the root's state and redraws whenever the
/// root does.
pub struct SidebarView {
    root: WeakEntity<Root>,
    list: ListState,
    items: Rc<Vec<Item>>,
    /// The root changed since the last draw, so rows may have changed height.
    stale: bool,
    /// Wheel travel not scrolled yet, positive toward the end, and when the last step went.
    pending: Rc<Cell<f32>>,
    stepped: Option<Instant>,
    /// While a row is dragged near the list's top or bottom, how far to scroll each frame.
    edge: f32,
    /// Threads growing into the list.
    arrivals: Rc<RefCell<HashMap<u64, Arrival>>>,
    /// The rows last listed were the saved state's, not the empty list before it loaded, which
    /// would make every row at launch look new.
    loaded: bool,
    _watch: Subscription,
}

impl SidebarView {
    pub fn new(root: &Entity<Root>, cx: &mut Context<Self>) -> Self {
        Self {
            root: root.downgrade(),
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            items: Rc::default(),
            stale: true,
            pending: Rc::default(),
            stepped: None,
            edge: 0.,
            arrivals: Rc::default(),
            loaded: false,
            _watch: cx.observe(root, |v: &mut Self, _, cx| {
                v.stale = true;
                cx.notify();
            }),
        }
    }
}

impl SidebarView {
    /// One frame of an eased scroll: part of the pending travel, more the longer the frame took.
    fn ease(&mut self, window: &mut Window) {
        let left = self.pending.get();
        if left == 0. {
            self.stepped = None;
            return;
        }
        let now = Instant::now();
        let dt = self
            .stepped
            .replace(now)
            .map_or(1. / 120., |t| (now - t).as_secs_f32().min(0.05));
        let step = if left.abs() < 0.5 {
            left
        } else {
            left * (1. - (-dt / EASE).exp())
        };
        self.list.scroll_by(px(step));
        self.pending.set(left - step);
        window.request_animation_frame();
    }

    /// Takes the wheel over the list before the list sees it, so it can be eased. A popup over
    /// the sidebar keeps its own wheel: the hitbox only counts while nothing covers it.
    fn wheel(&self, cx: &mut Context<Self>) -> AnyElement {
        let pending = self.pending.clone();
        let view = cx.weak_entity();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture || !hitbox.is_hovered(window) {
                        return;
                    }
                    let dy = e.delta.pixel_delta(px(LINE)).y;
                    pending.set(pending.get() - f32::from(dy));
                    cx.stop_propagation();
                    let _ = view.update(cx, |_, cx| cx.notify());
                });
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}

impl gpui::Render for SidebarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(root) = self.root.upgrade() else {
            return div().into_any_element();
        };
        let (items, loaded) = root.update(cx, |r, cx| (r.sidebar_items(cx), r.loaded));
        self.arrivals
            .borrow_mut()
            .retain(|_, a| a.since.elapsed() < ARRIVE);
        if items != *self.items {
            let new = arrived(&self.items, &items);
            if self.loaded && animations() && new.len() <= MAX_ARRIVALS {
                let since = Instant::now();
                let mut arrivals = self.arrivals.borrow_mut();
                for id in new {
                    arrivals.insert(
                        id,
                        Arrival {
                            since,
                            height: Rc::default(),
                        },
                    );
                }
            }
            let (range, count) = changed(&self.items, &items);
            if !range.is_empty() || count > 0 {
                // the view stays on the row it showed at the top: a splice over that row would reset
                // it, and a row moved from above would shift every index under it by one
                let top = self.list.logical_scroll_top();
                let anchor = self.items.get(top.item_ix).cloned();
                self.list.splice(range, count);
                let item_ix = anchor
                    .and_then(|a| items.iter().position(|i| i.same(&a)))
                    .unwrap_or(top.item_ix);
                self.list.scroll_to(ListOffset { item_ix, ..top });
            }
            self.items = Rc::new(items);
        }
        self.loaded = loaded;
        // a thread just opened or made shows its row, even with the list scrolled past it. This
        // runs before the remeasure below, which forgets every row's height: a row clicked in
        // plain view has to still read as shown, or each click would scroll the list
        if let Some(id) = root.update(cx, |r, _| r.reveal.take())
            && let Some(ix) = self
                .items
                .iter()
                .position(|i| matches!(i, Item::Thread(t) | Item::Shelved(t) if *t == id))
        {
            // by place in the list, not by height: rows far from view were never measured. A row
            // above the view, or not wholly in it, comes to the top with the one before it showing
            let view = self.list.viewport_bounds();
            let shown = self
                .list
                .bounds_for_item(ix)
                .is_some_and(|b| b.top() >= view.top() && b.bottom() <= view.bottom());
            if !shown {
                self.pending.set(0.);
                self.list.scroll_to(ListOffset {
                    item_ix: ix.saturating_sub(1),
                    offset_in_item: px(0.),
                });
            }
        }
        // a status, a line of activity or a subagent can change a row's height
        if std::mem::take(&mut self.stale) {
            self.list.remeasure();
        }
        self.ease(window);
        // a row held near an edge keeps the list moving, frame after frame, until it moves away
        if self.edge != 0. {
            if cx.has_active_drag() {
                self.list.scroll_by(px(self.edge));
                window.request_animation_frame();
                // rows slide under a still pointer, so no line is right until it moves again
                root.update(cx, |r, _| r.drop_at = None);
            } else {
                self.edge = 0.;
            }
        }
        let wheel = self.wheel(cx);
        let (items, weak, now) = (self.items.clone(), self.root.clone(), now_ms());
        let arrivals = self.arrivals.clone();
        let rows = list(self.list.clone(), move |ix, window, cx| {
            let item = items.get(ix).cloned();
            let arrival = match item {
                Some(Item::Thread(id) | Item::Shelved(id)) => arrivals.borrow().get(&id).cloned(),
                _ => None,
            };
            let row = weak
                .update(cx, |r, cx| match item {
                    Some(item) => r.sidebar_item(&item, now, cx),
                    None => div().into_any_element(),
                })
                .unwrap_or_else(|_| div().into_any_element());
            match arrival {
                Some(a) => a.draw(row, window),
                None => row,
            }
        })
        .size_full();
        let rows = div()
            .id("sidebar-list")
            .relative()
            .size_full()
            .on_drag_move(cx.listener(|v, e: &DragMoveEvent<PaneDrag>, _, cx| {
                let (p, b) = (e.event.position, e.bounds);
                let inside = p.x >= b.left() && p.x <= b.right();
                let edge = if inside {
                    edge_scroll(f32::from(p.y), f32::from(b.top()), f32::from(b.bottom()))
                } else {
                    0.
                };
                if edge != v.edge {
                    v.edge = edge;
                    cx.notify();
                }
            }))
            .child(rows)
            .child(wheel)
            .into_any_element();
        root.update(cx, |r, cx| r.sidebar(rows, cx))
    }
}

/// The wash a sidebar row lifts to on hover.
pub(crate) fn row_hover() -> gpui::Hsla {
    colors::surface3().opacity(0.55)
}

/// A shelf's arrow, as T3 Code's: up while it is open, down while it is shut.
fn twist(open: bool) -> impl IntoElement {
    icon("chevron-right", 12., colors::text3()).with_transformation(Transformation::rotate(
        percentage(if open { 0.75 } else { 0.25 }),
    ))
}

/// A row's place in the list: in from the edges, with a little air above.
fn slot() -> gpui::Div {
    div().pl(px(EDGE)).pr(px(EDGE)).pt(px(2.))
}

impl Root {
    /// The sidebar's rows, top to bottom: every thread at work in its place (new ones on top,
    /// moved ones where they were dropped, and none moved by activity); then the Snoozed and
    /// Settled shelves, most recently active first. A search keeps the threads whose title or project
    /// matches, and opens the shelves that hold one.
    fn sidebar_items(&self, cx: &mut Context<Self>) -> Vec<Item> {
        let q = self.search.read(cx).text().trim().to_lowercase();
        let hit = |s: &Space, t: &Thread| {
            q.is_empty()
                || t.title.to_lowercase().contains(&q)
                || s.name.to_lowercase().contains(&q)
        };
        let threads = |keep: fn(&Thread) -> bool, key: fn(&Thread) -> i64| -> Vec<&Thread> {
            let mut all: Vec<&Thread> = self
                .state
                .spaces
                .iter()
                .filter(|s| !s.archived)
                .flat_map(|s| s.threads.iter().filter(move |t| keep(t) && hit(s, t)))
                .collect();
            all.sort_by_key(|t| std::cmp::Reverse(key(t)));
            all
        };
        let recent = |t: &Thread| t.last_touch() as i64;
        let mut items = vec![Item::Gap(2)];
        let active = threads(Thread::active, Thread::rank);
        if active.is_empty() {
            items.push(Item::Nothing {
                searching: !q.is_empty(),
            });
        }
        items.extend(active.iter().map(|t| Item::Thread(t.id)));
        let searching = !q.is_empty();
        let snoozed = threads(|t| t.snooze.is_some(), recent);
        if !snoozed.is_empty() {
            let open = self.snoozed_open || searching;
            items.push(Item::Snoozed {
                count: snoozed.len(),
                open,
            });
            if open {
                items.extend(snoozed.iter().map(|t| Item::Shelved(t.id)));
            }
        }
        let settled = threads(|t| t.settled, recent);
        if !settled.is_empty() {
            let open = self.settled_open || searching;
            items.push(Item::Settled {
                count: settled.len(),
                open,
            });
            if open {
                items.extend(settled.iter().map(|t| Item::Shelved(t.id)));
            }
        }
        items.push(Item::Gap(4));
        items
    }

    fn sidebar_item(&self, item: &Item, now: u64, cx: &mut Context<Self>) -> AnyElement {
        match *item {
            Item::Thread(id) => match self.state.thread(id) {
                Some((_, t)) => slot().child(self.thread_row(t, now, cx)).into_any_element(),
                None => div().into_any_element(),
            },
            Item::Shelved(id) => match self.state.thread(id) {
                Some((_, t)) => slot()
                    .child(self.shelved_row(t, now, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Item::Gap(h) => div().h(px(h as f32)).into_any_element(),
            Item::Snoozed { count, open } => slot()
                .pt(px(10.))
                .child(self.shelf_header(
                    "snoozed",
                    "Snoozed",
                    count,
                    open,
                    cx.listener(|r, _: &ClickEvent, _, cx| {
                        r.snoozed_open = !r.snoozed_open;
                        cx.notify();
                    }),
                ))
                .into_any_element(),
            Item::Settled { count, open } => slot()
                .pt(px(10.))
                .child(
                    div()
                        .id("settled-shelf")
                        .rounded(px(7.))
                        .drag_over::<PaneDrag>(|s, _, _, _| s.bg(colors::accent().opacity(0.12)))
                        .on_drop(cx.listener(|r, d: &PaneDrag, window, cx| {
                            r.drop_on_settled(d, window, cx)
                        }))
                        .child(self.shelf_header(
                            "settled",
                            "Settled",
                            count,
                            open,
                            cx.listener(|r, _: &ClickEvent, _, cx| {
                                r.settled_open = !r.settled_open;
                                cx.notify();
                            }),
                        )),
                )
                .into_any_element(),
            Item::Nothing { searching } => div()
                .px(px(EDGE + 10.))
                .py(px(6.))
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(if searching {
                    "Nothing matches."
                } else {
                    "No threads yet."
                })
                .into_any_element(),
        }
    }

    /// The sidebar's frame around its list of rows.
    pub(crate) fn sidebar(&mut self, rows: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let q = self.search.read(cx).text().trim().to_lowercase();
        let width = self.state.sidebar_width.clamp(MIN_WIDTH, MAX_WIDTH);
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors::bg())
            .border_r_1()
            .border_color(colors::border0())
            .child(self.nav_row(&q, cx))
            .child(
                div()
                    .id("sidebar-rows")
                    .flex_1()
                    .min_h_0()
                    .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(colors::accent().opacity(0.06)))
                    .on_drop(cx.listener(|r, paths: &ExternalPaths, window, cx| {
                        r.drop_folders(paths, window, cx)
                    }))
                    .child(rows),
            )
            .child(self.foot(cx))
            .child(
                div()
                    .id("sidebar-resize")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(-3.))
                    .w(px(6.))
                    .cursor_col_resize()
                    .hover(|s| s.bg(colors::accent().opacity(0.45)))
                    .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| DragGhost)),
            )
            .into_any_element()
    }

    /// The top row: the search box, with the palette's shortcut as a keycap until something is
    /// typed, and a square New thread button that asks for a folder.
    fn nav_row(&self, q: &str, cx: &mut Context<Self>) -> AnyElement {
        let end = if q.is_empty() {
            div()
                .id("search-key")
                .flex_none()
                .h(px(20.))
                .px(px(5.))
                .flex()
                .items_center()
                .rounded(px(5.))
                .border_1()
                .border_color(colors::border1())
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .cursor_pointer()
                .hover(|s| {
                    s.text_color(colors::text1())
                        .border_color(colors::border2())
                })
                .child(if cfg!(target_os = "macos") {
                    "Cmd K"
                } else {
                    "Ctrl K"
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                    r.toggle_palette(&TogglePalette, window, cx)
                }))
        } else {
            div()
                .id("search-clear")
                .flex_none()
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.08)))
                .child(icon("x", 12., colors::text3()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                    r.search.update(cx, |i, cx| i.set_text("", cx));
                    cx.notify();
                }))
        };
        let focused = self.search.focus_handle(cx);
        let glass = if q.is_empty() {
            colors::text3()
        } else {
            colors::accent()
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(10.))
            .mb(px(4.))
            .border_b_1()
            .border_color(colors::border0())
            .child(
                div()
                    .id("sidebar-search")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(32.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .hover(|s| s.bg(colors::ink(0.06)))
                    .cursor_text()
                    .child(icon("search", 14., glass))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(colors::text1())
                            .child(self.search.clone()),
                    )
                    .child(end)
                    .on_click(move |_, window, cx| window.focus(&focused, cx)),
            )
            .child(
                div()
                    .id("sidebar-new")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(32.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)))
                    .child(icon("square-pen", 15., colors::text2()))
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                        r.menu = None;
                        r.pick_thread_folder(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// Opens `items` as a menu where the right-click was.
    pub(crate) fn context_menu(
        &self,
        items: MenuItems,
        cx: &mut Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        cx.listener(move |r, e: &MouseDownEvent, _, cx| {
            r.menu = Some((e.position, items.clone()));
            r.menu_sub = None;
            cx.stop_propagation();
            cx.notify();
        })
    }

    /// A shelf's heading, after T3 Code's: its name and count, a rule, and the arrow.
    fn shelf_header(
        &self,
        id: &'static str,
        label: &'static str,
        count: usize,
        open: bool,
        toggle: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(26.))
            .px(px(8.))
            .rounded(px(7.))
            .text_size(px(11.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|s| s.text_color(colors::text2()))
            .child(label)
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .child(count.to_string()),
            )
            .child(div().flex_1().h(px(1.)).bg(colors::ink(0.08)))
            .child(twist(open))
            .on_click(toggle)
            .into_any_element()
    }

    fn foot(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.screen == Screen::Settings;
        div()
            .flex_none()
            .flex()
            .items_center()
            .px(px(8.))
            .pt(px(7.))
            .pb(px(8.))
            .border_t_1()
            .border_color(colors::border0())
            .child(
                div()
                    .id("settings")
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .text_color(if on { colors::text1() } else { colors::text2() })
                    .cursor_pointer()
                    .when(on, |d| d.bg(colors::surface3()))
                    .when(!on, |d| {
                        d.hover(|s| s.bg(row_hover()).text_color(colors::text1()))
                    })
                    .child(icon("settings", 16., colors::text3()))
                    .child("Settings")
                    .on_click(
                        cx.listener(|r, _: &ClickEvent, window, cx| r.open_settings(window, cx)),
                    ),
            )
            .into_any_element()
    }
}

/// What follows the pointer while the sidebar edge is dragged: nothing visible.
struct DragGhost;

impl gpui::Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_changed_is_spliced() {
        let a = [Item::Gap(1), Item::Thread(1), Item::Thread(2), Item::Gap(4)];
        // a shelf shut drops its rows, and nothing else moves
        let b = [Item::Gap(1), Item::Gap(4)];
        assert_eq!(changed(&a, &b), (1..3, 0));
        assert_eq!(changed(&b, &a), (1..1, 2));
        assert_eq!(changed(&a, &a), (4..4, 0));
        let c = [Item::Gap(1), Item::Thread(9), Item::Thread(2), Item::Gap(4)];
        assert_eq!(changed(&a, &c), (1..2, 1));
        assert_eq!(changed(&[], &a), (0..0, 4));
        // a shelf opening keeps its heading in place
        let shut = [
            Item::Settled {
                count: 2,
                open: false,
            },
            Item::Gap(4),
        ];
        let open = [
            Item::Settled {
                count: 2,
                open: true,
            },
            Item::Shelved(7),
            Item::Shelved(8),
            Item::Gap(4),
        ];
        assert_eq!(changed(&shut, &open), (1..1, 2));
    }

    #[test]
    fn only_threads_new_to_the_list_arrive() {
        let old = [Item::Gap(2), Item::Thread(1), Item::Thread(2), Item::Gap(4)];
        let new = [
            Item::Gap(2),
            Item::Thread(3),
            Item::Thread(1),
            Item::Thread(2),
            Item::Settled {
                count: 1,
                open: true,
            },
            Item::Shelved(4),
            Item::Gap(4),
        ];
        assert_eq!(arrived(&old, &new), [3, 4]);
        // a thread that moves to a shelf joins it there
        let settled = [
            Item::Gap(2),
            Item::Thread(1),
            Item::Shelved(2),
            Item::Gap(4),
        ];
        assert_eq!(arrived(&old, &settled), [2]);
        assert!(arrived(&old, &old).is_empty());
    }

    #[test]
    fn a_dragged_row_near_an_edge_scrolls_the_list() {
        // nothing in the middle, faster nearer each edge, the right way
        assert_eq!(edge_scroll(300., 0., 600.), 0.);
        assert!(edge_scroll(10., 0., 600.) < edge_scroll(40., 0., 600.));
        assert!(edge_scroll(40., 0., 600.) < 0.);
        assert!(edge_scroll(590., 0., 600.) > edge_scroll(560., 0., 600.));
        assert!(edge_scroll(560., 0., 600.) > 0.);
        // past the edge is full speed, not more
        assert_eq!(edge_scroll(-20., 0., 600.), -EDGE_SPEED);
    }
}
