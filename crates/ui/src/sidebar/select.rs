use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseMoveEvent, Window, div,
    prelude::*,
};

use crate::colors;
use crate::root::{Action, MenuEntry, MenuItems, Root, Screen};
use crate::time::{local_now, presets, to_ms, wake_short};
use crate::zoom::px;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Settle,
    Wake,
    Unsettle,
}

impl Verb {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Verb::Settle => "Settle",
            Verb::Wake => "Wake",
            Verb::Unsettle => "Bring back",
        }
    }

    fn fits(self, t: &hyprspace_proto::Thread) -> bool {
        match self {
            Verb::Settle => t.active(),
            Verb::Wake => t.snooze.is_some(),
            Verb::Unsettle => t.settled,
        }
    }
}

pub(crate) struct Sweep {
    pub verb: Verb,
    pub ids: Vec<u64>,
}

impl Root {
    pub(crate) fn row_click(
        &mut self,
        id: u64,
        e: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        let m = e.modifiers();
        if m.secondary() {
            if self.selection.is_empty()
                && let Screen::Thread(on) = self.screen
                && on != id
            {
                self.selection.insert(on);
            }
            if !self.selection.remove(&id) {
                self.selection.insert(id);
            }
            self.anchor = Some(id);
            cx.notify();
        } else if m.shift {
            let list = self.jump_list(cx);
            let from = self.anchor.or(match self.screen {
                Screen::Thread(on) => Some(on),
                _ => None,
            });
            let (Some(a), Some(b)) = (
                from.and_then(|f| list.iter().position(|&t| t == f)),
                list.iter().position(|&t| t == id),
            ) else {
                return;
            };
            self.selection = list[a.min(b)..=a.max(b)].iter().copied().collect();
            cx.notify();
        } else {
            self.selection.clear();
            self.anchor = Some(id);
            self.open_thread(id, window, cx);
        }
    }

    pub(crate) fn bulk_menu(&self) -> MenuItems {
        let n = self.selection.len();
        let now = local_now();
        let times = presets(now)
            .into_iter()
            .map(|p| MenuEntry::Item {
                label: p.label.into(),
                hint: Some(wake_short(p.at, now).into()),
                action: Action::SnoozePicked(to_ms(p.at)),
            })
            .collect();
        let local = self.selection.iter().any(|&id| {
            self.state
                .thread(id)
                .is_some_and(|(s, _)| s.machine.is_none())
        });
        let mut menu = vec![
            MenuEntry::item(format!("Settle {n} threads"), Action::SettlePicked),
            MenuEntry::Sub {
                label: format!("Snooze {n} threads").into(),
                icon: "clock",
                entries: times,
            },
            MenuEntry::item(format!("Mark {n} unread"), Action::UnreadPicked),
        ];
        if local {
            menu.push(MenuEntry::Divider);
            menu.push(MenuEntry::item(
                format!("Delete {n} threads"),
                Action::DeletePicked,
            ));
        }
        menu
    }

    pub(crate) fn act_picked(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids: Vec<u64> = self.selection.drain().collect();
        match action {
            Action::SettlePicked => self.settle_many(&ids, true, window, cx),
            Action::SnoozePicked(at) => {
                self.snooze_many(&ids, Some(hyprspace_proto::Snooze::Time { at }), window, cx)
            }
            Action::UnreadPicked => {
                self.unseen.extend(ids.iter().filter(|&&id| {
                    self.state
                        .thread(id)
                        .is_some_and(|(s, _)| s.machine.is_none())
                }));
                cx.notify();
            }
            Action::DeletePicked => {
                for id in ids {
                    if self
                        .state
                        .thread(id)
                        .is_some_and(|(s, _)| s.machine.is_none())
                    {
                        self.act(Action::RemoveThread(id), window, cx);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn start_sweep(&mut self, id: u64, verb: Verb, cx: &mut Context<Self>) {
        self.sweep = Some(Sweep {
            verb,
            ids: vec![id],
        });
        cx.notify();
    }

    pub(crate) fn sweep_over(&mut self, id: u64, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let fits = self.state.thread(id).map(|(_, t)| t.clone());
        let Some(sweep) = self.sweep.as_mut() else {
            return;
        };
        if e.pressed_button != Some(gpui::MouseButton::Left) || sweep.ids.contains(&id) {
            return;
        }
        if fits.is_some_and(|t| sweep.verb.fits(&t)) {
            sweep.ids.push(id);
            cx.notify();
        }
    }

    pub(crate) fn end_sweep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sweep) = self.sweep.take() else {
            return;
        };
        match sweep.verb {
            Verb::Settle => self.settle_many(&sweep.ids, true, window, cx),
            Verb::Unsettle => self.settle_many(&sweep.ids, false, window, cx),
            Verb::Wake => self.snooze_many(&sweep.ids, None, window, cx),
        }
    }

    pub(crate) fn swept(&self, id: u64) -> Option<AnyElement> {
        let sweep = self.sweep.as_ref().filter(|s| s.ids.len() > 1)?;
        sweep
            .ids
            .contains(&id)
            .then(|| verb_badge(sweep.verb.word()))
    }
}

pub(crate) fn verb_badge(word: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(18.))
        .px(px(6.))
        .rounded(px(4.))
        .border_1()
        .border_color(colors::accent().opacity(0.4))
        .bg(colors::accent().opacity(0.1))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text1())
        .child(word)
        .into_any_element()
}
