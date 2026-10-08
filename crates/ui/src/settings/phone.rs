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
                "Your phone talks to this computer directly, encrypted. Nothing goes through a server, and it only works while HyprSpace is open here.",
                switch,
            ),
            row(
                "Networks",
                match prefs.network {
                    Network::Everywhere => {
                        "Any network this computer is on. Away from home, your phone reaches it over Tailscale."
                    }
                    Network::Tailscale => {
                        "Only devices on your tailnet can reach it, from home or anywhere else."
                    }
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
                        "On {} at port {}.",
                        match status.addresses.as_slice() {
                            [] => "no network yet".to_string(),
                            a => a.join(", "),
                        },
                        status.port
                    ),
                    false,
                ),
                None => row_note("Starting", "One moment.".to_string(), false),
            });
        }

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group("Access", access));
        if prefs.on && status.on {
            page = page.child(group("Pair a phone", vec![self.pairing_row(cx)]));
        }
        let mut paired = self.device_rows(cx);
        if !status.security.is_empty() {
            paired.push(row(
                "Security code",
                "A phone paired by typing the code shows this code too. If the two differ, forget that phone.",
                div()
                    .font_family(hyprspace_theme::MONO)
                    .text_size(px(13.))
                    .text_color(colors::text1())
                    .child(status.security.clone()),
            ));
        }
        page = page.child(group("Paired phones", paired));
        page.child(group(
            "What your phone gets",
            vec![
                row(
                    "What it sees",
                    "Your spaces and threads, their transcripts, the screens of terminal threads, the agents and models you can start, and your theme's colors.",
                    div(),
                ),
                row(
                    "What it can do",
                    "Send messages, answer approvals, stop a run, type into terminals, start threads and settle them. Each goes through the same steps as doing it here.",
                    div(),
                ),
                row(
                    "How it travels",
                    "Over TLS under a certificate made on this computer. Your phone saves its fingerprint when it pairs and refuses any other.",
                    div(),
                ),
                row(
                    "Get the app",
                    "Download HyprSpace-android.apk from a release on GitHub and open it on your phone.",
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
                "Show a pairing code",
                "Open HyprSpace on your phone, tap Pair and scan the code. A code works once, for five minutes.",
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
                            .child("Scan this with HyprSpace on your phone. If the camera can't read it, type the code instead."),
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
                        "It works once, until {until}. To type it in, your phone also needs this address: {}.",
                        self.phone
                            .status
                            .addresses
                            .first()
                            .map(|a| format!("{a}:{}", self.phone.status.port))
                            .unwrap_or_else(|| "this computer's address".into())
                    )))
                    .child(
                        div().flex().child(
                            widgets::button("phone-unpair", "Cancel").on_click(cx.listener(
                                |r, _: &ClickEvent, _, _| {
                                    r.client.send(Command::Phone(PhoneCommand::StopPairing))
                                },
                            )),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn device_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let devices = &self.phone.status.devices;
        if devices.is_empty() {
            return vec![row(
                "No phones yet",
                "A phone you pair shows here. Forget one to stop it connecting.",
                div(),
            )];
        }
        let now = now_ms();
        devices
            .iter()
            .map(|d| {
                let id = d.id.clone();
                let mut seen = if d.online {
                    "Connected now".to_string()
                } else {
                    match ago(d.seen, now).as_str() {
                        "now" => "Connected a moment ago".to_string(),
                        a => format!("Last connected {a} ago"),
                    }
                };
                if !d.app.is_empty() {
                    seen = format!("{seen}. App {}", d.app);
                }
                row(
                    d.name.clone(),
                    seen,
                    widgets::button(("phone-forget", d.paired as usize), "Forget").on_click(
                        cx.listener(move |r, _: &ClickEvent, _, _| {
                            r.client
                                .send(Command::Phone(PhoneCommand::Forget { device: id.clone() }))
                        }),
                    ),
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
