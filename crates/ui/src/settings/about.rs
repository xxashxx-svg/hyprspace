// Settings, About: the version and its updates (the same updater the corner card uses), this
// version's notes and the intro again, and where the code and the issue tracker live.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, ClickEvent, Context, Transformation, div, percentage,
    prelude::*, px, relative,
};

use super::Root;
use super::controls::{group, row, row_with, text};
use crate::assets::{icon, logo};
use crate::update::{Phase, VERSION};
use crate::{colors, widgets};

const REPO: &str = "https://github.com/xxashxx-svg/hyprspace";

impl Root {
    pub(super) fn about(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut app = vec![self.version_row(cx)];
        if crate::update::has_notes() {
            let updater = self.updater.clone();
            app.push(row(
                "What's new",
                format!("The changes in version {VERSION}."),
                widgets::button("about-news", "Show").on_click(move |_: &ClickEvent, _, cx| {
                    updater.update(cx, |u, cx| u.show_whats_new(cx))
                }),
            ));
        }
        app.push(row(
            "Intro",
            "The short tour from the first run.",
            widgets::button("about-intro", "Show")
                .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.show_intro(window, cx))),
        ));
        let link = |id: &'static str, url: String| {
            widgets::button_frame(id)
                .gap(px(6.))
                .child("Open")
                .child(icon("external-link", 12., colors::text3()))
                .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(group("HyprSpace", app))
            .child(group(
                "Links",
                vec![
                    row(
                        "Source code",
                        "github.com/xxashxx-svg/hyprspace",
                        link("about-repo", REPO.into()),
                    ),
                    row(
                        "Report a problem",
                        "Opens a new issue on GitHub.",
                        link("about-issue", format!("{REPO}/issues/new")),
                    ),
                ],
            ))
            .into_any_element()
    }

    /// The version, what the updater is doing, and the button that checks or installs.
    fn version_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let u = self.updater.read(cx);
        let busy = matches!(u.phase, Phase::Checking | Phase::Busy(_));
        let dot = match u.phase {
            Phase::UpToDate => colors::ok(),
            Phase::Checking | Phase::Busy(_) => colors::busy(),
            Phase::Available(_) => colors::accent(),
            Phase::Failed(_) => colors::error(),
            Phase::Idle | Phase::Unmanaged => colors::text3(),
        };
        let status = div()
            .flex()
            .items_center()
            .gap(px(7.))
            .min_w_0()
            .child(div().flex_none().size(px(6.)).rounded_full().bg(dot))
            .child(
                text(format!(
                    "Version {VERSION}. {}.",
                    u.status().trim_end_matches('.')
                ))
                .min_w_0(),
            );
        let updater = self.updater.clone();
        let action = if matches!(u.phase, Phase::Available(_)) {
            widgets::primary_frame("about-install")
                .gap(px(7.))
                .child(icon("download", 13., colors::on_accent()))
                .child("Restart and update")
                .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, |u, cx| u.install(cx)))
                .into_any_element()
        } else {
            let off = busy || u.phase == Phase::Unmanaged;
            let glyph = icon("refresh-cw", 13., colors::text2());
            let glyph = if busy {
                glyph
                    .with_animation(
                        "about-spin",
                        Animation::new(Duration::from_millis(800)).repeat(),
                        |s, t| s.with_transformation(Transformation::rotate(percentage(t))),
                    )
                    .into_any_element()
            } else {
                glyph.into_any_element()
            };
            widgets::button_frame("about-check")
                .gap(px(7.))
                .child(glyph)
                .child(if u.phase == Phase::Checking {
                    "Checking"
                } else {
                    "Check for updates"
                })
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
                        "about-slide",
                        Animation::new(Duration::from_millis(1100)).repeat(),
                        |d, t| d.left(relative(-0.35 + 1.35 * t)),
                    )
                    .into_any_element(),
            };
            div()
                .relative()
                .mt(px(6.))
                .h(px(2.))
                .rounded(px(2.))
                .overflow_hidden()
                .bg(colors::ink(0.08))
                .child(fill)
        });
        let lead = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(36.))
            .rounded(px(9.))
            .bg(colors::ink(0.05))
            .child(logo(22.))
            .into_any_element();
        row_with(
            Some(lead),
            "HyprSpace",
            div().flex().flex_col().child(status).children(progress),
            action,
        )
    }
}
