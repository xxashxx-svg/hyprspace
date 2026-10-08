use gpui::Context;
use hyprspace_proto::{Command, Delegation, Launch, Prompt, SessionId, Thread, ThreadKind};

use super::{Root, View};
use crate::time::now_ms;
use crate::transcript::Status;

impl Root {
    pub(crate) fn delegate(
        &mut self,
        request: u64,
        parent: SessionId,
        ask: Delegation,
        cx: &mut Context<Self>,
    ) {
        let found = self.sessions.get(&parent).and_then(|id| {
            let (space, t) = self.state.thread(*id)?;
            match &t.kind {
                ThreadKind::Structured { launch } => Some((*id, space.id, launch.clone())),
                ThreadKind::Terminal { .. } => None,
            }
        });
        let Some((parent, space, from)) = found else {
            self.client.send(Command::Delegated {
                request,
                ok: false,
                text: "This thread is gone.".into(),
            });
            return;
        };
        let mut launch = Launch::new(ask.agent, from.cwd.clone());
        launch.model = ask.model;
        launch.effort = ask.effort;
        launch.permission = from.permission;
        let title = ask.title.unwrap_or_else(|| {
            let line = ask.task.lines().next().unwrap_or_default();
            line.chars().take(60).collect()
        });
        let thread = Thread {
            id: self.state.take_id(),
            title,
            kind: ThreadKind::Structured { launch },
            created: now_ms(),
            touched: now_ms(),
            parent: Some(parent),
            ..Thread::default()
        };
        let Some(s) = self.state.space_mut(space) else {
            return;
        };
        s.threads.insert(0, thread.clone());
        self.delegations.insert(thread.id, request);
        self.make_view(&thread, Some(Prompt::text(ask.task)), false, cx);
        self.save();
        cx.notify();
    }

    pub(crate) fn delegation_moved(&mut self, thread: u64, status: Status, cx: &mut Context<Self>) {
        if matches!(status, Status::Working | Status::Waiting) {
            return;
        }
        if let Some(request) = self.delegations.remove(&thread) {
            let text = match self.views.get(&thread) {
                Some(View::Structured(v)) => v.read(cx).last_reply(),
                _ => String::new(),
            };
            let text = if text.is_empty() {
                "It finished without a reply.".into()
            } else {
                text
            };
            self.client.send(Command::Delegated {
                request,
                ok: status == Status::Done,
                text,
            });
        }
        let children: Vec<u64> = self
            .delegations
            .keys()
            .copied()
            .filter(|c| {
                self.state
                    .thread(*c)
                    .is_some_and(|(_, t)| t.parent == Some(thread))
            })
            .collect();
        for c in children {
            if let Some(View::Structured(v)) = self.views.get(&c) {
                v.update(cx, |v, cx| v.stop(cx));
            }
        }
    }
}
