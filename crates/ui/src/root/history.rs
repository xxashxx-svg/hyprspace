use gpui::{Context, Window};

use super::{Root, Screen};

const KEEP: usize = 50;

impl Root {
    pub(crate) fn left(&mut self, from: Screen) {
        if self.traveling || from == self.screen {
            return;
        }
        self.past.push(from);
        if self.past.len() > KEEP {
            self.past.remove(0);
        }
        self.ahead.clear();
    }

    pub(crate) fn can_travel(&self, back: bool) -> bool {
        let stack = if back { &self.past } else { &self.ahead };
        stack.iter().any(|s| self.reachable(*s))
    }

    pub(crate) fn travel(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        loop {
            let next = if back {
                self.past.pop()
            } else {
                self.ahead.pop()
            };
            let Some(to) = next else {
                return;
            };
            if !self.reachable(to) || to == self.screen {
                continue;
            }
            let from = self.screen;
            if back {
                self.ahead.push(from);
            } else {
                self.past.push(from);
            }
            self.traveling = true;
            match to {
                Screen::Thread(id) => self.open_thread(id, window, cx),
                Screen::Compose(space) => self.compose(space, window, cx),
                Screen::Settings => self.open_settings(window, cx),
            }
            self.traveling = false;
            return;
        }
    }

    fn reachable(&self, screen: Screen) -> bool {
        match screen {
            Screen::Thread(id) => self.state.thread(id).is_some(),
            Screen::Compose(Some(space)) => self.state.space(space).is_some(),
            Screen::Compose(None) | Screen::Settings => true,
        }
    }
}
