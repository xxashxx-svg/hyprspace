// The theme's tokens as GPUI colors. One theme for now: the dark side.

use gpui::{Hsla, rgba};
use hyprspace_theme::{Color, Theme};

pub const THEME: Theme = Theme::dark();

pub fn hsla(c: Color) -> Hsla {
    rgba(c.0).into()
}

pub fn bg() -> Hsla {
    hsla(THEME.bg)
}

pub fn surface() -> Hsla {
    hsla(THEME.surface)
}

pub fn border() -> Hsla {
    hsla(THEME.border)
}

pub fn text() -> Hsla {
    hsla(THEME.text)
}

pub fn muted() -> Hsla {
    hsla(THEME.muted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_alpha() {
        assert!((border().a - 0x1a as f32 / 255.0).abs() < 0.01);
        assert_eq!(bg().a, 1.0);
    }
}
