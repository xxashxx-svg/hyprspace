use gpui::{
    AnyElement, Context, Focusable, IntoElement, KeyDownEvent, Window, div, point, prelude::*,
};

use super::RADIUS;
use crate::colors;
use crate::root::{Rename, Root, Screen};
use crate::zoom::px;

impl Root {
    pub(crate) fn enter_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let list = self.jump_list(cx);
        let start = match self.screen {
            Screen::Thread(on) if list.contains(&on) => Some(on),
            _ => list.first().copied(),
        };
        if let Some(id) = start {
            self.cursor = Some(id);
            self.reveal = Some(id);
            window.focus(&self.list_focus, cx);
            cx.notify();
        }
    }

    fn leave_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cursor = None;
        window.focus(&self.search.focus_handle(cx), cx);
        cx.notify();
    }

    pub(crate) fn list_key(
        &mut self,
        e: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let list = self.jump_list(cx);
        let Some(at) = self
            .cursor
            .filter(|c| list.contains(c))
            .or(list.first().copied())
        else {
            return;
        };
        let i = list.iter().position(|&t| t == at).unwrap_or(0);
        let shift = e.keystroke.modifiers.shift;
        let to = match e.keystroke.key.as_str() {
            "down" => (i + 1).min(list.len() - 1),
            "up" if i == 0 => return self.leave_list(window, cx),
            "up" => i - 1,
            "home" => 0,
            "end" => list.len() - 1,
            "escape" => return self.leave_list(window, cx),
            "enter" | "space" => {
                cx.stop_propagation();
                self.cursor = None;
                self.selection.clear();
                return self.open_thread(at, window, cx);
            }
            "f2" => {
                cx.stop_propagation();
                return self.start_rename(Rename::Thread(at), window, cx);
            }
            "f10" if shift => return self.key_menu(at, cx),
            "menu" => return self.key_menu(at, cx),
            _ => return,
        };
        cx.stop_propagation();
        self.cursor = Some(list[to]);
        self.reveal = Some(list[to]);
        cx.notify();
    }

    fn key_menu(&mut self, id: u64, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(b) = self.row_bounds.borrow().get(&id).copied() else {
            return;
        };
        let Some((_, t)) = self.state.thread(id) else {
            return;
        };
        let menu = self.thread_menu(t);
        self.menu = Some((point(b.left() + px(8.), b.bottom()), menu));
        self.menu_sub = None;
        self.delete_armed = false;
        cx.notify();
    }

    pub(crate) fn key_ring(&self, id: u64) -> Option<AnyElement> {
        (self.list_focused && self.cursor == Some(id)).then(|| {
            div()
                .absolute()
                .inset_0()
                .rounded(px(RADIUS))
                .border_2()
                .border_color(colors::accent().opacity(0.75))
                .into_any_element()
        })
    }
}
