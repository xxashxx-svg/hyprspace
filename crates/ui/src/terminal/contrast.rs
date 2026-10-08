// A floor on how faint terminal text may be against what's behind it. Programs pick their own
// colors for one kind of background; Claude set to its dark theme paints pale lavender and
// pastel green that vanish on a light one. Text under the floor moves toward black on a light
// background, or white on a dark one, just far enough, keeping its hue.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{Hsla, Rgba};

/// WCAG's bar for body text.
const MIN: f32 = 4.5;

fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Rgba) -> f32 {
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}

fn ratio(a: Rgba, b: Rgba) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a,
    }
}

fn key(c: Rgba) -> u32 {
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    b(c.r) << 24 | b(c.g) << 16 | b(c.b) << 8 | b(c.a)
}

thread_local! {
    static SEEN: RefCell<HashMap<(u32, u32), Hsla>> = RefCell::new(HashMap::new());
}

/// `fg` as drawn over `bg`, moved only as far as it takes to read.
pub fn readable(fg: Hsla, bg: Hsla) -> Hsla {
    let (f, b) = (Rgba::from(fg), Rgba::from(bg));
    let k = (key(f), key(b));
    if let Some(c) = SEEN.with(|s| s.borrow().get(&k).copied()) {
        return c;
    }
    let out = if ratio(f, b) >= MIN {
        fg
    } else {
        let toward = if luminance(b) > 0.18 {
            Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: f.a,
            }
        } else {
            Rgba {
                r: 1.,
                g: 1.,
                b: 1.,
                a: f.a,
            }
        };
        let (mut lo, mut hi) = (0f32, 1f32);
        for _ in 0..12 {
            let mid = (lo + hi) / 2.;
            if ratio(mix(f, toward, mid), b) >= MIN {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Hsla::from(mix(f, toward, hi))
    };
    SEEN.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() > 4096 {
            s.clear();
        }
        s.insert(k, out);
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: u32) -> Hsla {
        Hsla::from(Rgba {
            r: ((hex >> 16) & 0xff) as f32 / 255.,
            g: ((hex >> 8) & 0xff) as f32 / 255.,
            b: (hex & 0xff) as f32 / 255.,
            a: 1.,
        })
    }

    #[test]
    fn pale_text_on_light_darkens_until_it_reads() {
        let bg = rgb(0xf7f7f7);
        let out = readable(rgb(0xb1b9f9), bg);
        assert!(ratio(Rgba::from(out), Rgba::from(bg)) >= MIN - 0.05);
        // still bluish, just darker
        let o = Rgba::from(out);
        assert!(o.b > o.r && o.b > o.g);
    }

    #[test]
    fn dark_text_on_dark_lightens_and_good_text_stays() {
        let bg = rgb(0x161616);
        let out = readable(rgb(0x1e2228), bg);
        assert!(ratio(Rgba::from(out), Rgba::from(bg)) >= MIN - 0.05);
        let fine = rgb(0xeeeeee);
        assert_eq!(readable(fine, bg), fine);
    }
}
