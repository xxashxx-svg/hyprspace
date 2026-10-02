// Settings, Appearance: the six themes as cards, each with a small picture of its own surfaces
// and accent, then light or dark. Both apply the moment they are clicked and are saved at once.

use gpui::{AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, div, prelude::*, px, relative};
use hyprspace_proto::state::Scheme;
use hyprspace_theme::{THEMES, ThemeInfo};

use super::section;
use crate::assets::icon;
use crate::root::Root;
use crate::{colors, widgets};

const SCHEMES: [(Scheme, &str, &str); 3] = [
    (Scheme::Dark, "Dark", "moon"),
    (Scheme::Light, "Light", "sun"),
    (Scheme::System, "Match the system", "monitor"),
];

impl Root {
    pub(super) fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = &self.state.appearance;
        let cards = THEMES.iter().enumerate().map(|(i, t)| {
            let id = t.id;
            theme_card(i, t, t.id == current.theme).on_click(cx.listener(
                move |r, _: &ClickEvent, window, cx| {
                    r.state.appearance.theme = id.to_string();
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                },
            ))
        });
        let schemes = SCHEMES
            .iter()
            .enumerate()
            .map(|(i, &(scheme, name, glyph))| {
                widgets::segment(
                    ("scheme", i),
                    Some(glyph),
                    name,
                    current.scheme == scheme,
                    false,
                )
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.state.appearance.scheme = scheme;
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
            });
        div()
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(section(
                "Theme",
                div().grid().grid_cols(3).gap(px(12.)).children(cards),
            ))
            .child(section(
                "Mode",
                div().flex().child(widgets::segments().children(schemes)),
            ))
            .into_any_element()
    }
}

/// A theme's card: the shell in miniature, drawn in that theme's own tokens on the side now
/// showing, and its name under it.
fn theme_card(i: usize, t: &ThemeInfo, on: bool) -> gpui::Stateful<Div> {
    let th = colors::other(t.id);
    let c = colors::hsla;
    let bar = |color: Hsla, w: f32| div().h(px(5.)).w(relative(w)).rounded(px(3.)).bg(color);
    let rail = div()
        .flex_none()
        .w(px(52.))
        .h_full()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(8.))
        .bg(c(th.surface1))
        .border_r_1()
        .border_color(c(th.border0))
        .child(bar(c(th.ink(0.3)), 0.8))
        .child(bar(c(th.ink(0.12)), 0.6))
        .child(bar(c(th.ink(0.12)), 0.7));
    let main = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_between()
        .p(px(8.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(bar(c(th.ink(0.3)), 0.75))
                .child(bar(c(th.ink(0.12)), 0.5)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .h(px(16.))
                .px(px(3.))
                .rounded(px(5.))
                .border_1()
                .border_color(c(th.border1))
                .bg(c(th.surface2))
                .child(div().size(px(8.)).rounded_full().bg(c(th.accent))),
        );
    let preview = div()
        .h(px(78.))
        .flex()
        .rounded(px(7.))
        .overflow_hidden()
        .border_1()
        .border_color(c(th.border1))
        .bg(c(th.bg))
        .child(rail)
        .child(main);
    div()
        .id(("theme", i))
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(10.))
        .rounded(px(10.))
        .border_1()
        .bg(colors::surface2())
        .cursor_pointer()
        .when(on, |d| d.border_color(colors::text3()))
        .when(!on, |d| {
            d.border_color(colors::border1())
                .hover(|s| s.border_color(colors::border2()))
        })
        .child(preview)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child(t.name),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.))
                        .text_color(colors::text3())
                        .child(t.blurb),
                )
                .when(on, |d| d.child(icon("check", 13., colors::text1()))),
        )
}
