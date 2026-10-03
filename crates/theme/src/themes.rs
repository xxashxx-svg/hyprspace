// The six themes of src/themes.ts: a hue and a name, with both sides derived from the hue the
// same way, so they read as one family. Shared bits (status colors, ANSI) come from tokens.css.

use crate::oklch::{oklch, oklch_a};
use crate::syntax;
use crate::{Color, Theme};

pub struct ThemeInfo {
    /// Saved in the app state. "t3" stays the default's id: the Tauri app persisted it so.
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    hue: f64,
    /// Chroma of the surfaces: 0 is pure neutral, about 0.012 a visible cast.
    tint: f64,
    /// Accent lightness and chroma on each side.
    dark: (f64, f64),
    light: (f64, f64),
}

pub const THEMES: &[ThemeInfo] = &[
    ThemeInfo {
        id: "t3",
        name: "HyprSpace",
        blurb: "Neutral",
        hue: 264.0,
        tint: 0.0,
        dark: (0.488, 0.217),
        light: (0.45, 0.2),
    },
    ThemeInfo {
        id: "orchid",
        name: "Orchid",
        blurb: "Pink",
        hue: 350.0,
        tint: 0.012,
        dark: (0.62, 0.22),
        light: (0.5, 0.22),
    },
    ThemeInfo {
        id: "grove",
        name: "Grove",
        blurb: "Green",
        hue: 155.0,
        tint: 0.012,
        dark: (0.68, 0.15),
        light: (0.5, 0.14),
    },
    ThemeInfo {
        id: "ocean",
        name: "Ocean",
        blurb: "Blue",
        hue: 235.0,
        tint: 0.012,
        dark: (0.68, 0.15),
        light: (0.5, 0.17),
    },
    ThemeInfo {
        id: "ember",
        name: "Ember",
        blurb: "Orange",
        hue: 45.0,
        tint: 0.012,
        dark: (0.72, 0.16),
        light: (0.55, 0.17),
    },
    ThemeInfo {
        id: "iris",
        name: "Iris",
        blurb: "Violet",
        hue: 300.0,
        tint: 0.012,
        dark: (0.65, 0.2),
        light: (0.5, 0.2),
    },
];

const WHITE: Color = Color::rgb(0xffffff);
const BLACK: Color = Color::rgb(0x000000);

/// The theme `id` (the default for an unknown one), on its dark or light side.
/// Added and removed in blue and orange, for the setting that swaps out red and green.
pub fn blue_orange(dark: bool) -> (Color, Color) {
    if dark {
        (Color::rgb(0x3b9eff), Color::rgb(0xf5a14b))
    } else {
        (Color::rgb(0x0b6bcb), Color::rgb(0xc2620a))
    }
}

