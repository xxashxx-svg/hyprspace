// Settings, Phone: the switch for the phone bridge (engine/src/phone), where it listens, the
// QR code a phone pairs with, and the phones paired so far. The copy says exactly what a phone
// gets and how it travels, since this is the one way into the app from another device.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*, px};
use hyprspace_proto::Command;
use hyprspace_proto::phone::{Network, PhoneCommand};

use super::Root;
use super::controls::{group, row, text};
use crate::assets::icon;
use crate::time::{ago, local, now_ms};
use crate::{colors, widgets};

/// The releases page: the Android app is the `HyprSpace-android.apk` asset on a release.
const APP: &str = "https://github.com/xxashxx-svg/hyprspace/releases";

impl Root {
    pub(super) fn phone_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let prefs = self.state.phone;
        let status = &self.phone.status;
        let switch = widgets::switch("phone-on", prefs.on).on_click(cx.listener(
            move |r, _: &ClickEvent, _, cx| {
                r.state.phone.on = !prefs.on;
                r.save();
                r.phone_enable();
                cx.notify();
            },
        ));
        let networks = widgets::segments().children(
            [
                (Network::Everywhere, "Wi-Fi and Tailscale"),
                (Network::Tailscale, "Tailscale only"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (value, name))| {
                widgets::segment(("phone-net", i), None, name, prefs.network == value, false)
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.state.phone.network = value;
                        r.save();
                        r.phone_enable();
                        cx.notify();
                    }))
            }),
        );
        let mut access = vec![
            row(
                "Let your phone connect",
                "Direct and encrypted, no server. Works while HyprSpace is open.",
                switch,
            ),
            row(
                "Networks",
                match prefs.network {
                    Network::Everywhere => "Wi-Fi at home, Tailscale away.",
                    Network::Tailscale => "Only devices on your tailnet.",
                },
                networks,
            ),
        ];
        if prefs.on {
            access.push(match &status.error {
                Some(e) => row_note("Not listening", e.clone(), true),
                None if status.on => row_note(
                    "Listening",
                    format!(
                        "{}, port {}",
                        match status.addresses.as_slice() {
                            [] => "no network yet".to_string(),
                            a => a.join(", "),
                        },
                        status.port
                    ),
                    false,
                ),
                None => row_note("Starting", String::new(), false),
            });
        }

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group("Access", access));
        let mut phones = Vec::new();
        if prefs.on && status.on {
            phones.push(self.pairing_row(cx));
        }
        phones.extend(self.device_rows(cx));
        page = page.child(group("Phones", phones));
        page.child(group(
            "The app",
            vec![
                row(
                    "What it can do",
                    "See your threads and terminals, send messages, answer approvals and start threads.",
                    div(),
                ),
                row(
                    "Security",
                    "TLS pinned to this computer's certificate. The pairing code is never sent.",
                    div(),
                ),
                row(
                    "Get the app",
                    "The Android APK is on GitHub releases.",
                    widgets::button_frame("phone-get")
                        .gap(px(6.))
                        .child("Open")
                        .child(icon("external-link", 12., colors::text3()))
                        .on_click(|_: &ClickEvent, _, cx| cx.open_url(APP)),
                ),
            ],
        ))
        .into_any_element()
    }

    fn pairing_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(p) = &self.phone.pairing else {
            return row(
                "Pair a phone",
                "Scan the code with the HyprSpace app.",
                widgets::button("phone-pair", "Show code").on_click(cx.listener(
                    |r, _: &ClickEvent, _, _| r.client.send(Command::Phone(PhoneCommand::Pair)),
                )),
            );
        };
        let light = hyprspace_theme::build(&self.state.appearance.theme, false);
        let until = local(p.expires).format("%-I:%M %p").to_string();
        div()
            .flex()
            .gap(px(20.))
            .p(px(16.))
            .items_center()
            .child(crate::phone::qr::qr(
                &p.link,
                220.,
                colors::hsla(light.text1),
                colors::hsla(light.bg),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(colors::text2())
                            .child("Scan with the HyprSpace app, or type the code."),
                    )
                    .child(
                        div()
                            .font_family(hyprspace_theme::MONO)
                            .text_size(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(p.code.clone()),
                    )
                    .child(text(format!(
                        "Address {}. New code at {until}.",
                        self.phone
                            .status
                            .addresses
                            .first()
                            .map(|a| format!("{a}:{}", self.phone.status.port))
                            .unwrap_or_else(|| "this computer's address".into())
                    )))
                    .child(
                        div()
                            .flex()
                            .child(widgets::button("phone-unpair", "Cancel").on_click(
                                cx.listener(|r, _: &ClickEvent, _, _| {
                                    r.client.send(Command::Phone(PhoneCommand::StopPairing))
                                }),
                            )),
                    ),
            )
            .into_any_element()
    }

    fn device_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let devices = &self.phone.status.devices;
        if devices.is_empty() {
            return vec![row("No phones yet", "", div())];
        }
        let now = now_ms();
        devices
            .iter()
            .map(|d| {
                let id = d.id.clone();
                let mut seen = if d.online {
                    "Online".to_string()
                } else {
                    match ago(d.seen, now).as_str() {
                        "now" => "Seen just now".to_string(),
                        a => format!("Seen {a} ago"),
                    }
                };
                if !d.app.is_empty() {
                    seen = format!("{seen} · App {}", d.app);
                }
                row(
                    d.name.clone(),
                    seen,
                    widgets::button(("phone-forget", d.paired as usize), "Revoke")
                        .tooltip(widgets::tip("Cut it off now. It has to pair again."))
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, _| {
                            r.client
                                .send(Command::Phone(PhoneCommand::Forget { device: id.clone() }))
                        })),
                )
            })
            .collect()
    }
}

fn row_note(title: &str, note: String, bad: bool) -> AnyElement {
    super::controls::row_with(
        None,
        title,
        div()
            .text_size(px(12.))
            .text_color(if bad {
                colors::error()
            } else {
                colors::text3()
            })
            .child(note),
        div(),
    )
}
