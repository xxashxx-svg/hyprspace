use gpui::{Context, Window};
use hyprspace_proto::Draft;

use super::{Root, Screen};
use crate::time::now_ms;

impl Root {
    pub(crate) fn keep_draft(&mut self, mut draft: Draft, cx: &mut Context<Self>) {
        if !self
            .state
            .space(draft.space)
            .is_some_and(|s| s.machine.is_none())
        {
            return;
        }
        let at = self
            .composing
            .and_then(|id| self.state.drafts.iter().position(|d| d.id == id));
        if draft.text.trim().is_empty() && draft.images.is_empty() {
            if at.is_some() {
                self.drop_draft();
                cx.notify();
            }
            return;
        } else if let Some(at) = at {
            let old = &mut self.state.drafts[at];
            draft.id = old.id;
            draft.created = old.created;
            if *old == draft {
                return;
            }
            *old = draft;
        } else {
            draft.id = self.state.take_id();
            draft.created = now_ms();
            self.composing = Some(draft.id);
            self.unlisted = Some(draft.id);
            self.state.drafts.push(draft);
        }
        self.save();
        cx.notify();
    }

    fn flush_draft(&mut self, cx: &mut Context<Self>) {
        let draft = self.composer.read(cx).snapshot(cx);
        self.keep_draft(draft, cx);
    }

    pub(crate) fn drop_draft(&mut self) {
        if let Some(id) = self.composing.take() {
            self.state.drafts.retain(|d| d.id != id);
            self.save();
        }
    }

    pub(crate) fn park_draft(&mut self, cx: &mut Context<Self>) {
        self.flush_draft(cx);
        if self.composing.take().is_some() {
            self.load_draft(None, cx);
        }
    }

    /// The composer's own space picker moves its draft; coming from anywhere else to another
    /// space leaves it behind.
    pub(crate) fn park_elsewhere(&mut self, space: Option<u64>, cx: &mut Context<Self>) {
        let theirs = self
            .composing
            .and_then(|id| self.state.drafts.iter().find(|d| d.id == id))
            .map(|d| d.space);
        if !matches!(self.screen, Screen::Compose(_)) && theirs.is_some_and(|s| Some(s) != space) {
            self.park_draft(cx);
        }
    }

    pub(crate) fn new_thread(
        &mut self,
        space: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.park_draft(cx);
        self.compose(space, window, cx);
    }

    pub(crate) fn open_draft(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_draft(cx);
        let Some(draft) = self.state.drafts.iter().find(|d| d.id == id).cloned() else {
            return;
        };
        self.composing = Some(id);
        self.unlisted = None;
        self.compose(Some(draft.space), window, cx);
        self.load_draft(Some(draft), cx);
    }

    pub(crate) fn discard_draft(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(at) = self.state.drafts.iter().position(|d| d.id == id) else {
            return;
        };
        let draft = self.state.drafts.remove(at);
        if self.composing == Some(id) {
            self.composing = None;
            self.load_draft(None, cx);
        }
        self.remember_draft(draft, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn drafting(&self) -> Option<u64> {
        self.composing
            .filter(|_| matches!(self.screen, Screen::Compose(_)))
    }

    /// A draft started on screen joins the sidebar once the user leaves it, after T3 Code's.
    pub(crate) fn listed(&self, id: u64) -> bool {
        self.drafting() != Some(id) || self.unlisted != Some(id)
    }

    fn load_draft(&mut self, draft: Option<Draft>, cx: &mut Context<Self>) {
        let mut draft = draft.unwrap_or_else(|| Draft {
            prefs: self.state.composer.clone(),
            ..Draft::default()
        });
        // structured sessions are a setting, not a pick
        draft.prefs.structured = self.state.composer.structured;
        self.composer.update(cx, |c, cx| c.load(draft, cx));
    }
}
