// The theme's tokens as GPUI colors. The theme and its side (light or dark) can change while the
// app runs; `set` swaps every token at once. Rendering only ever happens on the main thread, so
// the current theme is a thread local rather than state threaded through every render function.

use std::cell::Cell;

use gpui::{BoxShadow, Hsla, point, px, rgba};
use hyprspace_proto::Agent;
use hyprspace_theme::{Color, Theme, build};

thread_local! {
    static CURRENT: Cell<Theme> = Cell::new(build("t3", true));
}

/// Switches to theme `id` on its dark or light side.
pub fn set(id: &str, dark: bool) {
    CURRENT.with(|c| c.set(build(id, dark)));
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
        set("t3", true);
        assert!((border2().a - 0x1a as f32 / 255.0).abs() < 0.01);
        assert_eq!(bg().a, 1.0);
        let dark_text = text1();
        set("t3", false);
        assert_ne!(text1(), dark_text);
        set("iris", true);
        assert_ne!(accent(), hsla(build("t3", true).accent));
        set("t3", true);
    }
}
