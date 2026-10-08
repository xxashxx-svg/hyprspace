// A QR code drawn module by module, dark on light. Scanners want that way round, so the code
// takes its colors from the light side of the theme even when the app is dark.

use gpui::{AnyElement, Bounds, Hsla, IntoElement, canvas, div, fill, point, prelude::*, px, size};

/// Blank modules around the code, which scanners need to find it.
const QUIET: usize = 3;

pub fn qr(text: &str, side: f32, ink: Hsla, plate: Hsla) -> AnyElement {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else {
        return div().into_any_element();
    };
    let n = code.width();
    let dark: Vec<bool> = code
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let total = (n + 2 * QUIET) as f32;
            let side = bounds.size.width.min(bounds.size.height) / px(1.0);
            // whole pixels per module, so rows meet without seams
            let m = (side / total).floor().max(1.0);
            let pad = (side - m * total) / 2.0;
            window.paint_quad(fill(bounds, plate).corner_radii(px(8.)));
            let at = |x: usize, y: usize| {
                point(
                    bounds.origin.x + px(pad + (x + QUIET) as f32 * m),
                    bounds.origin.y + px(pad + (y + QUIET) as f32 * m),
                )
            };
            // a row's dark runs as one quad each, so edges between modules don't show
            for y in 0..n {
                let mut x = 0;
                while x < n {
                    if !dark[y * n + x] {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < n && dark[y * n + x] {
                        x += 1;
                    }
                    let b = Bounds::new(at(start, y), size(px((x - start) as f32 * m), px(m)));
                    window.paint_quad(fill(b, ink));
                }
            }
        },
    )
    .size(px(side))
    .flex_none()
    .into_any_element()
}
