// Settings, Appearance: the six themes as small pictures of their own surfaces and accent, light
// or dark, and the font terminal sessions draw with. Everything applies the moment it is clicked
// and is saved at once.

use std::cell::RefCell;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, SharedString, div, prelude::*, px,
    relative,
};
use hyprspace_proto::state::{Appearance, Scheme};
use hyprspace_theme::{TERM, THEMES, ThemeInfo};

use super::controls::{block, group, row, section, step};
use super::{Picker, Root};
use crate::assets::icon;
use crate::{colors, widgets};

const SCHEMES: [(Scheme, &str, &str); 3] = [
    (Scheme::Dark, "Dark", "moon"),
    (Scheme::Light, "Light", "sun"),
    (Scheme::System, "System", "monitor"),
];

pub(super) const MIN_SIZE: f32 = 10.;
pub(super) const MAX_SIZE: f32 = 20.;

/// Monospace families people install for terminals, offered when they are on this machine.
/// Anything else installed with "Mono" or "Code" in its name is offered too.
const KNOWN_MONO: &[&str] = &[
    "Cascadia Mono",
    "Cascadia Code",
    "Consolas",
    "Lucida Console",
    "Courier New",
    "Menlo",
    "Monaco",
    "SF Mono",
    "Hack",
    "Iosevka",
    "Inconsolata",
];

thread_local! {
    /// The terminal's font family and size. Painting reads it every frame, and only the main
    /// thread paints, so it lives here like the theme's colors do.
    static TERMINAL: RefCell<(SharedString, f32)> = RefCell::new((TERM.into(), 13.));
}

/// The family and size terminal sessions draw with.
pub(crate) fn terminal_font() -> (SharedString, f32) {
    TERMINAL.with(|t| t.borrow().clone())
}

/// Takes the terminal font from the saved appearance. Root calls it wherever the theme applies.
pub(crate) fn set_terminal_font(a: &Appearance) {
    let family = if a.terminal_font.trim().is_empty() {
        TERM.into()
    } else {
        SharedString::from(a.terminal_font.clone())
    };
    let size = a.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE);
    TERMINAL.with(|t| *t.borrow_mut() = (family, size));
}

/// The installed families the Font menu offers, besides the built-in one.
pub(super) fn mono_families(installed: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = installed
        .into_iter()
        .filter(|n| n != TERM && !n.starts_with('.') && !n.starts_with('@'))
        .filter(|n| KNOWN_MONO.contains(&n.as_str()) || n.contains("Mono") || n.contains("Code"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The Font menu's name for a saved family.
pub(super) fn font_label(family: &str) -> String {
    if family.trim().is_empty() {
        "JetBrains Mono (built in)".into()
    } else {
        family.to_string()
    }
}

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
        let cards: Vec<_> = cards.collect();
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
        let schemes: Vec<_> = schemes.collect();
        let mode_desc = match current.scheme {
            Scheme::System if cfg!(target_os = "macos") => {
                "Follows the light or dark mode set in macOS."
            }
            Scheme::System => "Follows the light or dark mode set in Windows.",
            Scheme::Dark => "Always the theme's dark side.",
            Scheme::Light => "Always the theme's light side.",
        };

        let size = current.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE);
        let stepper = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(8.))
            .bg(colors::ink(0.05))
            .child(self.size_step("font-smaller", "minus", -1., size > MIN_SIZE, cx))
            .child(
                div()
                    .w(px(44.))
                    .text_center()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(format!("{size:.0} px")),
            )
            .child(self.size_step("font-larger", "plus", 1., size < MAX_SIZE, cx));

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(section(
                "Theme",
                div().grid().grid_cols(3).gap(px(12.)).children(cards),
            ))
            .child(block(vec![row(
                "Mode",
                mode_desc,
                widgets::segments().children(schemes),
            )]))
            .child(group(
                "Terminal",
                vec![
                    row(
                        "Font",
                        "Terminal sessions draw in this font. Fonts without Nerd Font icons show fewer prompt symbols.",
                        self.dropdown(Picker::Font, font_label(&current.terminal_font), cx),
                    ),
                    row("Font size", "Open terminals resize to fit.", stepper),
                    sample(),
                ],
            ))
            .into_any_element()
    }

    fn size_step(
        &self,
        id: &'static str,
        glyph: &str,
        by: f32,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        step(id, glyph, enabled)
            .when(enabled, |d| {
                d.on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    let a = &mut r.state.appearance;
                    a.terminal_font_size = (a.terminal_font_size + by).clamp(MIN_SIZE, MAX_SIZE);
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
            })
            .into_any_element()
    }
}

