// Settings, General: the app card (version, updates and the intro, after General.tsx's
// AppCard), how new sessions start, and where the app keeps its data.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, ClickEvent, Context, FontWeight, Transformation, div,
    percentage, prelude::*, px, relative,
};
use hyprspace_theme::MONO;

use super::{group, row};
use crate::assets::{icon, logo};
use crate::root::Root;
use crate::update::{Phase, VERSION};
use crate::{colors, widgets};

/// The engine's `persist::state_dir`, worked out the same way. The channel has no request for it
/// yet, and the rule is three lines, so the UI repeats it rather than guess.
fn state_dir() -> PathBuf {
    if let Ok(d) = std::env::var("HYPRSPACE_STATE_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".hyprspace").join("native")
}

impl Root {
    pub(super) fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        let terminal = self.state.composer.terminal;
        let kinds = [
            (false, "Structured", "sparkles"),
            (true, "Terminal", "terminal"),
        ];
        let start = widgets::segments().children(kinds.into_iter().enumerate().map(
            |(i, (value, name, glyph))| {
                widgets::segment(("start-as", i), Some(glyph), name, terminal == value, false)
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.update_prefs(|p| p.terminal = value, cx)
                    }))
            },
        ));
        let mono = |text: String| {
            div()
                .max_w(px(360.))
                .truncate()
                .px(px(9.))
                .py(px(5.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(0.04))
                .font_family(MONO)
                .text_size(px(12.))
                .text_color(colors::text2())
                .child(text)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(self.app_card(cx))
            .child(group(
                "Sessions",
                vec![row(
                    "New sessions start as",
                    if terminal {
                        "The agent's own CLI runs in a terminal."
                    } else {
                        "The agent's work shows as a transcript you can read and steer."
                    },
                    start,
                )],
            ))
            .child(group(
                "Data",
                vec![row(
                    "State folder",
                    "Your spaces, threads and settings are saved here.",
                    mono(state_dir().display().to_string()),
                )],
            ))
            .into_any_element()
    }
}

impl Root {
    /// The version, whether an update is out, and the button that checks or installs it
    /// (settings.css `.gn-app`).
    fn app_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let u = self.updater.read(cx);
        let busy = matches!(u.phase, Phase::Checking | Phase::Busy(_));
        let (dot, text) = match u.phase {
            Phase::UpToDate => (colors::ok(), colors::text3()),
            Phase::Checking | Phase::Busy(_) => (colors::busy(), colors::text3()),
            Phase::Available(_) => (colors::accent(), colors::text1()),
            Phase::Failed(_) => (colors::error(), colors::text2()),
            Phase::Idle | Phase::Unmanaged => (colors::text3(), colors::text3()),
        };
        let status = div()
            .flex()
            .items_center()
            .gap(px(7.))
            .min_w_0()
            .text_size(px(12.5))
            .text_color(text)
            .child(div().flex_none().size(px(6.)).rounded_full().bg(dot))
            .child(div().min_w_0().truncate().child(u.status()));
        let updater = self.updater.clone();
        let action = if matches!(u.phase, Phase::Available(_)) {
            widgets::primary_frame("gn-install")
                .gap(px(7.))
                .child(icon("download", 14., colors::on_accent()))
                .child("Restart and update")
                .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, |u, cx| u.install(cx)))
                .into_any_element()
        } else {
            let off = busy || u.phase == Phase::Unmanaged;
            let glyph = icon("refresh-cw", 14., colors::text2());
            let glyph = if busy {
                glyph
                    .with_animation(
                        "gn-spin",
                        Animation::new(Duration::from_millis(800)).repeat(),
                        |s, t| s.with_transformation(Transformation::rotate(percentage(t))),
                    )
                    .into_any_element()
            } else {
                glyph.into_any_element()
            };
            let label = if u.phase == Phase::Checking {
                "Checking"
            } else {
                "Check for updates"
            };
            widgets::button_frame("gn-check")
                .gap(px(7.))
                .child(glyph)
                .child(label)
                .when(off, |b| b.opacity(0.5).cursor_default())
                .when(!off, |b| {
                    b.on_click(move |_: &ClickEvent, _, cx| {
                        updater.update(cx, |u, cx| u.check(false, cx))
                    })
                })
                .into_any_element()
        };
        let progress = matches!(u.phase, Phase::Busy(_)).then(|| {
            let fill = div().h_full().rounded(px(2.)).bg(colors::accent());
            let fill = match u.progress() {
                Some(p) => fill.w(relative(p)).into_any_element(),
                None => fill
                    .absolute()
                    .w(relative(0.35))
                    .with_animation(
                        "gn-slide",
                        Animation::new(Duration::from_millis(1100)).repeat(),
                        |d, t| d.left(relative(-0.35 + 1.35 * t)),
                    )
                    .into_any_element(),
            };
            div()
                .relative()
                .h(px(2.))
                .mx(px(16.))
                .mb(px(14.))
                .rounded(px(2.))
                .overflow_hidden()
                .bg(colors::ink(0.08))
                .child(fill)
        });
        let name = div()
            .flex()
            .items_baseline()
            .gap(px(4.))
            .text_size(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colors::text1())
            .child("HyprSpace")
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text3())
                    .child(format!("v{VERSION}")),
            );
        let main = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .p(px(16.))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(44.))
                    .rounded(px(11.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .child(logo(26.)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(name)
                    .child(status),
            )
            .child(action);
        let intro = div()
            .id("show-intro")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(26.))
            .px(px(9.))
            .rounded(px(7.))
            .text_color(colors::text2())
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
            .child(icon("circle-play", 13., colors::text2()))
            .child("Show the intro")
            .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.show_intro(window, cx)));
        let foot = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .px(px(16.))
            .py(px(11.))
            .border_t_1()
            .border_color(colors::border0())
            .bg(colors::ink(0.02))
            .text_size(px(12.))
            .text_color(colors::text3())
            .child(div().min_w_0().child(
                "Runs the Claude, Codex and Gemini CLIs you already have, side by side, on your own machine.",
            ))
            .child(intro);
        div()
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .overflow_hidden()
            .child(main)
            .children(progress)
            .child(foot)
            .into_any_element()
    }
}
