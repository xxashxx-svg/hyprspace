// The main area: the thread on screen, or the space's composer, with the dock on the right. The bar
// above is drawn in the title row. A structured thread runs edge to edge; a terminal sits in an
// 8px margin under its header, as the Tauri app padded its panes.

use gpui::{
    AnyElement, Context, DragMoveEvent, IntoElement, StyleRefinement, Window, div, prelude::*, px,
};

use crate::colors;
use crate::root::{Root, View};
use crate::slide::slide;

/// Space around a terminal.
const GAP: f32 = 8.;
const DOCK_MIN: f32 = 240.;
const DOCK_MAX: f32 = 720.;

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
    /// What the main area shows for a thread: the thread, the bar and the dock.
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
        self.sync_dock(space, cx);
        // a thread restored from the last run starts its session when it is first shown
        if !self.views.contains_key(&thread)
            && let Some((_, t)) = self.state.thread(thread)
        {
            let t = t.clone();
            self.make_view(&t, None, true, cx);
        }
        let body = match self.views.get(&thread) {
            Some(View::Structured(v)) => v.clone().into_any_element(),
            Some(View::Terminal(v)) => {
                // laid out again only when it changes: output, input, a resize
                let terminal = v.clone().cached(StyleRefinement::default().size_full());
                let header = self.pane_header(space, thread, cx);
                div()
                    .size_full()
                    .p(px(GAP))
                    .child(
                        div().size_full().flex().flex_col().child(header).child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .border_1()
                                .border_color(colors::border1())
                                .overflow_hidden()
                                .child(terminal),
                        ),
                    )
                    .into_any_element()
            }
            None => div().into_any_element(),
        };
        self.frame(body, window, cx)
    }

    /// The composer for a space, beside the same dock as its threads.
    pub(crate) fn compose_screen(
        &mut self,
        space: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // settles down into place as it fades in, each time a space's composer comes up
        let composer = crate::slide::ease_in(
            div().relative().size_full().child(self.composer.clone()),
            ("compose-in", space.unwrap_or(0) as usize),
            300,
            |d, t| d.opacity(t).top(px(-10. * (1. - t))),
        );
        match space.filter(|s| self.state.space(*s).is_some()) {
            Some(space) => {
                self.sync_dock(space, cx);
                self.frame(composer, window, cx)
            }
            None => composer,
        }
    }

    /// `body` with the dock on its right. The bar above is drawn in the title row.
    fn frame(
        &mut self,
        body: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // once it has slid away the dock's column stays, zero wide
        let open = self.state.dock.open;
        let flips = self.work.dock_flips.see(open);
        let dock = (open || flips > 0).then(|| {
            let width = self.state.dock.width;
            slide("dock", flips, open, (0., width), false, self.dock_column())
        });
        let popup = self.bar_popup(window, cx);
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
}
