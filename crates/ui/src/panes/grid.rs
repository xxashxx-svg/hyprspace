// The workbench: the bar on top, the space's panes tiled below it, the dock on the right. Panes
// sit on fractions of the frame so dragged track sizes need no measuring; each pane pads its
// inner edges by half the 8px gap, as the Tauri app's grid gapped and padded by 8px.

use gpui::{
    AnyElement, Context, DragMoveEvent, IntoElement, MouseButton, Pixels, Point, StyleRefinement,
    Window, div, prelude::*, px, relative,
};
use hyprspace_proto::Pane;
use hyprspace_proto::grid::Tracks;

use super::header::PaneDrag;
use super::layout::{self, Axis, Layout};
use crate::colors;
use crate::root::{Root, View};
use crate::slide::slide;

/// Space around the panes and between them.
const GAP: f32 = 8.;
const DOCK_MIN: f32 = 240.;
const DOCK_MAX: f32 = 720.;

/// A boundary between tracks, carried while it is dragged.
pub struct GutterDrag {
    space: u64,
    axis: Axis,
    b: usize,
}

/// The dock's left edge, carried while it is dragged.
pub struct DockDrag;

/// A clear view for drags that need nothing under the cursor.
struct Nothing;

impl Render for Nothing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Root {
    /// What the main area shows for a thread: its space's panes, the bar and the dock.
    pub(crate) fn workbench(
        &mut self,
        thread: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(space) = self.state.thread(thread).map(|(s, _)| s.id) else {
            return div().into_any_element();
        };
        if let Some((path, line, col)) = self.work.pending.take() {
            self.open_path(path, line, col, window, cx);
        }
        // a thread on screen is always one of the panes, whatever the saved grid says
        if !self
            .live_panes(space)
            .contains(&Pane::Thread { id: thread })
        {
            self.place_thread(thread, false);
        }
        self.sync_dock(space, cx);
        let panes = self.live_panes(space);
        // threads restored from the last run start their sessions once their space is shown
        for id in panes.iter().filter_map(Pane::thread) {
            if !self.views.contains_key(&id)
                && let Some((_, t)) = self.state.thread(id)
            {
                let t = t.clone();
                self.make_view(&t, None, true, cx);
            }
        }
        let grid = self.grid(space, &panes, window, cx);
        self.frame(space, panes.len(), grid, window, cx)
    }

    /// The composer for a space, beside the same dock as its panes.
    pub(crate) fn compose_screen(
        &mut self,
        space: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let composer = self.composer.clone().into_any_element();
        match space.filter(|s| self.state.space(*s).is_some()) {
            Some(space) => {
                self.sync_dock(space, cx);
                self.frame(space, 0, composer, window, cx)
            }
            None => composer,
        }
    }

    /// `body` with the dock on its right. The bar above is drawn in the title row.
    fn frame(
        &mut self,
        space: u64,
        panes: usize,
        body: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // once it has slid away the dock's column stays, zero wide
        let open = self.state.dock.open;
        let flips = self.work.dock_flips.see(open);
        let dock = (open || flips > 0).then(|| {
            let width = self.state.dock.width;
            slide("dock", flips, open, (0., width), true, self.dock_column())
        });
        let popup = self.bar_popup(space, panes, window, cx);
        div()
            .id("workbench")
            .size_full()
            .flex()
            .on_action(cx.listener(Self::toggle_dock))
            .on_drag_move(cx.listener(|r, e: &DragMoveEvent<DockDrag>, _, cx| {
                let x: f32 = e.event.position.x.into();
                let right: f32 = e.bounds.right().into();
                r.state.dock.width = (right - x).clamp(DOCK_MIN, DOCK_MAX);
                cx.notify();
            }))
            .on_drop(cx.listener(|r, _: &DockDrag, _, _| r.save()))
            .on_drop(cx.listener(|r, _: &GutterDrag, _, _| r.save()))
            .child(div().flex_1().min_w_0().h_full().child(body))
            .children(dock)
            .children(popup)
            .into_any_element()
    }

