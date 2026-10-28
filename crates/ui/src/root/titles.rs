use std::time::{Duration, Instant};

use gpui::Context;
use hyprspace_proto::{Agent, Command};

use super::{Root, View};
use crate::time::now_ms;

const ASKED: usize = 6000;
const KEY: Duration = Duration::from_millis(32);
const BLINK: u64 = 530;

impl Root {
    pub(super) fn name_thread(
        &mut self,
        thread: u64,
        text: &str,
        previous: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let text = text.trim();
        if text.is_empty() || self.machines.remote(thread).is_some() {
            return;
        }
        let Some((_, t)) = self.state.thread(thread) else {
            return;
        };
        let Some(launch) = t.agent() else {
            return;
        };
        let model = match launch.agent {
            Agent::Codex => self
                .agents
                .iter()
                .find(|a| a.agent == Agent::Codex)
                .and_then(|a| a.catalog.models.iter().find(|m| m.id.contains("luna")))
                .map(|m| m.id.clone()),
            Agent::Claude => None,
        };
        self.naming.insert(thread, t.title.clone());
        self.client.send(Command::Title {
            thread,
            agent: launch.agent,
            cwd: launch.cwd.clone(),
            model,
            text: shorten(text),
            previous,
        });
        self.tick_titles(cx);
    }

    pub(super) fn named(&mut self, thread: u64, title: Option<String>, cx: &mut Context<Self>) {
        let Some(asked) = self.naming.remove(&thread) else {
            return;
        };
        let same = self
            .state
            .thread(thread)
            .is_some_and(|(_, t)| t.title == asked);
        if let Some(title) = title.filter(|_| same) {
            self.retitle(thread, title);
            self.typing.insert(thread, Instant::now());
            self.tick_titles(cx);
        }
    }

    pub(crate) fn naming(&self, thread: u64) -> bool {
        self.naming.contains_key(&thread)
    }

    pub(crate) fn typed(&self, thread: u64, title: &str) -> Option<(String, bool)> {
        if self.naming.contains_key(&thread) {
            return Some((title.to_string(), (now_ms() / BLINK).is_multiple_of(2)));
        }
        let keys = (self.typing.get(&thread)?.elapsed().as_millis() / KEY.as_millis()) as usize;
        (keys < title.chars().count()).then(|| (title.chars().take(keys).collect(), true))
    }

    fn tick_titles(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.title_tick, true) {
            return;
        }
        cx.spawn(async move |this, cx| {
            loop {
                let wait = this.update(cx, |r, cx| {
                    let state = &r.state;
                    r.typing.retain(|id, at| {
                        state.thread(*id).is_some_and(|(_, t)| {
                            at.elapsed() < KEY * (t.title.chars().count() as u32 + 1)
                        })
                    });
                    cx.notify();
                    let wait = if !r.typing.is_empty() {
                        Some(KEY)
                    } else if !r.naming.is_empty() {
                        Some(Duration::from_millis(BLINK))
                    } else {
                        None
                    };
                    r.title_tick = wait.is_some();
                    wait
                });
                let Ok(Some(wait)) = wait else {
                    break;
                };
                cx.background_executor().timer(wait).await;
            }
        })
        .detach();
    }

    pub(crate) fn can_rename_itself(&self, thread: u64) -> bool {
        matches!(self.views.get(&thread), Some(View::Structured(_)))
            && self.machines.remote(thread).is_none()
            && !self.naming.contains_key(&thread)
    }

    pub(crate) fn regenerate_title(&mut self, thread: u64, cx: &mut gpui::Context<Self>) {
        let Some(View::Structured(v)) = self.views.get(&thread) else {
            return;
        };
        let text = v.read(cx).gist();
        let previous = self.state.thread(thread).map(|(_, t)| t.title.clone());
        self.name_thread(thread, &text, previous, cx);
    }
}

fn shorten(text: &str) -> String {
    if text.len() <= ASKED {
        return text.to_string();
    }
    let cut = |at: usize| {
        (0..=at)
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(0)
    };
    let head = cut(ASKED / 3);
    let tail = cut(text.len() - ASKED * 2 / 3);
    format!("{}\n\n...\n\n{}", &text[..head], &text[tail..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_conversation_keeps_its_start_and_end() {
        assert_eq!(shorten("fix it"), "fix it");
        let long = format!("{}{}", "a".repeat(ASKED), "é".repeat(ASKED));
        let short = shorten(&long);
        assert!(short.starts_with('a') && short.ends_with('é'));
        assert!(short.len() < ASKED + 20);
    }
}
