// Side chats (`/btw`): a thread forked from the one on screen, hidden from every list and shown as
// a tab in the dock. Bring back hands what it found to its thread's next message.

use gpui::Context;
use hyprspace_proto::state::{Thread, ThreadKind};
use hyprspace_proto::{Launch, Prompt};

use super::{Root, Screen, View};
use crate::dock::Side;
use crate::time::now_ms;
use crate::transcript::Status;

/// A side chat holds a whole thread, so the dock opens at least this wide for one.
const SIDE_WIDTH: f32 = 480.;
const TITLE: usize = 40;

/// The side chats on the dock, the one picked, and whether their thread is idle.
pub(crate) type SideSync = (Vec<(u64, String)>, Option<u64>, bool);

fn title_of(question: &str) -> String {
    let flat = question.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(TITLE) {
        Some((at, _)) => format!("{}…", flat[..at].trim_end()),
        None => flat,
    }
}

impl Root {
    /// `prompt` asked in a side chat of `from`, opened in the dock.
    pub(crate) fn open_side(
        &mut self,
        from: u64,
        launch: Launch,
        prompt: Prompt,
        cx: &mut Context<Self>,
    ) {
        let Some(space) = self.state.thread(from).map(|(s, _)| s.id) else {
            return;
        };
        let thread = Thread {
            id: self.state.take_id(),
            title: title_of(&prompt.text),
            kind: ThreadKind::Structured { launch },
            created: now_ms(),
            touched: now_ms(),
            parent: Some(from),
            side: true,
            ..Thread::default()
        };
        let Some(s) = self.state.space_mut(space) else {
            return;
        };
        s.threads.push(thread.clone());
        let text = prompt.text.clone();
        self.make_view(&thread, Some(prompt), false, cx);
        self.name_thread(thread.id, &text, None, cx);
        self.work.side = Some(thread.id);
        self.state.dock.open = true;
        self.state.dock.width = self.state.dock.width.max(SIDE_WIDTH);
        self.save();
        cx.notify();
    }

    /// Tells the dock which side chats the thread on screen has, when that changed.
    pub(crate) fn sync_sides(&mut self, cx: &mut Context<Self>) {
        let parent = match self.screen {
            Screen::Thread(id) => Some(id),
            _ => None,
        };
        let sides: Vec<Thread> = parent
            .map(|p| {
                self.state
                    .spaces
                    .iter()
                    .flat_map(|s| &s.threads)
                    .filter(|t| t.side && t.parent == Some(p))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        // made on first show: after a restart a side chat has a journal but no view
        for t in &sides {
            if !self.views.contains_key(&t.id) {
                self.make_view(t, None, true, cx);
            }
        }
        let side = self
            .work
            .side
            .filter(|id| sides.iter().any(|t| t.id == *id));
        let idle = parent
            .and_then(|p| match self.views.get(&p) {
                Some(View::Structured(v)) => Some(v.read(cx).status()),
                _ => None,
            })
            .is_none_or(|s| !matches!(s, Status::Working | Status::Waiting));
        let want: SideSync = (
            sides.iter().map(|t| (t.id, t.title.clone())).collect(),
            side,
            idle,
        );
        if self.work.side_sync.as_ref() == Some(&want) {
            return;
        }
        self.work.side_sync = Some(want);
        let tabs = sides
            .iter()
            .filter_map(|t| match self.views.get(&t.id) {
                Some(View::Structured(v)) => Some(Side {
                    id: t.id,
                    title: t.title.clone().into(),
                    view: v.clone().into(),
                }),
                _ => None,
            })
            .collect();
        self.work
            .dock
            .update(cx, |d, cx| d.set_sides(tabs, side, idle, cx));
    }

    /// What `side` found, carried with its thread's next message.
    pub(crate) fn bring_side_back(&mut self, side: u64, cx: &mut Context<Self>) {
        let Some((parent, title)) = self
            .state
            .thread(side)
            .and_then(|(_, t)| Some((t.parent?, t.title.clone())))
        else {
            return;
        };
        let Some(View::Structured(v)) = self.views.get(&side) else {
            return;
        };
        let notes = v.read(cx).notes();
        if let Some(View::Structured(v)) = self.views.get(&parent) {
            v.update(cx, |v, cx| v.carry(title, notes, cx));
        }
    }

    pub(crate) fn delete_side(&mut self, side: u64, cx: &mut Context<Self>) {
        self.drop_view(side);
        for s in &mut self.state.spaces {
            s.threads.retain(|t| t.id != side);
        }
        if self.work.side == Some(side) {
            self.work.side = None;
        }
        self.save();
        cx.notify();
    }

    /// The side chats of `thread`, which go when it does.
    pub(crate) fn sides_of(&self, thread: u64) -> Vec<u64> {
        self.state
            .spaces
            .iter()
            .flat_map(|s| &s.threads)
            .filter(|t| t.side && t.parent == Some(thread))
            .map(|t| t.id)
            .collect()
    }
}