/// A few lines in the terminal's own colors, font and size, so a change shows before leaving
/// Settings.
fn sample() -> AnyElement {
    let t = colors::theme();
    let (family, size) = super::terminal_font();
    let line = |prompt: &str, rest: &str| {
        div()
            .flex()
            .child(
                div()
                    .text_color(colors::hsla(t.ansi[2]))
                    .child(prompt.to_string()),
            )
            .child(rest.to_string())
    };
    div()
        .my(px(12.))
        .p(px(12.))
        .rounded(px(8.))
        .bg(colors::hsla(t.term_bg))
        .border_1()
        .border_color(colors::border1())
        .font_family(family)
        .text_size(px(size))
        .line_height(px((size * 1.35).round()))
        .text_color(colors::hsla(t.term_fg))
        .child(line("~/hyprspace ", "$ cargo build -p hyprspace"))
        .child(format!(
            "   Compiling hyprspace-ui v{}",
            crate::update::VERSION
        ))
        .child(line("~/hyprspace ", "$ git status --short  0O 1lI {}[]"))
        .into_any_element()
}

/// A theme's card: the shell in miniature, drawn in that theme's own tokens on the side now
/// showing, with its name under it.
fn theme_card(i: usize, t: &ThemeInfo, on: bool) -> gpui::Stateful<Div> {
    let th = colors::other(t.id);
    let c = colors::hsla;
    let bar = |color: Hsla, w: f32| div().h(px(4.)).w(relative(w)).rounded(px(2.)).bg(color);
    let rail = div()
        .flex_none()
        .w(px(38.))
        .h_full()
        .flex()
        .flex_col()
        .gap(px(5.))
        .p(px(6.))
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
        .p(px(6.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(bar(c(th.ink(0.3)), 0.75))
                .child(bar(c(th.ink(0.12)), 0.5)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .h(px(12.))
                .px(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(c(th.border1))
                .bg(c(th.surface2))
                .child(div().size(px(6.)).rounded_full().bg(c(th.accent))),
        );
    let preview = div()
        .h(px(56.))
        .flex()
        .rounded(px(7.))
        .overflow_hidden()
        .border_1()
        .border_color(if on { colors::text2() } else { c(th.border1) })
        .bg(c(th.bg))
        .child(rail)
        .child(main);
    div()
        .id(("theme", i))
        .flex()
        .flex_col()
        .gap(px(7.))
        .cursor_pointer()
        .group("theme-card")
        .child(preview)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(2.))
                .text_size(px(12.5))
                .when(on, |d| {
                    d.font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                })
                .when(!on, |d| {
                    d.text_color(colors::text2())
                        .group_hover("theme-card", |s| s.text_color(colors::text1()))
                })
                .child(t.name)
                .when(on, |d| d.child(icon("check", 12., colors::text1()))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_menu_offers_monospace_families_only() {
        let names = [
            "Arial",
            "Consolas",
            "Fira Code",
            "Noto Sans Mono",
            TERM,
            "Segoe UI",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            mono_families(names),
            vec!["Consolas", "Fira Code", "Noto Sans Mono"]
        );
    }

    #[test]
    fn a_saved_size_out_of_range_is_clamped() {
        set_terminal_font(&Appearance {
            terminal_font_size: 64.,
            ..Appearance::default()
        });
        assert_eq!(terminal_font(), (SharedString::from(TERM), MAX_SIZE));
        set_terminal_font(&Appearance::default());
        assert_eq!(terminal_font().1, 13.);
    }
}
