// Settings, Appearance, after the Tauri app's redesign: a live miniature of the app at the top,
// drawn in the theme and side now showing, with the Light, Dark and System switch beside it; the
// six themes as cards with a swatch for each side; the interface's font, diff colors and
// animations; and the terminal's font, size and line height over a sample in them. Everything
// applies the moment it is clicked and is saved at once.

use std::cell::RefCell;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, Rgba,
    SharedString, div, linear_color_stop, linear_gradient, prelude::*, px, relative,
};
use hyprspace_proto::Agent;
use hyprspace_proto::state::{Appearance, DiffColors, Scheme};
use hyprspace_theme::{SANS, TERM, THEMES, ThemeInfo};

use super::controls::{group, row, section, step};
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
const MIN_LINE: f32 = 1.0;
const MAX_LINE: f32 = 1.6;
/// The interface font that follows the system: Segoe UI on Windows, SF on macOS.
pub(super) const SYSTEM_UI: &str = ".SystemUIFont";

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

/// The fonts as last applied. Painting reads them every frame, and only the main thread paints,
/// so they live here like the theme's colors do.
struct Fonts {
    term: SharedString,
    size: f32,
    line: f32,
    ui: SharedString,
}

thread_local! {
    static FONTS: RefCell<Fonts> = RefCell::new(Fonts {
        term: TERM.into(),
        size: 13.,
        line: 1.1,
        ui: SANS.into(),
    });
}

/// The family and size terminal sessions draw with.
pub(crate) fn terminal_font() -> (SharedString, f32) {
    FONTS.with(|f| {
        let f = f.borrow();
        (f.term.clone(), f.size)
    })
}

/// A terminal row's height as a multiple of the font's own line box.
pub(crate) fn terminal_line_height() -> f32 {
    FONTS.with(|f| f.borrow().line)
}

/// The family everything outside terminals draws in.
pub(crate) fn ui_font() -> SharedString {
    FONTS.with(|f| f.borrow().ui.clone())
}

/// Takes the fonts from the saved appearance. Root calls it wherever the theme applies.
pub(crate) fn set_terminal_font(a: &Appearance) {
    let term = if a.terminal_font.trim().is_empty() {
        TERM.into()
    } else {
        SharedString::from(a.terminal_font.clone())
    };
    let ui = if a.ui_font.trim().is_empty() {
        SANS.into()
    } else {
        SharedString::from(a.ui_font.clone())
    };
    FONTS.with(|f| {
        *f.borrow_mut() = Fonts {
            term,
            size: a.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE),
            line: line_height(a.terminal_line_height),
            ui,
        }
    });
}

/// A saved line height kept in range; nothing saved yet is the default.
fn line_height(saved: f32) -> f32 {
    if saved <= 0. {
        1.1
    } else {
        saved.clamp(MIN_LINE, MAX_LINE)
    }
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

/// The interface Font menu's name for a saved family.
pub(super) fn ui_font_label(family: &str) -> &'static str {
    if family == SYSTEM_UI {
        "System"
    } else {
        "Geist (built in)"
    }
}

/// `a` laid over `b` at `t`, for colors a stylesheet would `color-mix`.
fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let l = |x: f32, y: f32| x * t + y * (1. - t);
    Rgba {
        r: l(a.r, b.r),
        g: l(a.g, b.g),
        b: l(a.b, b.b),
        a: 1.,
    }
    .into()
}

