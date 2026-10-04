// Dropping things on the sidebar. A thread row drags like a pane does: dropped on a pane it
// opens there (panes/grid.rs), dropped on the Settled shelf it settles. Folders dropped from File
// Explorer or Finder open as spaces.

use gpui::{Context, ExternalPaths, Window};

use crate::panes::PaneDrag;
use crate::root::Root;

impl Root {
    /// A thread dropped on the Settled shelf settles.
    pub(crate) fn drop_on_settled(
        &mut self,
        d: &PaneDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = d.pane.thread()
            && self.state.thread(id).is_some_and(|(_, t)| !t.settled)
        {
            self.settle(id, true, window, cx);
        }
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
