// The sidebar, after the Tauri app's Rail (src/components/Rail.tsx at v0.21.1): a search box and
// a New thread button on top, every space as a section that folds open to its threads, an
// Archived group under a divider, and Settings at the foot. Clicking a space opens it and its
// arrow folds it. Right-click a space or a thread for its menu; drag the right edge to resize.
// The drag handle follows zeron's shell (MIT, see THIRD_PARTY_NOTICES.md).
//
// It is a view of its own, cached, so a frame that only changes a terminal reuses its layout. Its
// rows are a virtual list: only the ones on screen are laid out, which is what keeps scrolling
// smooth with dozens of spaces. The wheel eases the list along instead of jumping a notch at a
// time, the way the Tauri app's webview scrolled.

mod drag;
mod row;

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, DispatchPhase, Entity, ExternalPaths, Focusable, FontWeight,
    HitboxBehavior, IntoElement, ListAlignment, ListState, MouseButton, MouseDownEvent,
    ScrollWheelEvent, SharedString, Subscription, Transformation, WeakEntity, Window, canvas, div,
    list, percentage, prelude::*, px,
};
use hyprspace_proto::{Pane, Space, Thread};
use hyprspace_theme::MONO;

use crate::assets::icon;
use crate::colors;
use crate::palette::TogglePalette;
use crate::panes::PaneDrag;
use crate::root::{Action, MenuItems, Rename, Root, Screen, SidebarDrag};
use crate::time::now_ms;
use drag::SpaceDrag;

pub const MIN_WIDTH: f32 = 200.;
pub const MAX_WIDTH: f32 = 480.;
/// Rows laid out past each edge of the list, so a quick scroll doesn't show them arriving.
const OVERDRAW: f32 = 240.;
/// Rows sit this far in from the sidebar's edges, and a space's threads this much further.
const EDGE: f32 = 8.;
const INDENT: f32 = 12.;
/// How fast an eased scroll settles: each frame covers this share of what is left per second's
/// worth of time constant. About 60 ms, the feel of a browser's smooth scrolling.
const EASE: f32 = 0.06;
/// A wheel line's worth of travel.
const LINE: f32 = 20.;

/// One line of the sidebar's list.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// A space's header, the active one drawn stronger.
    Space {
        id: u64,
        open: bool,
        active: bool,
    },
    /// An open space's working tree line.
    Summary(u64),
    /// A thread, under its space or under Archived.
    Thread(u64),
    /// An open space with no threads.
    Empty(u64),
    /// Room after an open section, or at the end.
    Gap(u8),
    /// A space's settled threads, folded under one row.
    Settled {
        space: u64,
        count: usize,
        open: bool,
    },
    /// The snoozed threads of every space, on a shelf near the bottom.
    Snoozed {
        count: usize,
        open: bool,
    },
    /// The Archived heading and how many sit under it.
    Archived {
        count: usize,
        open: bool,
    },
    ArchivedSpace(u64),
    /// Nothing to list: no threads yet, or a search with no hits.
    Nothing {
        searching: bool,
    },
}

