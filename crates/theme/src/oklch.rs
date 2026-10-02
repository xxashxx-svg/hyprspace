// oklch to sRGB, so the themes can be written the way src/themes.ts writes them: one hue, with
// every surface at a fixed perceived lightness. Formulas from Björn Ottosson's OKLab post.

use crate::Color;

/// `l` 0..1, `c` chroma (0 is grey), `h` hue in degrees. Out-of-gamut channels are clipped.
pub fn oklch(l: f64, c: f64, h: f64) -> Color {
    oklch_a(l, c, h, 1.0)
}

pub fn oklch_a(l: f64, c: f64, h: f64, alpha: f64) -> Color {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3;
    let byte = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        let srgb = if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u32
    };
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    Color((byte(r) << 24) | (byte(g) << 16) | (byte(bl) << 8) | a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greys_and_known_colors_land_where_css_puts_them() {
        assert_eq!(oklch(0.0, 0.0, 0.0), Color::rgb(0x000000));
        assert_eq!(oklch(1.0, 0.0, 0.0), Color::rgb(0xffffff));
        // tokens.css's default accent, oklch(0.488 0.217 264), is about #1d4ed8
        let accent = oklch(0.488, 0.217, 264.0).0 >> 8;
        let (r, g, b) = (accent >> 16, (accent >> 8) & 0xff, accent & 0xff);
        assert!(
            r < 0x30 && (0x40..0x60).contains(&g) && b > 0xd0,
            "{accent:06x}"
        );
        assert_eq!(oklch_a(0.5, 0.0, 0.0, 0.5).alpha(), 128);
    }
}
