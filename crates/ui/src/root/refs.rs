use std::hash::{DefaultHasher, Hash, Hasher};

use hyprspace_proto::{Command, ThreadKind};

use super::Root;

impl Root {
    pub(super) fn list_threads(&mut self, cx: &mut gpui::App) {
        let mut h = DefaultHasher::new();
        for s in &self.state.spaces {
            (&s.name, s.machine.is_none()).hash(&mut h);
            for t in &s.threads {
                let structured = matches!(t.kind, ThreadKind::Structured { .. });
                (t.id, &t.title, structured).hash(&mut h);
            }
        }
        let seen = h.finish();
        if std::mem::replace(&mut self.listed, seen) != seen {
            crate::suggest::set_threads(self.thread_items(), cx);
        }
    }

    fn thread_items(&self) -> Vec<crate::suggest::Item> {
        self.state
            .spaces
            .iter()
            .filter(|s| s.machine.is_none())
            .flat_map(|s| {
                s.threads
                    .iter()
                    .filter(|t| matches!(t.kind, ThreadKind::Structured { .. }))
                    .map(|t| crate::suggest::Item {
                        label: t.title.clone(),
                        detail: format!("Thread in {}", s.name),
                        insert: format!("{}{}", crate::suggest::THREAD, t.id),
                    })
            })
            .collect()
    }

    pub(super) fn reference(&mut self, from: u64, target: u64, cx: &mut gpui::Context<Self>) {
        let Some((_, t)) = self.state.thread(target).filter(|_| target != from) else {
            return;
        };
        self.client.send(Command::ExportThread {
            thread: from,
            journal: t.journal(),
            title: t.title.clone(),
        });
        cx.notify();
    }
}