    fn dock_column(&self) -> AnyElement {
        div()
            .relative()
            .flex_none()
            .w(px(self.state.dock.width))
            .h_full()
            .border_l_1()
            .border_color(colors::border0())
            .bg(colors::bg())
            .child(
                div()
                    .id("dock-edge")
                    .absolute()
                    .left(px(-3.))
                    .top_0()
                    .bottom_0()
                    .w(px(6.))
                    .cursor_col_resize()
                    .hover(|s| s.bg(colors::accent().opacity(0.45)))
                    .on_drag(DockDrag, |_, _, _, cx| cx.new(|_| Nothing)),
            )
            .child(self.work.dock.clone())
            .into_any_element()
    }

    fn grid(
        &mut self,
        space: u64,
        panes: &[Pane],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // a structured thread alone fills the main area edge to edge, with no frame or header
        if let Some(id) = self.lone_structured(space)
            && let Some(View::Structured(v)) = self.views.get(&id)
        {
            return v.clone().into_any_element();
        }
        let Some(grid) = self.state.space(space).map(|s| s.grid.clone()) else {
            return div().into_any_element();
        };
        let n = panes.len();
        let maxed = grid.maximized.clone().filter(|m| panes.contains(m));
        let layout = layout::resolve(n, grid.layouts.get(&n).map(String::as_str));
        let saved = grid
            .tracks
            .get(&layout::key(n, &layout))
            .cloned()
            .unwrap_or_default();
        let cols = layout.weights(Axis::Col, &saved.cols);
        let rows = layout.weights(Axis::Row, &saved.rows);
        let focus = grid.focus.clone();

        let mut cells: Vec<AnyElement> = Vec::new();
        for (i, pane) in panes.iter().enumerate() {
            let rect = match &maxed {
                Some(m) if m == pane => (0., 0., 1., 1.),
                Some(_) => continue,
                None => {
                    let Some(cell) = layout.cells.get(i) else {
                        continue;
                    };
                    (
                        layout::edge(&cols, cell.cols.start),
                        layout::edge(&rows, cell.rows.start),
                        layout::edge(&cols, cell.cols.end),
                        layout::edge(&rows, cell.rows.end),
                    )
                }
            };
            let focused = focus.as_ref() == Some(pane);
            // the focus ring only tells panes apart, so a pane alone goes without it
            let ring = focused && n > 1;
            cells.push(self.cell(
                space,
                i,
                pane,
                rect,
                (focused, ring),
                maxed.is_some(),
                window,
                cx,
            ));
        }
        let gutters = if maxed.is_some() || n < 2 {
            vec![]
        } else {
            gutters(space, &layout, &cols, &rows)
        };
        let pad = if maxed.is_some() { 0. } else { GAP };
        div()
            .id("grid")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .on_drag_move(cx.listener(move |r, e: &DragMoveEvent<GutterDrag>, _, cx| {
                let d = e.drag(cx);
                let (space, axis, b) = (d.space, d.axis, d.b);
                r.drag_gutter(space, axis, b, e.event.position, e.bounds, cx);
            }))
            .child(
                div()
                    .absolute()
                    .top(px(pad))
                    .left(px(pad))
                    .right(px(pad))
                    .bottom(px(pad))
                    .children(cells)
                    .children(gutters),
            )
            .into_any_element()
    }

