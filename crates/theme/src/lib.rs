//! Color tokens and the terminal palette, as plain data. The UI turns a `Color` into whatever
//! its renderer wants, so this crate stays free of GPUI.
//!
//! The tokens are the Tauri app's (`src/styles/tokens.css`), and the themes are its
//! `src/themes.ts`: each is one hue, and both its light and dark side are derived from that hue
//! in oklch. Lines and washes are the "ink" color at low alpha (white on the dark side, black on
//! the light side), as CLAUDE.md rule 3 asks.

mod oklch;
mod syntax;
mod themes;

pub use oklch::{oklch, oklch_a};
pub use syntax::Syntax;
pub use themes::{THEMES, ThemeInfo, blue_orange, build};

/// 0xRRGGBBAA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgb(hex: u32) -> Self {
        Self((hex << 8) | 0xff)
    }

    pub const fn rgba(hex: u32) -> Self {
        Self(hex)
    }

    pub const fn alpha(self) -> u8 {
        (self.0 & 0xff) as u8
    }

    /// The same color at `alpha` (0-255).
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self((self.0 & 0xffff_ff00) | alpha as u32)
    }

    /// The same color at `a` (0.0-1.0), the way CSS writes `rgba(var(--ink), 0.1)`.
    pub fn a(self, a: f32) -> Self {
        self.with_alpha((a.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// Every token the UI draws with. Names follow tokens.css.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
    pub bg: Color,
    /// Chrome (sidebar, headers): flat on the background, split only by borders.
    pub surface1: Color,
    /// Cards, inputs, raised fills.
    pub surface2: Color,
    /// Menus, pressed and selected fills.
    pub surface3: Color,
    pub term_bg: Color,
    /// The one primary button and the focus ring.
    pub accent: Color,
    pub accent_hover: Color,
    /// Neutral hover and active washes.
    pub accent_dim: Color,
    pub on_accent: Color,
    /// Link text: the accent, lifted on the dark side so it reads on the background.
    pub link: Color,
    pub text1: Color,
    pub text2: Color,
    pub text3: Color,
    /// Shell dividers.
    pub border0: Color,
    pub border1: Color,
    /// Inputs and cards.
    pub border2: Color,
    /// White on the dark side, black on the light side. Lines and washes are this at low alpha.
    pub ink: Color,
    /// What drop shadows are drawn in.
    pub shadow: Color,
    pub idle: Color,
    pub busy: Color,
    pub waiting: Color,
    pub ok: Color,
    pub error: Color,
    pub diff_add: Color,
    pub diff_del: Color,
    pub term_fg: Color,
    pub cursor: Color,
    pub selection: Color,
    /// The 16 ANSI colors.
    pub ansi: [Color; 16],
    /// Code colors, for highlighted code blocks and files.
    pub syntax: Syntax,
}

impl Theme {
    /// The default theme's dark side.
    pub fn dark() -> Self {
        build("t3", true)
    }

    /// The default theme's light side.
    pub fn light() -> Self {
        build("t3", false)
    }

    /// The ink at alpha `a`: `rgba(var(--ink), a)`.
    pub fn ink(&self, a: f32) -> Color {
        self.ink.a(a)
    }

    /// One of the 256 indexed terminal colors: 0-15 ANSI, 16-231 the color cube, 232-255 the
    /// grayscale ramp.
    pub fn indexed(&self, ix: u8) -> Color {
        match ix {
            0..=15 => self.ansi[ix as usize],
            16..=231 => {
                let i = ix - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v as u32 * 40 };
                Color::rgb((level(i / 36) << 16) | (level(i / 6 % 6) << 8) | level(i % 6))
            }
            _ => {
                let v = 8 + (ix - 232) as u32 * 10;
                Color::rgb((v << 16) | (v << 8) | v)
            }
        }
    }
}

/// Each agent's signature color and the second stop of its gradient (src/lib/brand.ts), for
/// tinting controls that belong to that agent.
pub fn brand(agent: &str) -> (Color, Color) {
    match agent {
        "claude" => (Color::rgb(0xd97757), Color::rgb(0xe8a07e)),
        "codex" => (Color::rgb(0x10a37f), Color::rgb(0x5ed3b3)),
        "gemini" => (Color::rgb(0x4c8bf5), Color::rgb(0x9b72cb)),
        _ => (Color::rgb(0x8f8f8f), Color::rgb(0xe5e5e5)),
    }
}

/// A project's tag, after T3 Code's: the fill and lettering of the small square that carries its
/// initials. The hue comes from the project's name, so a project keeps its color from run to run,
/// at a fixed lightness for each side so every tag reads the same.
pub fn tag(name: &str, dark: bool) -> (Color, Color) {
    // FNV-1a: stable across runs and builds, unlike the standard hasher
    let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let hue = (h % 360) as f64;
    if dark {
        (oklch(0.36, 0.075, hue), oklch(0.88, 0.1, hue))
    } else {
        (oklch(0.9, 0.05, hue), oklch(0.42, 0.12, hue))
    }
}

/// The UI font, zeron's. The UI bundles it (crates/ui/assets/fonts).
pub const SANS: &str = "Geist";

/// Code, paths and numbers in the UI, zeron's. Bundled with the UI like `SANS`.
pub const MONO: &str = "Geist Mono";

/// The terminal font: the Nerd Font build, so agent status lines draw their glyphs. Bundled with
/// the UI like `SANS`.
pub const TERM: &str = "JetBrainsMono Nerd Font Mono";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_is_opaque_and_rgba_keeps_alpha() {
        assert_eq!(Color::rgb(0x161616), Color(0x161616ff));
        assert_eq!(Color::rgba(0xffffff1a).alpha(), 0x1a);
        assert_eq!(Color::rgb(0x123456).with_alpha(0x80), Color(0x12345680));
        assert_eq!(Color::rgb(0xffffff).a(0.1).alpha(), 26);
    }

    #[test]
    fn indexed_covers_ansi_cube_and_ramp() {
        let t = Theme::dark();
        assert_eq!(t.indexed(16), Color::rgb(0x000000));
        assert_eq!(t.indexed(196), Color::rgb(0xff0000));
        assert_eq!(t.indexed(231), Color::rgb(0xffffff));
        assert_eq!(t.indexed(232), Color::rgb(0x080808));
        assert_eq!(t.indexed(255), Color::rgb(0xeeeeee));
    }

    #[test]
    fn each_side_draws_lines_in_its_own_ink() {
        let (d, l) = (Theme::dark(), Theme::light());
        assert_eq!(d.border1.0 >> 8, 0xffffff);
        assert_eq!(l.border1.0 >> 8, 0x000000);
        assert_eq!(d.ink(0.1), d.border2);
        assert!(d.dark && !l.dark);
    }
}
