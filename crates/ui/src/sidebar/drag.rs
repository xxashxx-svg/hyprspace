// Dropping things on the sidebar. A thread row dropped on another row takes that row's place, and
// dropped on the Settled shelf it settles. Folders dropped from File Explorer or Finder open as
// spaces.

use gpui::{Context, ExternalPaths, Window};

use crate::root::Root;
use crate::workbench::PaneDrag;

/// How far apart neighbouring rows' places are after a move, so later moves have room between.
const STEP: i64 = 1000;

/// The places for the list after `id` is dropped on `target`, just above it or just `below`, given
/// each row's place, top first. The top row keeps its place, so a thread made later still goes
/// above them all.
fn reorder(list: &[(u64, i64)], id: u64, target: u64, below: bool) -> Option<Vec<(u64, i64)>> {
    if id == target {
        return None;
    }
    list.iter().position(|(t, _)| *t == id)?;
    let top = list.first()?.1;
    let mut ids: Vec<u64> = list.iter().map(|(t, _)| *t).filter(|t| *t != id).collect();
    let at = ids.iter().position(|t| *t == target)? + usize::from(below);
    ids.insert(at, id);
    Some(
        ids.into_iter()
            .enumerate()
            .map(|(i, t)| (t, top - i as i64 * STEP))
            .collect(),
    )
}

impl Root {
    /// A thread dropped on another row of the list moves to the line shown: above that row, or
    /// below it. Every row at work gets its place written down, so they all keep the order shown.
    pub(crate) fn drop_on_thread(
        &mut self,
        d: &PaneDrag,
        target: u64,
        below: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = d.pane.thread() else {
            return;
        };
        let mut list: Vec<(u64, i64)> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .flat_map(|s| s.threads.iter().filter(|t| t.active()))
            .map(|t| (t.id, t.rank()))
            .collect();
        list.sort_by_key(|(_, rank)| std::cmp::Reverse(*rank));
        let Some(places) = reorder(&list, id, target, below) else {
            return;
        };
        for (t, place) in places {
            if let Some(thread) = self.state.thread_mut(t) {
                thread.order = Some(place);
            }
        }
        self.save();
        cx.notify();
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(places: &[(u64, i64)]) -> Vec<u64> {
        places.iter().map(|(t, _)| *t).collect()
    }

    #[test]
    fn a_row_lands_on_the_line_shown() {
        let list = [(1, 9000), (2, 8000), (3, 7000), (4, 6000)];
        assert_eq!(ids(&reorder(&list, 4, 2, false).unwrap()), [1, 4, 2, 3]);
        assert_eq!(ids(&reorder(&list, 4, 2, true).unwrap()), [1, 2, 4, 3]);
        // to the very bottom, and to the very top
        assert_eq!(ids(&reorder(&list, 1, 4, true).unwrap()), [2, 3, 4, 1]);
        assert_eq!(ids(&reorder(&list, 3, 1, false).unwrap()), [3, 1, 2, 4]);
        assert!(reorder(&list, 2, 2, true).is_none());
        // places count down from the top one's, so newer threads still land above
        let places = reorder(&list, 4, 1, false).unwrap();
        assert_eq!(places[0], (4, 9000));
        assert!(places.windows(2).all(|w| w[0].1 > w[1].1));
    }
}
