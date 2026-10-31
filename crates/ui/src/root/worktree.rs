use std::path::PathBuf;

use gpui::{Context, Window};
use hyprspace_proto::git::{Worktree, WorktreeAction};
use hyprspace_proto::{Command, FolderCommand};

use super::Root;

impl Root {
    pub(crate) fn finish_worktree(
        &mut self,
        id: u64,
        action: WorktreeAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((_, t)) = self.state.thread(id) else {
            return;
        };
        let Some(worktree) = t.worktree.clone() else {
            return;
        };
        if self.busy(id) {
            self.notify("Wait for the thread to finish first.".into(), cx);
            return;
        }
        let message = t.title.clone();
        let doing = match action {
            WorktreeAction::Merge => format!("Merging {} into {}", worktree.branch, worktree.base),
            WorktreeAction::PullRequest => {
                format!("Opening a pull request for {}", worktree.branch)
            }
            WorktreeAction::Remove => {
                if let Some(t) = self.state.thread_mut(id) {
                    t.settled = true;
                    t.snooze = None;
                    t.pinned = None;
                }
                self.drop_view(id);
                self.leave(window, cx);
                self.save();
                "Removing the worktree".into()
            }
        };
        self.send_worktree(worktree, action, message);
        self.notify(doing, cx);
    }

    pub(crate) fn send_worktree(
        &self,
        worktree: Worktree,
        action: WorktreeAction,
        message: String,
    ) {
        self.client.send(Command::Folder(FolderCommand::Worktree {
            worktree,
            action,
            message,
        }));
    }

    /// Every thread on that worktree, forks included, now names the branch by its new name.
    pub(crate) fn branch_renamed(
        &mut self,
        path: &PathBuf,
        from: &str,
        to: &str,
        cx: &mut Context<Self>,
    ) {
        for view in self.views.values() {
            if let super::View::Structured(v) = view {
                v.update(cx, |v, cx| v.branch_moved(path, cx));
            }
        }
        for s in &mut self.state.spaces {
            for t in &mut s.threads {
                if let Some(w) = t
                    .worktree
                    .as_mut()
                    .filter(|w| &w.path == path && w.branch == from)
                {
                    w.branch = to.to_string();
                }
            }
        }
        self.save();
    }

    pub(crate) fn worktree_done(
        &mut self,
        path: &PathBuf,
        action: WorktreeAction,
        result: &Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        if action == WorktreeAction::Remove && result.is_ok() {
            for s in &mut self.state.spaces {
                for t in &mut s.threads {
                    if t.worktree.as_ref().is_some_and(|w| &w.path == path) {
                        t.worktree = None;
                    }
                }
            }
            self.save();
        }
        let (Ok(line) | Err(line)) = result;
        self.notify(line.clone(), cx);
    }
}