impl Root {
    pub(super) fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = &self.state.appearance;
        let schemes: Vec<_> = SCHEMES
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
            })
            .collect();
        let dark = colors::theme().dark;
        let cards: Vec<_> = THEMES
            .iter()
            .enumerate()
            .map(|(i, t)| self.theme_card(i, t, t.id == current.theme, dark, cx))
            .collect();

        let size = current.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE);
        let size_stepper = stepper(
            format!("{size:.0} px"),
            self.step_by("font-smaller", "minus", size > MIN_SIZE, cx, |a| {
                a.terminal_font_size = (a.terminal_font_size - 1.).clamp(MIN_SIZE, MAX_SIZE)
            }),
            self.step_by("font-larger", "plus", size < MAX_SIZE, cx, |a| {
                a.terminal_font_size = (a.terminal_font_size + 1.).clamp(MIN_SIZE, MAX_SIZE)
            }),
        );
        let line = line_height(current.terminal_line_height);
        let line_stepper = stepper(
            format!("{line:.2}"),
            self.step_by("line-tighter", "minus", line > MIN_LINE + 0.001, cx, |a| {
                a.terminal_line_height =
                    ((line_height(a.terminal_line_height) - 0.05) * 100.).round() / 100.
            }),
            self.step_by("line-airier", "plus", line < MAX_LINE - 0.001, cx, |a| {
                a.terminal_line_height =
                    ((line_height(a.terminal_line_height) + 0.05) * 100.).round() / 100.
            }),
        );
        let diff = current.diff_colors;
        let diff_control = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(div().size(px(8.)).rounded_full().bg(colors::diff_add()))
                    .child(div().size(px(8.)).rounded_full().bg(colors::diff_del())),
            )
            .child(
                widgets::segments().children(
                    [
                        (DiffColors::RedGreen, "Red and green"),
                        (DiffColors::BlueOrange, "Blue and orange"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (value, name))| {
                        widgets::segment(("diff", i), None, name, diff == value, false).on_click(
                            cx.listener(move |r, _: &ClickEvent, window, cx| {
                                r.state.appearance.diff_colors = value;
                                r.apply_theme(window);
                                r.save();
                                cx.notify();
                            }),
                        )
                    }),
                ),
            );
        let animated = current.animations;
        let animation_control = widgets::switch("animations", animated).on_click(cx.listener(
            move |r, _: &ClickEvent, window, cx| {
                r.state.appearance.animations = !animated;
                r.apply_theme(window);
                r.save();
                cx.notify();
            },
        ));

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .px(px(4.))
                                    .text_size(px(13.))
                                    .text_color(colors::text3())
                                    .child("Preview"),
                            )
                            .child(widgets::segments().children(schemes)),
                    )
                    .child(shell_preview()),
            )
            .child(section(
                "Theme",
                div().grid().grid_cols(3).gap(px(12.)).children(cards),
            ))
            .child(group(
                "Interface",
                vec![
                    row(
                        "Font",
                        "Everything outside the terminal.",
                        self.dropdown(Picker::UiFont, ui_font_label(&current.ui_font), cx),
                    ),
                    row(
                        "Diff colors",
                        "Added and removed lines in diffs and change counts.",
                        diff_control,
                    ),
                    row(
                        "Animations",
                        "Menus, panels and rows ease in. Off, they snap.",
                        animation_control,
                    ),
                ],
            ))
            .child(group(
                "Terminal",
                vec![
                    row(
                        "Font",
                        "Terminal sessions draw in this font. Fonts without Nerd Font icons show fewer prompt symbols.",
                        self.dropdown(Picker::Font, font_label(&current.terminal_font), cx),
                    ),
                    row("Font size", "Open terminals resize to fit.", size_stepper),
                    row("Line height", "Lower is tighter, higher is airier.", line_stepper),
                    sample(),
                ],
            ))
            .into_any_element()
    }

    /// A stepper button that changes the saved appearance with `change`.
    fn step_by(
        &self,
        id: &'static str,
        glyph: &str,
        enabled: bool,
        cx: &mut Context<Self>,
        change: fn(&mut Appearance),
    ) -> AnyElement {
        step(id, glyph, enabled)
            .when(enabled, |d| {
                d.on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    change(&mut r.state.appearance);
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
            })
            .into_any_element()
    }

    /// A theme's card: a swatch for each side, the one showing ringed, then the name and a word
    /// on it. The card picks the theme; a swatch picks the theme and that side.
    fn theme_card(
        &self,
        i: usize,
        t: &ThemeInfo,
        on: bool,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = t.id;
        let orb = |side_dark: bool| {
            let th = hyprspace_theme::build(id, side_dark);
            let (bg, accent) = (colors::hsla(th.bg), colors::hsla(th.accent));
            let lit = on && dark == side_dark;
            let scheme = if side_dark {
                Scheme::Dark
            } else {
                Scheme::Light
            };
            div()
                .id(("orb", i * 2 + side_dark as usize))
                .relative()
                .p(px(3.))
                .rounded_full()
                .border_2()
                .border_color(if lit {
                    colors::accent()
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .child(
                    // a pool of the accent sitting on that side's background
                    div()
                        .size(px(52.))
                        .rounded_full()
                        .border_1()
                        .border_color(colors::ink(0.1))
                        .bg(linear_gradient(
                            150.,
                            linear_color_stop(mix(accent, bg, 0.85), 0.),
                            linear_color_stop(bg, 0.85),
                        )),
                )
                .when(lit, |d| {
                    d.child(
                        div()
                            .absolute()
                            .right(px(-4.))
                            .bottom(px(-4.))
                            .size(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(colors::surface2())
                            .border_1()
                            .border_color(colors::border2())
                            .child(icon(
                                if side_dark { "moon" } else { "sun" },
                                10.,
                                colors::text1(),
                            )),
                    )
                })
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    r.state.appearance.theme = id.to_string();
                    r.state.appearance.scheme = scheme;
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
        };
        div()
            .id(("theme", i))
            .flex()
            .flex_col()
            .gap(px(14.))
            .pt(px(18.))
            .px(px(14.))
            .pb(px(12.))
            .rounded(px(12.))
            .bg(colors::ink(0.035))
            .border_1()
            .border_color(if on {
                colors::accent()
            } else {
                colors::border1()
            })
            .cursor_pointer()
            .when(!on, |d| d.hover(|s| s.border_color(colors::border2())))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap(px(16.))
                    .child(orb(false))
                    .child(orb(true)),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(t.name),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .child(t.blurb),
                    ),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.state.appearance.theme = id.to_string();
                r.apply_theme(window);
                r.save();
                cx.notify();
            }))
            .into_any_element()
    }
}

