// The theme's tokens as GPUI colors. The theme and its side (light or dark) can change while the
// app runs; `set` swaps every token at once. Rendering only ever happens on the main thread, so
// the current theme is a thread local rather than state threaded through every render function.

use std::cell::Cell;

use gpui::{
    BoxShadow, FontStyle, FontWeight, HighlightStyle, Hsla, WindowAppearance, point, px, rgba,
};
use hyprspace_proto::Agent;
use hyprspace_proto::state::Scheme;
use hyprspace_syntax::Kind;
use hyprspace_theme::{Color, Theme, build};

thread_local! {
    static CURRENT: Cell<Theme> = Cell::new(build("t3", true));
}

/// Switches to theme `id` on its dark or light side, with blue and orange for added and removed
/// lines when `blue_orange`.
pub fn set(id: &str, dark: bool, blue_orange: bool) {
    let mut t = build(id, dark);
    if blue_orange {
        (t.diff_add, t.diff_del) = hyprspace_theme::blue_orange(dark);
    }
    CURRENT.with(|c| c.set(t));
}

/// Whether `scheme` paints the dark side, given what the system is set to.
pub fn dark(scheme: Scheme, system: WindowAppearance) -> bool {
    match scheme {
        Scheme::Dark => true,
        Scheme::Light => false,
        Scheme::System => matches!(
            system,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

pub fn theme() -> Theme {
    CURRENT.with(Cell::get)
}

pub fn hsla(c: Color) -> Hsla {
    rgba(c.0).into()
}

macro_rules! tokens {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name() -> Hsla {
            hsla(theme().$name)
        })*
    };
}

tokens!(
    bg,
    surface1,
    surface2,
    surface3,
    accent,
    accent_hover,
    accent_dim,
    on_accent,
    link,
    text1,
    text2,
    text3,
    border0,
    border1,
    border2,
    busy,
    waiting,
    ok,
    error,
    diff_add,
    diff_del,
    selection,
);

/// `rgba(var(--ink), a)`: a line or wash that works on both sides.
pub fn ink(a: f32) -> Hsla {
    hsla(theme().ink(a))
}

/// An agent's signature color and its gradient's second stop.
pub fn brand(agent: Agent) -> (Hsla, Hsla) {
    let (a, b) = hyprspace_theme::brand(agent.cli());
    (hsla(a), hsla(b))
}

/// How code of `kind` is drawn: the theme's code colors, comments in italics. Code blocks in the
/// transcript use it; the file viewer can move over to it from its ANSI-based colors.
pub fn syntax(kind: Kind) -> HighlightStyle {
    let s = theme().syntax;
    let color = |c: Color| HighlightStyle {
        color: Some(hsla(c)),
        ..Default::default()
    };
    match kind {
        Kind::Comment => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..color(s.comment)
        },
        Kind::Keyword => color(s.keyword),
        Kind::String | Kind::Code => color(s.string),
        Kind::Number => color(s.number),
        Kind::Constant | Kind::Escape | Kind::Attribute => color(s.constant),
        Kind::Type | Kind::Tag => color(s.ty),
        Kind::Function | Kind::Macro => color(s.function),
        Kind::Operator | Kind::Punctuation => color(s.punctuation),
        Kind::Property | Kind::Variable => color(s.plain),
        Kind::Link => HighlightStyle {
            color: Some(link()),
            ..Default::default()
        },
        Kind::Heading => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..color(s.keyword)
        },
        Kind::Emphasis => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
    }
}

/// tokens.css's `--shadow-2`: under the composer and popups.
pub fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(theme().shadow),
        offset: point(px(0.), px(12.)),
        blur_radius: px(34.),
        spread_radius: px(0.),
        inset: false,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_theme_it_is_set_to() {
        set("t3", true, false);
        assert!((border2().a - 0x1a as f32 / 255.0).abs() < 0.01);
        assert_eq!(bg().a, 1.0);
        let dark_text = text1();
        set("t3", false, false);
        assert_ne!(text1(), dark_text);
        set("iris", true, false);
        assert_ne!(accent(), hsla(build("t3", true).accent));
        set("t3", true, false);
    }

    #[test]
    fn the_scheme_overrides_the_system() {
        assert!(dark(Scheme::Dark, WindowAppearance::Light));
        assert!(!dark(Scheme::Light, WindowAppearance::Dark));
        assert!(dark(Scheme::System, WindowAppearance::Dark));
        assert!(!dark(Scheme::System, WindowAppearance::Light));
    }
}