impl Item {
    /// The same row, whatever it shows: a header that folds or becomes active stays where it is,
    /// and only the rows that come or go are spliced.
    fn same(&self, other: &Item) -> bool {
        match (self, other) {
            (Item::Space { id: a, .. }, Item::Space { id: b, .. }) => a == b,
            (Item::Archived { .. }, Item::Archived { .. }) => true,
            (Item::Snoozed { .. }, Item::Snoozed { .. }) => true,
            (Item::Settled { space: a, .. }, Item::Settled { space: b, .. }) => a == b,
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
        let items = root.update(cx, |r, cx| r.sidebar_items(cx));
        if items != *self.items {
            let (range, count) = changed(&self.items, &items);
            if !range.is_empty() || count > 0 {
                self.list.splice(range, count);
            }
            self.items = Rc::new(items);
        }
        // a status, a line of activity or a subagent can change a row's height
        if std::mem::take(&mut self.stale) {
            self.list.remeasure();
        }
        self.ease(window);
        let wheel = self.wheel(cx);
        let (items, weak, now) = (self.items.clone(), self.root.clone(), now_ms());
        let rows = list(self.list.clone(), move |ix, _, cx| {
            let item = items.get(ix).cloned();
            weak.update(cx, |r, cx| match item {
                Some(item) => r.sidebar_item(&item, now, cx),
                None => div().into_any_element(),
            })
            .unwrap_or_else(|_| div().into_any_element())
        })
        .size_full();
        let rows = div()
            .relative()
            .size_full()
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

/// The arrow that folds a section, turned down while it is open.
fn twist(open: bool) -> impl IntoElement {
    icon("chevron-right", 12., colors::text3()).with_transformation(Transformation::rotate(
        percentage(if open { 0.25 } else { 0. }),
    ))
}

/// A 22px button in a space's header that only shows on hover, or always on the active space.
fn header_button(
    id: (&'static str, u64),
    glyph: &str,
    shown: bool,
    group: SharedString,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface3()))
        .when(!shown, |d| {
            d.opacity(0.).group_hover(group, |s| s.opacity(1.))
        })
        .child(icon(glyph, 13., colors::text3()))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// A row's place in the list: in from the edges, threads further under their space, and a little
/// air above.
fn slot(indent: bool) -> gpui::Div {
    div()
        .pl(px(if indent { EDGE + INDENT } else { EDGE }))
        .pr(px(EDGE))
        .pt(px(2.))
}

impl Root {
    /// The sidebar's rows, top to bottom: each space, and while it is open its summary and
    /// threads; then Archived. A search keeps only what matches and opens every space with a hit.
    fn sidebar_items(&self, cx: &mut Context<Self>) -> Vec<Item> {
        let q = self.search.read(cx).text().trim().to_lowercase();
        let active = self.current_space();
        let mut items = Vec::new();
        let mut shown = 0;
        for space in self.state.spaces.iter().filter(|s| !s.archived) {
            let hit = |t: &&Thread| q.is_empty() || t.title.to_lowercase().contains(&q);
            let threads: Vec<&Thread> = space
                .threads
                .iter()
                .filter(|t| t.active())
                .filter(hit)
                .collect();
            let settled: Vec<&Thread> = space
                .threads
                .iter()
                .filter(|t| t.settled)
                .filter(hit)
                .collect();
            if !q.is_empty()
                && threads.is_empty()
                && settled.is_empty()
                && !space.name.to_lowercase().contains(&q)
            {
                continue;
            }
            shown += 1;
            let open = if q.is_empty() {
                !space.folded
            } else {
                !threads.is_empty() || !settled.is_empty()
            };
            items.push(Item::Space {
                id: space.id,
                open,
                active: active == Some(space.id),
            });
            if open {
                if self.summary(space).is_some() {
                    items.push(Item::Summary(space.id));
                }
                items.extend(threads.iter().map(|t| Item::Thread(t.id)));
                if threads.is_empty() && settled.is_empty() {
                    items.push(Item::Empty(space.id));
                }
                if !settled.is_empty() {
                    // a search shows settled hits without asking
                    let open = !q.is_empty() || self.settled_open.contains(&space.id);
                    items.push(Item::Settled {
                        space: space.id,
                        count: settled.len(),
                        open,
                    });
                    if open {
                        items.extend(settled.iter().map(|t| Item::Thread(t.id)));
                    }
                }
                items.push(Item::Gap(6));
            }
        }
        if shown == 0 {
            items.push(Item::Nothing {
                searching: !q.is_empty(),
            });
        }
        let snoozed: Vec<u64> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .flat_map(|s| s.threads.iter())
            .filter(|t| t.snooze.is_some())
            .filter(|t| q.is_empty() || t.title.to_lowercase().contains(&q))
            .map(|t| t.id)
            .collect();
        if !snoozed.is_empty() {
            let open = self.snoozed_open || !q.is_empty();
            items.push(Item::Snoozed {
                count: snoozed.len(),
                open,
            });
            if open {
                items.extend(snoozed.into_iter().map(Item::Thread));
            }
        }
        let spaces: Vec<&Space> = self.state.spaces.iter().filter(|s| s.archived).collect();
        if !spaces.is_empty() {
            let open = self.archived_open;
            items.push(Item::Archived {
                count: spaces.len(),
                open,
            });
            if open {
                items.extend(spaces.iter().map(|s| Item::ArchivedSpace(s.id)));
            }
        }
        items.push(Item::Gap(4));
        items
    }

    fn sidebar_item(&self, item: &Item, now: u64, cx: &mut Context<Self>) -> AnyElement {
        match *item {
            Item::Space { id, open, active } => match self.state.space(id) {
                Some(space) => slot(false)
                    .child(self.space_header(space, open, active, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Item::Summary(id) => slot(true)
                .children(self.state.space(id).and_then(|s| self.summary(s)))
                .into_any_element(),
            Item::Thread(id) => match self.state.thread(id) {
                Some((_, t)) => slot(true)
                    .child(self.thread_row(t, now, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Item::Empty(_) => slot(true)
                .child(
                    div()
                        .px(px(8.))
                        .pt(px(2.))
                        .pb(px(4.))
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("No threads yet"),
                )
                .into_any_element(),
            Item::Gap(h) => div().h(px(h as f32)).into_any_element(),
            Item::Settled { space, count, open } => slot(true)
                .child(self.settled_row(space, count, open, cx))
                .into_any_element(),
            Item::Snoozed { count, open } => slot(false)
                .pt(px(8.))
                .child(
                    div()
                        .pt(px(6.))
                        .border_t_1()
                        .border_color(colors::border1())
                        .child(self.shelf_header(
                            "snoozed",
                            "clock",
                            "Snoozed",
                            count,
                            open,
                            cx.listener(|r, _: &ClickEvent, _, cx| {
                                r.snoozed_open = !r.snoozed_open;
                                cx.notify();
                            }),
                        )),
                )
                .into_any_element(),
            Item::Archived { count, open } => slot(false)
                .pt(px(8.))
                .child(
                    div()
                        .pt(px(6.))
                        .border_t_1()
                        .border_color(colors::border1())
                        .child(self.archived_header(count, open, cx)),
                )
                .into_any_element(),
            Item::ArchivedSpace(id) => match self.state.space(id) {
                Some(space) => slot(true)
                    .child(self.archived_space(space, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
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

    /// A space's header: the fold arrow, the name, and New thread and Archive on hover.
    fn space_header(
        &self,
        space: &Space,
        open: bool,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = space.id;
        let group: SharedString = format!("space-{id}").into();
        let name: AnyElement = match &self.rename {
            Some((Rename::Space(r), input, _)) if *r == id => div()
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::accent())
                .bg(colors::bg())
                .text_color(colors::text1())
                .child(input.clone())
                .into_any_element(),
            _ => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(space.name.clone())
                .into_any_element(),
        };
        let mut menu: MenuItems = vec![
            ("New thread".into(), Action::NewThread(id)),
            ("New terminal".into(), Action::NewTerminal(id)),
        ];
        // the same apps, in the same order, as the Open button's menu
        if space.cwd.is_some() {
            menu.extend(self.work.openers.iter().map(|&o| {
                (
                    format!("Open in {}", o.name()).into(),
                    Action::OpenIn(o, id),
                )
            }));
        }
        menu.extend([
            ("Rename".into(), Action::Rename(Rename::Space(id))),
            ("Archive".into(), Action::ArchiveSpace(id, true)),
            ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
        ]);
        div()
            .id(("space", id))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(30.))
            .pl(px(2.))
            .pr(px(4.))
            .rounded(px(7.))
            .text_size(px(13.))
            .font_weight(if active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(if active {
                colors::text1()
            } else {
                colors::text2()
            })
            .cursor_pointer()
            .hover(|s| s.bg(row_hover()).text_color(colors::text1()))
            .child(
                div()
                    .id(("space-fold", id))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(20.))
                    .rounded(px(5.))
                    .hover(|s| s.bg(colors::surface3()))
                    .child(twist(open))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.toggle_fold(id, cx))),
            )
            .child(name)
            .child(
                header_button(("space-new", id), "plus", active, group.clone()).on_click(
                    cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::NewThread(id), window, cx)
                    }),
                ),
            )
            .child(
                header_button(("space-archive", id), "archive", active, group).on_click(
                    cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::ArchiveSpace(id, true), window, cx)
                    }),
                ),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_space(id, window, cx)
            }))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .on_drag(
                SpaceDrag {
                    id,
                    name: space.name.clone().into(),
                },
                |d, offset, _, cx| cx.new(|_| d.ghost(offset)),
            )
            .drag_over::<SpaceDrag>(|s, _, _, _| s.bg(colors::accent().opacity(0.12)))
            .drag_over::<PaneDrag>(|s, _, _, _| s.bg(colors::accent().opacity(0.12)))
            .on_drop(cx.listener(move |r, d: &SpaceDrag, _, cx| r.drop_space(d, id, cx)))
            .on_drop(cx.listener(move |r, d: &PaneDrag, _, cx| r.drop_on_space(d, id, cx)))
            .into_any_element()
    }

    /// The working tree under an open space: file count and line deltas. A clean tree says
    /// nothing; the absence of the line is the message.
    fn summary(&self, space: &Space) -> Option<AnyElement> {
        let status = self.git.get(space.cwd.as_ref()?)?;
        if status.changes.is_empty() {
            return None;
        }
        let files = status.changes.len();
        let added: u32 = status.changes.iter().map(|c| c.added).sum();
        let removed: u32 = status.changes.iter().map(|c| c.removed).sum();
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .pb(px(2.))
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(colors::text3())
                .child(format!(
                    "{files} {}",
                    if files == 1 { "file" } else { "files" }
                ))
                .child(
                    div()
                        .text_color(colors::diff_add())
                        .child(format!("+{added}")),
                )
                .child(
                    div()
                        .text_color(colors::diff_del())
                        .child(format!("\u{2212}{removed}")),
                )
                .into_any_element(),
        )
    }

    /// Opens a space: the thread its grid shows first, or its composer when the grid is empty.
    pub(crate) fn open_space(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(id) {
            s.folded = false;
        }
        let first = self.live_panes(id).into_iter().find_map(|p| match p {
            Pane::Thread { id } => Some(id),
            _ => None,
        });
        match first {
            Some(thread) => self.open_thread(thread, window, cx),
            None => self.compose(Some(id), window, cx),
        }
        self.save();
        self.git_poll(std::time::Duration::MAX);
    }

    /// Opens `items` as a menu where the right-click was.
    pub(crate) fn context_menu(
        &self,
        items: MenuItems,
        cx: &mut Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        cx.listener(move |r, e: &MouseDownEvent, _, cx| {
            r.menu = Some((e.position, items.clone()));
            cx.stop_propagation();
            cx.notify();
        })
    }

    /// The Archived heading over archived spaces.
    fn archived_header(&self, count: usize, open: bool, cx: &mut Context<Self>) -> AnyElement {
        self.shelf_header(
            "archived",
            "archive",
            "Archived",
            count,
            open,
            cx.listener(|r, _: &ClickEvent, _, cx| {
                r.archived_open = !r.archived_open;
                cx.notify();
            }),
        )
    }

    /// A space's Settled row: folded, it is one quiet line with the count.
    fn settled_row(
        &self,
        space: u64,
        count: usize,
        open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(("settled", space))
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(26.))
            .pl(px(4.))
            .pr(px(8.))
            .rounded(px(7.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|s| s.bg(row_hover()).text_color(colors::text2()))
            .child(twist(open))
            .child(icon("circle-check", 12., colors::text3()))
            .child(div().flex_1().child("Settled"))
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                if !r.settled_open.remove(&space) {
                    r.settled_open.insert(space);
                }
                cx.notify();
            }))
            .drag_over::<PaneDrag>(|s, _, _, _| s.bg(colors::accent().opacity(0.12)))
            .on_drop(cx.listener(move |r, d: &PaneDrag, window, cx| {
                r.drop_on_settled(d, space, window, cx)
            }))
            .into_any_element()
    }

    /// The heading of a shelf near the bottom: Snoozed, Archived.
    #[allow(clippy::too_many_arguments)]
    fn shelf_header(
        &self,
        id: &'static str,
        glyph: &str,
        label: &'static str,
        count: usize,
        open: bool,
        toggle: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(30.))
            .pl(px(2.))
            .pr(px(4.))
            .rounded(px(7.))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|s| s.bg(row_hover()).text_color(colors::text1()))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(20.))
                    .child(twist(open)),
            )
            .child(div().mr(px(2.)).child(icon(glyph, 13., colors::text3())))
            .child(div().flex_1().child(label))
            .child(
                div()
                    .pr(px(4.))
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .child(count.to_string()),
            )
            .on_click(toggle)
            .into_any_element()
    }

    /// An archived space: its name and thread count, which give way to Restore on hover.
    fn archived_space(&self, s: &Space, cx: &mut Context<Self>) -> AnyElement {
        let id = s.id;
        let group: SharedString = format!("arch-{id}").into();
        let menu: MenuItems = vec![
            ("Restore".into(), Action::ArchiveSpace(id, false)),
            ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
        ];
        let n = s.threads.len();
        div()
            .id(("archived-space", id))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(30.))
            .pl(px(8.))
            .pr(px(6.))
            .rounded(px(7.))
            .text_size(px(13.))
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|d| d.bg(row_hover()).text_color(colors::text1()))
            .child(div().flex_1().min_w_0().truncate().child(s.name.clone()))
            .when(n > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .font_family(MONO)
                        .text_size(px(10.5))
                        .group_hover(group.clone(), |s| s.opacity(0.))
                        .child(format!("{n} {}", if n == 1 { "thread" } else { "threads" })),
                )
            })
            .child(
                div()
                    .id(("restore", id))
                    .absolute()
                    .right(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .h(px(20.))
                    .px(px(7.))
                    .rounded(px(5.))
                    .bg(colors::surface3())
                    .text_size(px(11.))
                    .text_color(colors::text1())
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .child(icon("archive-restore", 11., colors::text1()))
                    .child("Restore")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::ArchiveSpace(id, false), window, cx)
                    })),
            )
            .on_click(
                cx.listener(move |r, _: &ClickEvent, window, cx| r.open_space(id, window, cx)),
            )
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
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
        // a space folded shut drops its rows, and nothing else moves
        let b = [Item::Gap(1), Item::Gap(4)];
        assert_eq!(changed(&a, &b), (1..3, 0));
        assert_eq!(changed(&b, &a), (1..1, 2));
        assert_eq!(changed(&a, &a), (4..4, 0));
        let c = [Item::Gap(1), Item::Thread(9), Item::Thread(2), Item::Gap(4)];
        assert_eq!(changed(&a, &c), (1..2, 1));
        assert_eq!(changed(&[], &a), (0..0, 4));
        // a header turning active is the same row, so nothing is spliced
        let x = [
            Item::Space {
                id: 1,
                open: true,
                active: false,
            },
            Item::Thread(5),
        ];
        let y = [
            Item::Space {
                id: 1,
                open: true,
                active: true,
            },
            Item::Thread(5),
        ];
        assert_eq!(changed(&x, &y), (2..2, 0));
    }
}
