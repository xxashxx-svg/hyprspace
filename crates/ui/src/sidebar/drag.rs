// Dragging in the sidebar. A thread row dropped on another row of its space moves there, and
// takes that row's side of Settled; dropped on its space's header it comes back to the active
// list; dropped on a Settled row it settles. A pane's header drags the same way, and a row
// dropped on a pane opens there (panes/grid.rs). A space's header dropped on another moves the
// space above it. Folders dropped from File Explorer or Finder open as spaces.

use gpui::{
    Context, ExternalPaths, IntoElement, Pixels, Point, Render, SharedString, Window, div,
    prelude::*, px,
};

use crate::assets::icon;
use crate::colors;
use crate::panes::PaneDrag;
use crate::root::Root;
use crate::time::now_ms;

/// A space picked up by its header.
pub struct SpaceDrag {
    pub id: u64,
    pub name: SharedString,
}

/// The chip that follows the cursor while a space is dragged.
pub struct SpaceGhost {
    name: SharedString,
    grab: Point<Pixels>,
}

impl SpaceDrag {
    pub fn ghost(&self, grab: Point<Pixels>) -> SpaceGhost {
        SpaceGhost {
            name: self.name.clone(),
            grab,
        }
    }
}

impl Render for SpaceGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .pl((self.grab.x - px(16.)).max(px(0.)))
            .pt((self.grab.y - px(14.)).max(px(0.)))
            .child(
                div()
                    .w(px(200.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border2())
                    .bg(colors::surface3())
                    .shadow(colors::shadow())
                    .text_size(px(12.5))
                    .text_color(colors::text1())
                    .child(icon("folder", 12., colors::text2()))
                    .child(div().truncate().child(self.name.clone())),
            )
    }
}

impl Root {
    fn space_of(&self, thread: u64) -> Option<u64> {
        self.state.thread(thread).map(|(s, _)| s.id)
    }

    /// A thread dropped on another row of its space: it moves there, settled or not like that row.
    pub(crate) fn drop_on_thread(&mut self, d: &PaneDrag, target: u64, cx: &mut Context<Self>) {
        let Some(id) = d.pane.thread() else {
            return;
        };
        let space = self.space_of(target);
        if id == target || space.is_none() || self.space_of(id) != space {
            return;
        }
        let settled = self.state.thread(target).is_some_and(|(_, t)| t.settled);
        let Some(s) = space.and_then(|s| self.state.space_mut(s)) else {
            return;
        };
        let Some(from) = s.threads.iter().position(|t| t.id == id) else {
            return;
        };
        let mut t = s.threads.remove(from);
        if t.settled != settled || t.snooze.is_some() {
            t.settled = settled;
            t.snooze = None;
            t.touched = now_ms();
        }
        let to = s.threads.iter().position(|x| x.id == target).unwrap_or(0);
        s.threads.insert(to, t);
        if settled {
            self.free(id);
        }
        self.save();
        cx.notify();
    }

    /// A thread dropped on its space's header comes back to the active list.
    pub(crate) fn drop_on_space(&mut self, d: &PaneDrag, space: u64, cx: &mut Context<Self>) {
        let Some(id) = d.pane.thread() else {
            return;
        };
        if self.space_of(id) != Some(space) {
            return;
        }
        if let Some(t) = self.state.thread_mut(id)
            && !t.active()
        {
            t.settled = false;
            t.snooze = None;
            t.touched = now_ms();
            self.save();
            cx.notify();
        }
    }

    /// A thread dropped on its space's Settled row settles.
    pub(crate) fn drop_on_settled(
        &mut self,
        d: &PaneDrag,
        space: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = d.pane.thread()
            && self.space_of(id) == Some(space)
            && self.state.thread(id).is_some_and(|(_, t)| !t.settled)
        {
            self.settle(id, true, window, cx);
        }
    }

    /// A space dropped on another's header moves above it.
    pub(crate) fn drop_space(&mut self, d: &SpaceDrag, before: u64, cx: &mut Context<Self>) {
        if d.id == before {
            return;
        }
        let spaces = &mut self.state.spaces;
        let Some(from) = spaces.iter().position(|s| s.id == d.id) else {
            return;
        };
        let space = spaces.remove(from);
        let to = spaces
            .iter()
            .position(|s| s.id == before)
            .unwrap_or(spaces.len());
        spaces.insert(to, space);
        self.save();
        cx.notify();
    }

    /// Folders dropped from the system open as spaces; the last one's composer shows.
    pub(crate) fn drop_folders(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut last = None;
        for p in paths.paths().iter().filter(|p| p.is_dir()) {
            last = Some(self.add_project(p.clone(), cx));
        }
        if let Some(space) = last {
            self.save();
            self.compose(Some(space), window, cx);
        }
    }
}