    /// Moves boundary `b` to the cursor and keeps the new sizes for this layout.
    fn drag_gutter(
        &mut self,
        space: u64,
        axis: Axis,
        b: usize,
        at: Point<Pixels>,
        bounds: gpui::Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let n = self.live_panes(space).len();
        let Some(s) = self.state.space_mut(space) else {
            return;
        };
        let layout = layout::resolve(n, s.grid.layouts.get(&n).map(String::as_str));
        let key = layout::key(n, &layout);
        let saved = s.grid.tracks.get(&key).cloned().unwrap_or_default();
        let (pos, start, size): (f32, f32, f32) = match axis {
            Axis::Col => (at.x.into(), bounds.left().into(), bounds.size.width.into()),
            Axis::Row => (at.y.into(), bounds.top().into(), bounds.size.height.into()),
        };
        let inner = (size - 2. * GAP).max(1.);
        let want = ((pos - start - GAP) / inner).clamp(0., 1.);
        let w = layout.weights(
            axis,
            match axis {
                Axis::Col => &saved.cols,
                Axis::Row => &saved.rows,
            },
        );
        let next = layout::drag(&w, b, want - layout::edge(&w, b));
        let tracks = s.grid.tracks.entry(key).or_insert_with(Tracks::default);
        match axis {
            Axis::Col => tracks.cols = next,
            Axis::Row => tracks.rows = next,
        }
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn cell(
        &mut self,
        space: u64,
        ix: usize,
        pane: &Pane,
        (x0, y0, x1, y1): (f32, f32, f32, f32),
        (focused, ring): (bool, bool),
        maxed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let half = GAP / 2.;
        let edge = |at_edge: bool| if at_edge { 0. } else { half };
        let body: AnyElement = match pane {
            Pane::Thread { id } => match self.views.get(id) {
                Some(View::Structured(v)) => v.clone().into_any_element(),
                // laid out again only when it changes: output, input, a resize. Redoing every
                // terminal on every frame is what made scrolling anywhere slow
                Some(View::Terminal(v)) => v
                    .clone()
                    .cached(StyleRefinement::default().size_full())
                    .into_any_element(),
                None => div().into_any_element(),
            },
            _ => {
                let v = self.viewer(space, cx);
                if v.read(cx).pane() != Some(pane) {
                    v.update(cx, |v, cx| v.show(pane.clone(), cx));
                }
                v.into_any_element()
            }
        };
        let header = self.pane_header(space, pane, focused, maxed, window, cx);
        let target = pane.clone();
        let clicked = pane.clone();
        div()
            .id(("cell", ix))
            .absolute()
            .left(relative(x0))
            .top(relative(y0))
            .w(relative(x1 - x0))
            .h(relative(y1 - y0))
            .pl(px(edge(x0 <= 0.)))
            .pr(px(edge(x1 >= 1.)))
            .pt(px(edge(y0 <= 0.)))
            .pb(px(edge(y1 >= 1.)))
            .capture_any_mouse_down(cx.listener(move |r, _, window, cx| {
                r.focus_pane(space, clicked.clone(), false, window, cx)
            }))
            .on_drop(cx.listener(move |r, d: &PaneDrag, _, cx| {
                if d.space == space && d.pane != target {
                    r.swap_panes(space, &d.pane, &target, cx);
                }
            }))
            .child(
                div().size_full().flex().flex_col().child(header).child(
                    div()
                        .id(("pane-body", ix))
                        .flex_1()
                        .min_h_0()
                        .border_1()
                        .border_color(if ring && !maxed {
                            colors::accent()
                        } else if maxed {
                            colors::border1().opacity(0.)
                        } else {
                            colors::border1()
                        })
                        .drag_over::<PaneDrag>(|s, _, _, _| {
                            s.border_color(colors::accent())
                                .bg(colors::accent().opacity(0.07))
                        })
                        .overflow_hidden()
                        .child(body),
                ),
            )
            .into_any_element()
    }
}

/// The drag handles over the gaps between tracks: invisible until hovered, then a 2px line.
fn gutters(space: u64, layout: &Layout, cols: &[f32], rows: &[f32]) -> Vec<AnyElement> {
    let mut out = Vec::new();
    for axis in [Axis::Col, Axis::Row] {
        let w = if axis == Axis::Col { cols } else { rows };
        for b in layout.resizable(axis) {
            let at = layout::edge(w, b);
            let handle = div()
                .id((
                    if axis == Axis::Col {
                        "gutter-col"
                    } else {
                        "gutter-row"
                    },
                    b,
                ))
                .absolute()
                .group("gutter")
                .flex()
                .justify_center()
                .items_center()
                .on_drag(GutterDrag { space, axis, b }, |_, _, _, cx| {
                    cx.new(|_| Nothing)
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            let line = div().bg(colors::accent().opacity(0.)).rounded(px(2.));
            let el = match axis {
                Axis::Col => handle
                    .top_0()
                    .bottom_0()
                    .left(relative(at))
                    .ml(px(-GAP / 2.))
                    .w(px(GAP))
                    .cursor_col_resize()
                    .child(
                        line.w(px(2.))
                            .h_full()
                            .group_hover("gutter", |s| s.bg(colors::accent().opacity(0.6))),
                    ),
                Axis::Row => handle
                    .left_0()
                    .right_0()
                    .top(relative(at))
                    .mt(px(-GAP / 2.))
                    .h(px(GAP))
                    .cursor_row_resize()
                    .child(
                        line.h(px(2.))
                            .w_full()
                            .group_hover("gutter", |s| s.bg(colors::accent().opacity(0.6))),
                    ),
            };
            out.push(el.into_any_element());
        }
    }
    out
}
