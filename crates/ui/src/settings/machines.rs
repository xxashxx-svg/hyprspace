// Settings, Computers: the computers this one can run threads on, and the ones that can run
// threads on this one. Both ride the phone bridge (engine/src/phone), so this computer lets
// others in through the same switch, code and revoke as phones.

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, Focusable, FontWeight, Window, div,
    prelude::*, px,
};
use hyprspace_proto::Command;
use hyprspace_proto::peer::{Found, PeerCommand};
use hyprspace_proto::phone::PhoneCommand;

use super::Root;
use super::controls::{group, row, text};
use crate::input::{InputEvent, TextInput};
use crate::time::{ago, local, now_ms};
use crate::{colors, widgets};

impl Root {
    pub(super) fn machines_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let input = self.pair_input(cx);
        let mut mine = Vec::new();
        for (i, p) in self.machines.peers.iter().enumerate() {
            let id = p.id.clone();
            let note = match (&p.error, p.online) {
                (_, true) if p.version.is_empty() => "Connected".to_string(),
                (_, true) => format!("Connected · HyprSpace {}", p.version),
                (Some(e), false) => e.clone(),
                (None, false) => "Connecting".to_string(),
            };
            mine.push(row(
                p.name.clone(),
                note,
                widgets::button(("peer-forget", i), "Forget")
                    .tooltip(widgets::tip("Unpair here and on that computer."))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, _| {
                        r.client
                            .send(Command::Peer(PeerCommand::Forget { peer: id.clone() }))
                    })),
            ));
        }
        if mine.is_empty() {
            mine.push(row(
                "None yet",
                "Pair one below. Its threads show in your sidebar, and new threads can start there.",
                div(),
            ));
        }

        let paired: Vec<&str> = self.machines.peers.iter().map(|p| p.id.as_str()).collect();
        let found: Vec<Found> = self
            .machines
            .found
            .iter()
            .filter(|f| !paired.contains(&f.fingerprint.as_str()))
            .cloned()
            .collect();
        let mut add = vec![
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .py(px(14.))
                .child(text(
                    "On the other computer, open Settings, Computers, and show its code. Paste its link here, or type its code and pick it from the list.",
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h(px(32.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .rounded(px(8.))
                                .bg(colors::ink(0.05))
                                .text_size(px(13.))
                                .child(input.clone()),
                        )
                        .child(widgets::primary("peer-pair", "Pair").on_click(cx.listener(
                            |r, _: &ClickEvent, _, cx| r.pair_typed(None, cx),
                        ))),
                )
                .children(self.machines.error.clone().map(|e| {
                    div()
                        .text_size(px(12.))
                        .text_color(colors::error())
                        .child(e)
                }))
                .into_any_element(),
        ];
        for (i, f) in found.into_iter().enumerate() {
            let name = f.name.clone();
            let picked = self
                .machines
                .chosen
                .as_ref()
                .is_some_and(|c| c.fingerprint == f.fingerprint);
            add.push(row(
                name,
                if picked {
                    "Type its code above"
                } else {
                    "On your network"
                },
                widgets::button(("peer-found", i), "Pair with code").on_click(cx.listener(
                    move |r, _: &ClickEvent, window, cx| r.pick_found(f.clone(), window, cx),
                )),
            ));
        }

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group("Computers you can use", mine))
            .child(group("Add a computer", add))
            .child(group(
                "Let other computers use this one",
                self.host_rows(cx),
            ))
            .into_any_element()
    }

    fn pair_input(&mut self, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some(i) = &self.machines.input {
            return i.clone();
        }
        let input = cx.new(|cx| TextInput::new("Link or code", false, cx));
        self._subs
            .push(cx.subscribe(&input, |r, _, e: &InputEvent, cx| match e {
                InputEvent::Submit => r.pair_typed(None, cx),
                InputEvent::Changed => {
                    r.machines.error = None;
                    cx.notify();
                }
                _ => {}
            }));
        self.machines.input = Some(input.clone());
        input
    }

    fn pick_found(&mut self, found: Found, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.machines.input.clone() else {
            return;
        };
        if !input.read(cx).text().trim().is_empty() {
            return self.pair_typed(Some(found), cx);
        }
        let hint = format!("Code for {}", found.name);
        input.update(cx, |i, cx| i.set_placeholder(hint, cx));
        self.machines.chosen = Some(found);
        self.machines.error = None;
        window.focus(&input.focus_handle(cx), cx);
        cx.notify();
    }

    fn pair_typed(&mut self, found: Option<Found>, cx: &mut Context<Self>) {
        let Some(input) = self.machines.input.clone() else {
            return;
        };
        let typed = input.read(cx).text().trim().to_string();
        let only = (self.machines.found.len() == 1).then(|| self.machines.found[0].clone());
        let link = if typed.starts_with("hyprspace://") {
            typed
        } else if typed.is_empty() {
            self.machines.error = Some("Paste the link or type the code first.".into());
            cx.notify();
            return;
        } else if let Some(f) = found.or(self.machines.chosen.clone()).or(only) {
            format!(
                "hyprspace://pair?n={}&h={}&p={}&f={}&c={}",
                encode(&f.name),
                f.hosts.join(","),
                f.port,
                f.fingerprint,
                encode(&typed)
            )
        } else {
            self.machines.error =
                Some("Pick the computer from the list, or paste its whole link.".into());
            cx.notify();
            return;
        };
        self.machines.error = None;
        self.machines.chosen = None;
        input.update(cx, |i, cx| {
            i.set_text("", cx);
            i.set_placeholder("Link or code", cx);
        });
        self.client.send(Command::Peer(PeerCommand::Pair { link }));
        cx.notify();
    }

    fn host_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let prefs = self.state.phone;
        let status = &self.phone.status;
        let mut rows = Vec::new();
        if !(prefs.on && status.on) {
            rows.push(row(
                "Off",
                "Turn on Settings, Phone, Let your phone connect. Computers come in the same way.",
                div(),
            ));
        } else {
            rows.push(match &self.phone.pairing {
                None => row(
                    "Pair a computer",
                    "Shows a code to type on the other computer.",
                    widgets::button("peer-code", "Show code").on_click(cx.listener(
                        |r, _: &ClickEvent, _, _| {
                            r.client.send(Command::Phone(PhoneCommand::Pair))
                        },
                    )),
                ),
                Some(p) => {
                    let link = p.link.clone();
                    let until = local(p.expires).format("%-I:%M %p").to_string();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .py(px(14.))
                        .child(
                            div()
                                .font_family(hyprspace_theme::MONO)
                                .text_size(px(22.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child(p.code.clone()),
                        )
                        .child(text(format!(
                            "Type it on the other computer, or copy the link over. New code at {until}."
                        )))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(widgets::button("peer-copy", "Copy link").on_click(
                                    move |_: &ClickEvent, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            link.clone(),
                                        ))
                                    },
                                ))
                                .child(widgets::button("peer-stop", "Cancel").on_click(
                                    cx.listener(|r, _: &ClickEvent, _, _| {
                                        r.client.send(Command::Phone(PhoneCommand::StopPairing))
                                    }),
                                )),
                        )
                        .into_any_element()
                }
            });
        }
        let now = now_ms();
        for d in status.devices.iter().filter(|d| d.computer) {
            let id = d.id.clone();
            let seen = if d.online {
                "Connected".to_string()
            } else {
                match ago(d.seen, now).as_str() {
                    "now" => "Seen just now".to_string(),
                    a => format!("Seen {a} ago"),
                }
            };
            rows.push(row(
                d.name.clone(),
                seen,
                widgets::button(("peer-revoke", d.paired as usize), "Revoke")
                    .tooltip(widgets::tip("Cut it off now. It has to pair again."))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, _| {
                        r.client
                            .send(Command::Phone(PhoneCommand::Forget { device: id.clone() }))
                    })),
            ));
        }
        rows
    }
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            b => format!("%{b:02X}"),
        })
        .collect()
}
