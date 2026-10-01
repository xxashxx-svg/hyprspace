// Spike colors, taken from the dark side of src/styles/tokens.css. The real theme crate (light and
// dark from one hue) comes with the UI shell.

use gpui::{Hsla, rgb, rgba};

use crate::term::CellColor;

pub const MONO: &str = if cfg!(windows) {
    "Cascadia Mono"
} else {
    "Menlo"
};

pub fn bg() -> Hsla {
    rgb(0x161616).into()
}

pub fn term_bg() -> Hsla {
    bg()
}

pub fn surface() -> Hsla {
    rgb(0x1e1e1e).into()
}

pub fn border() -> Hsla {
    rgba(0xffffff1a).into()
}

pub fn text() -> Hsla {
    rgb(0xf5f5f5).into()
}

pub fn muted() -> Hsla {
    rgb(0xa1a1a1).into()
}

pub fn cursor() -> Hsla {
    rgba(0xf5f5f5aa).into()
}

// xterm's default 16 colors
const ANSI: [u32; 16] = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
    0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
];

pub fn cell(color: CellColor) -> Hsla {
    match color {
        CellColor::Foreground => rgb(0xe5e5e5).into(),
        CellColor::Background => term_bg(),
        CellColor::Rgb(r, g, b) => rgb(u32::from_be_bytes([0, r, g, b])).into(),
        CellColor::Indexed(ix) => rgb(indexed(ix)).into(),
    }
}

fn indexed(ix: u8) -> u32 {
    match ix {
        0..=15 => ANSI[ix as usize],
        16..=231 => {
            let i = ix - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v as u32 * 40 };
            (level(i / 36) << 16) | (level(i / 6 % 6) << 8) | level(i % 6)
        }
        _ => {
            let v = 8 + (ix - 232) as u32 * 10;
            (v << 16) | (v << 8) | v
        }
    }
}
