// Code colors, one set per theme and side. Each category sits at a fixed turn of the hue wheel
// from the theme's own hue, so a theme's code reads as part of it (Ember's keywords are amber,
// Grove's teal) while the categories stay far enough apart to tell at a glance. Every category
// shares one lightness and chroma per side, which keeps them equally readable: about 8:1 on the
// dark side and 5:1 or better on the light side.

use crate::Color;
use crate::oklch::oklch;

/// The colors code is drawn in. Comments are meant to be drawn in italics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syntax {
    /// Names, properties and anything else no category claims.
    pub plain: Color,
    pub keyword: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub function: Color,
    pub ty: Color,
    /// Booleans, named constants, escapes.
    pub constant: Color,
    pub punctuation: Color,
}

/// Degrees from the theme's hue. On the default theme (hue 264) this lands on violet keywords,
/// blue functions, cyan types, green strings, pink constants and orange numbers.
const KEYWORD: f64 = 36.0;
const FUNCTION: f64 = -14.0;
const TYPE: f64 = -64.0;
const STRING: f64 = -119.0;
const CONSTANT: f64 = 100.0;
const NUMBER: f64 = 156.0;

pub(crate) fn derive(hue: f64, tint: f64, dark: bool) -> Syntax {
    let (l, c) = if dark { (0.8, 0.11) } else { (0.49, 0.12) };
    let at = |turn: f64| oklch(l, c, (hue + turn).rem_euclid(360.0));
    let grey = |dark_l: f64, light_l: f64| oklch(if dark { dark_l } else { light_l }, tint, hue);
    Syntax {
        plain: grey(0.92, 0.25),
        keyword: at(KEYWORD),
        string: at(STRING),
        number: at(NUMBER),
        comment: grey(0.64, 0.54),
        function: at(FUNCTION),
        ty: at(TYPE),
        constant: at(CONSTANT),
        punctuation: grey(0.72, 0.46),
    }
}

#[cfg(test)]
mod tests {
    use crate::{THEMES, build};

    /// WCAG contrast between two opaque colors.
    fn contrast(a: crate::Color, b: crate::Color) -> f64 {
        let lum = |c: crate::Color| {
            let ch = |shift: u32| {
                let v = ((c.0 >> shift) & 0xff) as f64 / 255.0;
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * ch(24) + 0.7152 * ch(16) + 0.0722 * ch(8)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn code_reads_on_every_theme_and_side() {
        for t in THEMES {
            for dark in [true, false] {
                let theme = build(t.id, dark);
                let s = theme.syntax;
                // code blocks sit on surface2; the viewer draws on bg
                for ground in [theme.bg, theme.surface2] {
                    for (name, c) in [
                        ("plain", s.plain),
                        ("keyword", s.keyword),
                        ("string", s.string),
                        ("number", s.number),
                        ("function", s.function),
                        ("type", s.ty),
                        ("constant", s.constant),
                        ("punctuation", s.punctuation),
                        ("comment", s.comment),
                    ] {
                        let got = contrast(c, ground);
                        assert!(got >= 4.5, "{} {dark} {name}: {got:.2}", t.id);
                    }
                }
            }
        }
    }

    #[test]
    fn each_theme_colors_code_its_own_way() {
        let (a, b) = (build("t3", true).syntax, build("ember", true).syntax);
        assert_ne!(a.keyword, b.keyword);
        assert_ne!(build("t3", false).syntax.keyword, a.keyword);
    }
}