/// A value between a minus and a plus button.
fn stepper(value: String, less: AnyElement, more: AnyElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(8.))
        .bg(colors::ink(0.05))
        .child(less)
        .child(
            div()
                .w(px(48.))
                .text_center()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text1())
                .child(value),
        )
        .child(more)
}

/// A line of colored spans, the way a terminal shows one.
fn spans(parts: &[(&str, Hsla)]) -> Div {
    div().flex().children(
        parts
            .iter()
            .map(|(text, color)| div().text_color(*color).child(text.to_string())),
    )
}

/// The caret at a prompt, blinking like the terminal's.
fn caret(h: f32) -> AnyElement {
    let t = colors::theme();
    div()
        .w(px(h * 0.5))
        .h(px(h))
        .bg(colors::hsla(t.cursor))
        .with_animation(
            "preview-caret",
            Animation::new(Duration::from_millis(1100)).repeat(),
            |d, t| d.opacity(if t < 0.5 { 1. } else { 0. }),
        )
        .into_any_element()
}

/// The app in miniature, painted by the live tokens, so whatever is picked is what this shows:
/// the sidebar with three agent threads and the one on screen, a terminal with Claude in it.
fn shell_preview() -> AnyElement {
    let t = colors::theme();
    let term_fg = colors::hsla(t.term_fg);
    let (ok, busy, dim) = (colors::ok(), colors::busy(), colors::text3());
    let line = |w: f32, strong: bool| {
        div()
            .h(px(5.))
            .w(relative(w))
            .rounded(px(3.))
            .bg(colors::ink(if strong { 0.3 } else { 0.12 }))
    };
    let mark = |agent: Agent| {
        div()
            .flex_none()
            .size(px(8.))
            .rounded_full()
            .bg(colors::brand(agent).0)
    };
    let threads = [
        (Agent::Claude, 0.78, ok),
        (Agent::Codex, 0.62, busy),
        (Agent::Claude, 0.7, colors::ink(0.25)),
    ];
    let rail = div()
        .flex_none()
        .w(px(152.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .py(px(10.))
        .px(px(8.))
        .bg(colors::surface1())
        .border_r_1()
        .border_color(colors::border0())
        .children(threads.iter().enumerate().map(|(i, &(agent, w, state))| {
            div()
                .flex()
                .items_center()
                .gap(px(7.))
                .py(px(7.))
                .px(px(8.))
                .rounded(px(6.))
                .when(i == 0, |d| {
                    d.bg(colors::surface2())
                        .border_1()
                        .border_color(colors::border1())
                })
                .child(mark(agent))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(line(w, true))
                        .child(line(w - 0.24, false)),
                )
                .child(div().flex_none().size(px(6.)).rounded_full().bg(state))
        }));
    let (family, _) = super::terminal_font();
    let pane = |agent: Option<Agent>, head_w: f32, body: Vec<Div>, cursor: bool| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(7.))
            .overflow_hidden()
            .bg(colors::hsla(t.term_bg))
            .border_1()
            .border_color(colors::border1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(22.))
                    .px(px(9.))
                    .bg(colors::surface1())
                    .border_b_1()
                    .border_color(colors::border0())
                    .child(match agent {
                        Some(a) => mark(a),
                        None => div().size(px(8.)).rounded(px(2.)).bg(colors::ink(0.25)),
                    })
                    .child(
                        div()
                            .h(px(5.))
                            .w(px(head_w))
                            .rounded(px(3.))
                            .bg(colors::ink(0.3)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .py(px(9.))
                    .px(px(10.))
                    .font_family(family.clone())
                    .text_size(px(10.5))
                    .line_height(px(16.))
                    .text_color(term_fg)
                    .children(body)
                    .when(cursor, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().text_color(ok).child("\u{276f}"))
                                .child(caret(12.)),
                        )
                    }),
            )
    };
    let thread = pane(
        Some(Agent::Claude),
        46.,
        vec![
            spans(&[("\u{276f} ", ok), ("claude --resume", term_fg)]),
            spans(&[("Reading src/themes.rs", dim)]),
            spans(&[("\u{2713} ", ok), ("3 files changed", term_fg)]),
        ],
        true,
    );
    let dot = || div().size(px(6.)).rounded_full().bg(colors::ink(0.16));
    div()
        .h(px(236.))
        .flex()
        .flex_col()
        .rounded(px(12.))
        .overflow_hidden()
        .bg(colors::bg())
        .border_1()
        .border_color(colors::border2())
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(5.))
                .h(px(24.))
                .px(px(10.))
                .bg(colors::surface1())
                .border_b_1()
                .border_color(colors::border0())
                .child(dot())
                .child(dot())
                .child(
                    div()
                        .ml(px(6.))
                        .w(px(44.))
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(colors::ink(0.1)),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .w(px(22.))
                        .h(px(7.))
                        .rounded(px(3.))
                        .bg(colors::accent()),
                ),
        )
        .child(
            div().flex_1().min_h_0().flex().child(rail).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .gap(px(8.))
                    .p(px(10.))
                    .child(thread),
            ),
        )
        .into_any_element()
}