pub fn build(id: &str, dark: bool) -> Theme {
    let t = THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0]);
    let (hue, tint) = (t.hue, t.tint);
    // white text on a deep accent, near-black text on a bright one
    let on_accent = |l: f64| {
        if l >= 0.66 {
            oklch(0.2, 0.02, hue)
        } else {
            WHITE
        }
    };
    let status = Status::default();
    if dark {
        let (al, ac) = t.dark;
        let ink = WHITE;
        Theme {
            dark,
            bg: oklch(0.2, tint, hue),
            surface1: oklch(0.2, tint, hue),
            surface2: oklch(0.24, tint, hue),
            surface3: oklch(0.29, tint, hue),
            term_bg: oklch(0.2, tint, hue),
            accent: oklch(al, ac, hue),
            accent_hover: oklch(al + 0.06, ac, hue),
            accent_dim: ink.a(0.07),
            on_accent: on_accent(al),
            link: oklch(al.max(0.72), ac.min(0.14), hue),
            text1: oklch(0.97, tint / 2.0, hue),
            text2: oklch(0.72, tint / 2.0, hue),
            text3: oklch(0.56, tint / 2.0, hue),
            border0: ink.a(0.035),
            border1: ink.a(0.06),
            border2: ink.a(0.1),
            ink,
            shadow: BLACK.a(0.55),
            idle: status.idle,
            busy: status.busy,
            waiting: status.waiting,
            ok: status.ok,
            error: status.error,
            diff_add: Color::rgb(0x10b981),
            diff_del: Color::rgb(0xef4444),
            term_fg: oklch(0.95, tint / 2.0, hue),
            cursor: oklch(0.82, 0.1, hue),
            selection: oklch_a(0.82, 0.1, hue, 0.25),
            ansi: XTERM,
            syntax: syntax::derive(hue, tint, true),
        }
    } else {
        let (al, ac) = t.light;
        let ink = BLACK;
        Theme {
            dark,
            bg: oklch(0.975, tint, hue),
            surface1: oklch(0.975, tint, hue),
            surface2: oklch(0.995, tint / 2.0, hue),
            surface3: oklch(0.94, tint, hue),
            term_bg: oklch(0.975, tint, hue),
            accent: oklch(al, ac, hue),
            accent_hover: oklch(al - 0.06, ac, hue),
            accent_dim: ink.a(0.05),
            on_accent: on_accent(al),
            link: oklch(al, ac, hue),
            text1: oklch(0.22, tint, hue),
            text2: oklch(0.46, tint, hue),
            text3: oklch(0.6, tint, hue),
            border0: ink.a(0.05),
            border1: ink.a(0.08),
            border2: ink.a(0.13),
            ink,
            shadow: BLACK.a(0.16),
            idle: status.idle,
            busy: status.busy,
            waiting: status.waiting,
            ok: status.ok,
            error: status.error,
            diff_add: Color::rgb(0x059669),
            diff_del: Color::rgb(0xdc2626),
            term_fg: oklch(0.25, tint, hue),
            cursor: oklch(al, ac, hue),
            selection: oklch_a(al, ac, hue, 0.22),
            ansi: XTERM_LIGHT,
            syntax: syntax::derive(hue, tint, false),
        }
    }
}

/// tokens.css's status colors, the same on both sides.
struct Status {
    idle: Color,
    busy: Color,
    waiting: Color,
    ok: Color,
    error: Color,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            idle: Color::rgb(0x737373),
            busy: Color::rgb(0xf59e0b),
            waiting: Color::rgb(0x3b82f6),
            ok: Color::rgb(0x10b981),
            error: Color::rgb(0xef4444),
        }
    }
}

const fn palette(hex: [u32; 16]) -> [Color; 16] {
    let mut out = [Color(0); 16];
    let mut i = 0;
    while i < 16 {
        out[i] = Color::rgb(hex[i]);
        i += 1;
    }
    out
}

// The Tauri app's terminal palette on its default "adaptive" setting (src/terminal/
// createTerminal.ts): T3 Code's muted ANSI set on the dark side.
const XTERM: [Color; 16] = palette([
    0x181e26, 0xff7a8e, 0x86e795, 0xf4cd72, 0x89beff, 0xd0b0ff, 0x7ce8ed, 0xd2dae6, 0x6e7888,
    0xffa8b4, 0xb0f5ba, 0xffe095, 0xaed2ff, 0xe5cbff, 0xa7f4f7, 0xf4f7fc,
]);

// The same 16 slots on the light side, deep enough to read on a near-white page.
const XTERM_LIGHT: [Color; 16] = palette([
    0x282c34, 0xc4283c, 0x248444, 0xaa7400, 0x1e64d6, 0x923cba, 0x00869a, 0x78808c, 0x6e7682,
    0xd83c50, 0x2e9854, 0xba840a, 0x3278ec, 0xa650ce, 0x0a9ab0, 0x1e2228,
]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_matches_tokens_css() {
        let d = build("t3", true);
        assert_eq!(d.bg, Color::rgb(0x161616));
        assert_eq!(d.border2, Color::rgba(0xffffff1a));
        assert_eq!(d.on_accent, WHITE);
        let l = build("t3", false);
        assert_eq!(l.ink, BLACK);
        assert_eq!(build("nope", true), d);
    }

    #[test]
    fn every_theme_builds_both_sides() {
        for t in THEMES {
            let (d, l) = (build(t.id, true), build(t.id, false));
            assert!(d.dark && !l.dark);
            assert_ne!(d.accent, l.accent, "{}", t.id);
            // a tinted theme's surfaces lean toward its hue
            if t.tint > 0.0 {
                assert_ne!(d.bg, build("t3", true).bg, "{}", t.id);
            }
        }
        // a bright accent gets dark text on it
        assert_ne!(build("ember", true).on_accent, WHITE);
    }
}
