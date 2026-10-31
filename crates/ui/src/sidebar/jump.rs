use std::time::Duration;

use gpui::{Context, KeyBinding, Modifiers, Task, Window};

use crate::root::{Root, Screen};
use crate::transcript::Status;

const HOLD: Duration = Duration::from_millis(200);

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = sidebar, no_json)]
pub struct Jump(pub usize);

gpui::actions!(sidebar, [NextWaiting]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys(
        (1..=9)
            .map(|n| KeyBinding::new(&format!("secondary-{n}"), Jump(n - 1), None))
            .chain([KeyBinding::new("secondary-shift-j", NextWaiting, None)]),
    );
}

fn next_after(list: &[u64], current: Option<u64>, hit: impl Fn(u64) -> bool) -> Option<u64> {
    let from = current
        .and_then(|c| list.iter().position(|&id| id == c))
        .map_or(0, |i| i + 1);
    (0..list.len())
        .map(|k| list[(from + k) % list.len()])
        .find(|&id| Some(id) != current && hit(id))
}

#[derive(Default)]
pub(crate) struct Hold {
    timer: Option<Task<()>>,
    pub(crate) shown: bool,
    spoiled: bool,
}

impl Root {
    pub(crate) fn hold_keys(&mut self, m: &Modifiers, window: &mut Window, cx: &mut Context<Self>) {
        if !m.secondary() {
            self.spoil_hold(false, cx);
        } else if m.number_of_modifiers() > 1 {
            self.spoil_hold(true, cx);
        } else if !self.hold.spoiled && self.hold.timer.is_none() && !self.hold.shown {
            self.hold.timer = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(HOLD).await;
                let _ = this.update(cx, |r, cx| {
                    r.hold.shown = true;
                    cx.notify();
                });
            }));
        }
    }

    pub(crate) fn spoil_hold(&mut self, held: bool, cx: &mut Context<Self>) {
        self.hold.timer = None;
        self.hold.spoiled = held;
        if std::mem::take(&mut self.hold.shown) {
            cx.notify();
        }
    }

    pub(crate) fn jump(&mut self, a: &Jump, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(&id) = self.jump_list(cx).get(a.0) {
            self.menu = None;
            self.open_thread(id, window, cx);
        }
    }

    pub(crate) fn next_waiting(
        &mut self,
        _: &NextWaiting,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = match self.screen {
            Screen::Thread(id) => Some(id),
            _ => None,
        };
        let waiting = |id| self.status.get(&id) == Some(&Status::Waiting);
        if let Some(id) = next_after(&self.jump_list(cx), current, waiting) {
            self.menu = None;
            self.open_thread(id, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_waiting_thread_comes_after_this_one_and_wraps() {
        let list = [1, 2, 3, 4];
        let waiting = |id| id == 1 || id == 3;
        assert_eq!(next_after(&list, Some(1), waiting), Some(3));
        assert_eq!(next_after(&list, Some(3), waiting), Some(1));
        assert_eq!(next_after(&list, Some(4), waiting), Some(1));
        assert_eq!(next_after(&list, None, waiting), Some(1));
        assert_eq!(next_after(&list, Some(9), waiting), Some(1));
        assert_eq!(next_after(&list, Some(1), |id| id == 1), None);
        assert_eq!(next_after(&[], None, waiting), None);
    }
}
