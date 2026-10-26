// The theme's tokens as GPUI colors. The theme and its side (light or dark) can change while the
// app runs; `set` swaps every token at once. Rendering only ever happens on the main thread, so
// the current theme is a thread local rather than state threaded through every render function.

use std::cell::Cell;
use std::sync::RwLock;

use gpui::{BoxShadow, FontStyle, FontWeight, HighlightStyle, Hsla, WindowAppearance, point, rgba};
use hyprspace_proto::Agent;
use hyprspace_proto::state::Scheme;
use hyprspace_syntax::Kind;
use hyprspace_theme::{Color, Theme, build};

use crate::zoom::px;

thread_local! {
    static CURRENT: Cell<Theme> = Cell::new(build("t3", true));
}

// terminals parse their output off the main thread, and answer color queries from there
static SHARED: RwLock<Option<Theme>> = RwLock::new(None);

/// The theme id taken from the background picture's main color.
pub const WALLPAPER: &str = "wallpaper";

/// Switches to theme `id` on its dark or light side, with blue and orange for added and removed
/// lines when `blue_orange`. `picture` is the background's hue and chroma, for `WALLPAPER`.
pub fn set(id: &str, dark: bool, blue_orange: bool, picture: Option<(f32, f32)>, glass: bool) {
    let mut t = match picture.filter(|_| id == WALLPAPER) {
        Some((hue, chroma)) => hyprspace_theme::from_color(hue as f64, chroma as f64, dark),
        None => build(id, dark),
    };
    if blue_orange {
        (t.diff_add, t.diff_del) = hyprspace_theme::blue_orange(dark);
    }
    if glass {
        t = hyprspace_theme::glass(t);
    }
    CURRENT.with(|c| c.set(t));
    *SHARED.write().unwrap_or_else(|e| e.into_inner()) = Some(t);
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

pub fn shared_theme() -> Theme {
    SHARED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or_else(|| build("t3", true))
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

/// A project's tag colors, fill and lettering, on the side showing now.
/// A project's tag fill and ink: at its picked hue, else the one its name gives.
pub fn tag(name: &str, hue: Option<u16>) -> (Hsla, Hsla) {
    let hue = hue.map_or_else(|| hyprspace_theme::name_hue(name), f64::from);
    let (fill, ink) = hyprspace_theme::tag(hue, theme().dark);
    (hsla(fill), hsla(ink))
}

pub fn rainbow(hue: f32) -> Hsla {
    hsla(hyprspace_theme::rainbow(hue as f64, theme().dark))
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

pub fn lift() -> Vec<BoxShadow> {
    let t = theme();
    vec![
        BoxShadow {
            color: hsla(t.shadow).opacity(if t.dark { 0.5 } else { 0.45 }),
            offset: point(px(0.), px(6.)),
            blur_radius: px(22.),
            spread_radius: px(0.),
            inset: false,
        },
        BoxShadow {
            color: ink(if t.dark { 0.05 } else { 0. }),
            offset: point(px(0.), px(1.)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_theme_it_is_set_to() {
        set("t3", true, false, None, false);
        assert!((border2().a - 0x1a as f32 / 255.0).abs() < 0.01);
        assert_eq!(bg().a, 1.0);
        let dark_text = text1();
        set("t3", false, false, None, false);
        assert_ne!(text1(), dark_text);
        set("iris", true, false, None, false);
        assert_ne!(accent(), hsla(build("t3", true).accent));
        set("t3", true, false, None, false);
    }

    #[test]
    fn the_wallpaper_theme_follows_the_picture_and_glass_lets_the_desktop_through() {
        set(WALLPAPER, true, false, Some((150.0, 0.15)), false);
        assert_eq!(
            accent(),
            hsla(hyprspace_theme::from_color(150.0, 0.15, true).accent)
        );
        set(WALLPAPER, true, false, None, false);
        assert_eq!(accent(), hsla(build("t3", true).accent));
        set("t3", true, false, None, true);
        assert!(bg().a < 1.0 && surface2().a == 1.0);
        set("t3", true, false, None, false);
    }

    #[test]
    fn the_scheme_overrides_the_system() {
        assert!(dark(Scheme::Dark, WindowAppearance::Light));
        assert!(!dark(Scheme::Light, WindowAppearance::Dark));
        assert!(dark(Scheme::System, WindowAppearance::Dark));
        assert!(!dark(Scheme::System, WindowAppearance::Light));
    }
}
