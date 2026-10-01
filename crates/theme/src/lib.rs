//! Color tokens and the terminal palette, as plain data. The UI turns a `Color` into whatever
//! its renderer wants, so this crate stays free of GPUI.
//!
//! Only the dark side exists so far, taken from `src/styles/tokens.css`. The light side, derived
//! from one hue the way `src/themes.ts` does it, comes with the UI shell.

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: Color,
    pub term_bg: Color,
    /// Cards, inputs, raised fills.
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub cursor: Color,
    pub term_fg: Color,
    /// The 16 ANSI colors, xterm's defaults.
    pub ansi: [Color; 16],
}

impl Theme {
    pub const fn dark() -> Self {
        Self {
            bg: Color::rgb(0x161616),
            term_bg: Color::rgb(0x161616),
            surface: Color::rgb(0x1e1e1e),
            border: Color::rgba(0xffffff1a),
            text: Color::rgb(0xf5f5f5),
            muted: Color::rgb(0xa1a1a1),
            cursor: Color::rgba(0xf5f5f5aa),
            term_fg: Color::rgb(0xe5e5e5),
            ansi: XTERM,
        }
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

const XTERM: [Color; 16] = {
    let hex = [
        0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
        0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
    ];
    let mut out = [Color(0); 16];
    let mut i = 0;
    while i < 16 {
        out[i] = Color::rgb(hex[i]);
        i += 1;
    }
    out
};

/// The terminal font. Cascadia ships with Windows 11; Menlo with every macOS.
pub const MONO: &str = if cfg!(windows) {
    "Cascadia Mono"
} else {
    "Menlo"
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_is_opaque_and_rgba_keeps_alpha() {
        assert_eq!(Color::rgb(0x161616), Color(0x161616ff));
        assert_eq!(Color::rgba(0xffffff1a).alpha(), 0x1a);
    }

    #[test]
    fn indexed_covers_ansi_cube_and_ramp() {
        let t = Theme::dark();
        assert_eq!(t.indexed(1), Color::rgb(0xcd3131));
        assert_eq!(t.indexed(16), Color::rgb(0x000000));
        assert_eq!(t.indexed(196), Color::rgb(0xff0000));
        assert_eq!(t.indexed(231), Color::rgb(0xffffff));
        assert_eq!(t.indexed(232), Color::rgb(0x080808));
        assert_eq!(t.indexed(255), Color::rgb(0xeeeeee));
    }
}