/// A build log in the terminal's own colors, font, size and line height, so a change shows
/// before leaving Settings.
fn sample() -> AnyElement {
    let t = colors::theme();
    let (family, size) = super::terminal_font();
    let fg = colors::hsla(t.term_fg);
    let (ok, busy, wait, dim) = (
        colors::ok(),
        colors::busy(),
        colors::waiting(),
        colors::text3(),
    );
    let row_h = (size * 1.35 * terminal_line_height() / 1.1).round();
    div()
        .my(px(12.))
        .py(px(12.))
        .px(px(14.))
        .rounded(px(8.))
        .bg(colors::hsla(t.term_bg))
        .border_1()
        .border_color(colors::border1())
        .font_family(family)
        .text_size(px(size))
        .line_height(px(row_h))
        .text_color(fg)
        .child(spans(&[
            ("\u{276f} ", ok),
            ("cargo build -p hyprspace", fg),
        ]))
        .child(spans(&[(
            &format!("   Compiling hyprspace-ui v{}", crate::update::VERSION),
            dim,
        )]))
        .child(spans(&[
            ("\u{2713} ", ok),
            ("Finished in ", fg),
            ("6.8s", busy),
        ]))
        .child(spans(&[
            ("src/main.rs", wait),
            ("  0O 1lI {}[] () ;:", dim),
        ]))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(size * 0.6))
                .child(div().text_color(ok).child("\u{276f}"))
                .child(caret(size)),
        )
        .into_any_element()
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
    fn saved_sizes_out_of_range_are_clamped() {
        set_terminal_font(&Appearance {
            terminal_font_size: 64.,
            terminal_line_height: 9.,
            ..Appearance::default()
        });
        assert_eq!(terminal_font(), (SharedString::from(TERM), MAX_SIZE));
        assert_eq!(terminal_line_height(), MAX_LINE);
        // a state saved before line height existed reads as the default
        assert_eq!(line_height(0.), 1.1);
        set_terminal_font(&Appearance::default());
        assert_eq!(terminal_font().1, 13.);
        assert_eq!(ui_font(), SharedString::from(SANS));
    }

    #[test]
    fn mixing_lands_between_the_two() {
        let black: Hsla = Rgba {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 1.,
        }
        .into();
        let white: Hsla = Rgba {
            r: 1.,
            g: 1.,
            b: 1.,
            a: 1.,
        }
        .into();
        let half = Rgba::from(mix(white, black, 0.5));
        assert!((half.r - 0.5).abs() < 0.01);
    }
}
