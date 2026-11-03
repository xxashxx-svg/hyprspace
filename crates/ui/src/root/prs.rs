// Each thread's pull request, found by its branch in its repo's list from `gh`, read for unsettled
// threads every few minutes. A thread whose pull request merges while we watch settles.

use std::path::PathBuf;

use gpui::Context;
use hyprspace_proto::git::{PrState, PullRequest};
use hyprspace_proto::{Command, FolderCommand, Thread};

use super::Root;

impl Root {
    /// The repo to ask and the branch to look for: a worktree's own, or the branch the thread's
    /// folder is on when that isn't the default one.
    fn branch_of(&self, t: &Thread) -> Option<(PathBuf, String)> {
        if let Some(w) = &t.worktree {
            return (!w.placeholder()).then(|| (w.repo.clone(), w.branch.clone()));
        }
        let cwd = t.cwd().clone();
        let branch = &self.git.get(&cwd)?.branch;
        let named = branch.is_repo && !["", "HEAD", "main", "master"].contains(&&*branch.branch);
        named.then(|| (cwd, branch.branch.clone()))
    }

    pub(crate) fn poll_prs(&mut self) {
        let mut repos: Vec<PathBuf> = Vec::new();
        for s in self.state.spaces.iter().filter(|s| s.machine.is_none()) {
            for t in s.threads.iter().filter(|t| t.active()) {
                if let Some((repo, _)) = self.branch_of(t)
                    && !repos.contains(&repo)
                {
                    repos.push(repo);
                }
            }
        }
        for cwd in repos {
            self.client
                .send(Command::Folder(FolderCommand::PullRequests { cwd }));
        }
    }

    pub(crate) fn pr_of(&self, t: &Thread) -> Option<&PullRequest> {
        let (repo, branch) = self.branch_of(t)?;
        let mine = self.prs.get(&repo)?.iter().filter(|p| p.branch == branch);
        let mut mine = mine.peekable();
        let newest = *mine.peek()?;
        let open = |p: &&PullRequest| matches!(p.state, PrState::Open | PrState::Draft);
        Some(mine.find(open).unwrap_or(newest))
    }

    pub(crate) fn prs_arrived(
        &mut self,
        cwd: PathBuf,
        prs: Vec<PullRequest>,
        cx: &mut Context<Self>,
    ) {
        self.prs.insert(cwd, prs);
        let mut merged = Vec::new();
        for s in self.state.spaces.iter().filter(|s| s.machine.is_none()) {
            for t in s.threads.iter().filter(|t| t.active()) {
                match self.pr_of(t).map(|p| p.state) {
                    Some(PrState::Open | PrState::Draft) => {
                        self.pr_open.insert(t.id);
                    }
                    Some(PrState::Merged) if self.pr_open.contains(&t.id) => merged.push(t.id),
                    _ => {}
                }
            }
        }
        for id in merged {
            self.pr_open.remove(&id);
            if self.busy(id) || self.on_screen(id) {
                continue;
            }
            if let Some(t) = self.state.thread_mut(id) {
                t.set_settled(true, crate::time::now_ms());
            }
            self.free(id);
            self.save();
        }
        cx.notify();
    }
}
