// Block elements and box-drawing lines drawn as rectangles instead of font glyphs, the way the
// Tauri app's xterm did with `customGlyphs`. A font's glyphs stop short of the cell when the line
// height adds leading, so stacked blocks (Claude's logo, progress bars) show seams and box sides
// break between rows. Rectangles fill the cell exactly.

/// A rectangle in fractions of the cell, with the share of the foreground it is painted in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub alpha: f32,
}

const fn part(x: f32, y: f32, w: f32, h: f32) -> Part {
    Part {
        x,
        y,
        w,
        h,
        alpha: 1.0,
    }
}

/// U+2580 to U+259F.
fn block(ch: char) -> Option<Vec<Part>> {
    let eighth = |n: u32| n as f32 / 8.0;
    let shade = |alpha| {
        vec![Part {
            alpha,
            ..part(0., 0., 1., 1.)
        }]
    };
    let (l, r, t) = (
        part(0., 0., 0.5, 1.),
        part(0.5, 0., 0.5, 1.),
        part(0., 0., 1., 0.5),
    );
    let quad = |q: &[(f32, f32)]| q.iter().map(|&(x, y)| part(x, y, 0.5, 0.5)).collect();
    let (tl, tr, bl, br) = ((0., 0.), (0.5, 0.), (0., 0.5), (0.5, 0.5));
    Some(match ch {
        '\u{2580}' => vec![t],
        // lower one eighth to seven eighths, then the full block
        '\u{2581}'..='\u{2588}' => {
            let n = ch as u32 - 0x2580;
            vec![part(0., 1. - eighth(n), 1., eighth(n))]
        }
        // left seven eighths down to left one eighth
        '\u{2589}'..='\u{258f}' => {
            let n = 0x2590 - ch as u32;
            vec![part(0., 0., eighth(n), 1.)]
        }
        '\u{2590}' => vec![r],
        '\u{2591}' => shade(0.25),
        '\u{2592}' => shade(0.5),
        '\u{2593}' => shade(0.75),
        '\u{2594}' => vec![part(0., 0., 1., eighth(1))],
        '\u{2595}' => vec![part(1. - eighth(1), 0., eighth(1), 1.)],
        '\u{2596}' => quad(&[bl]),
        '\u{2597}' => quad(&[br]),
        '\u{2598}' => quad(&[tl]),
        '\u{2599}' => vec![l, quad(&[br])[0]],
        '\u{259a}' => quad(&[tl, br]),
        '\u{259b}' => vec![t, quad(&[bl])[0]],
        '\u{259c}' => vec![t, quad(&[br])[0]],
        '\u{259d}' => quad(&[tr]),
        '\u{259e}' => quad(&[tr, bl]),
        '\u{259f}' => vec![r, quad(&[bl])[0]],
        _ => return None,
    })
}

/// How heavy each arm of a box-drawing char is: up, right, down, left. 0 none, 1 light, 2 heavy.
fn arms(ch: char) -> Option<[u8; 4]> {
    Some(match ch {
        '─' => [0, 1, 0, 1],
        '━' => [0, 2, 0, 2],
        '│' => [1, 0, 1, 0],
        '┃' => [2, 0, 2, 0],
        '┌' => [0, 1, 1, 0],
        '┐' => [0, 0, 1, 1],
        '└' => [1, 1, 0, 0],
        '┘' => [1, 0, 0, 1],
        '┏' => [0, 2, 2, 0],
        '┓' => [0, 0, 2, 2],
        '┗' => [2, 2, 0, 0],
        '┛' => [2, 0, 0, 2],
        '├' => [1, 1, 1, 0],
        '┤' => [1, 0, 1, 1],
        '┬' => [0, 1, 1, 1],
        '┴' => [1, 1, 0, 1],
        '┼' => [1, 1, 1, 1],
        '┣' => [2, 2, 2, 0],
        '┫' => [2, 0, 2, 2],
        '┳' => [0, 2, 2, 2],
        '┻' => [2, 2, 0, 2],
        '╋' => [2, 2, 2, 2],
        '╴' => [0, 0, 0, 1],
        '╵' => [1, 0, 0, 0],
        '╶' => [0, 1, 0, 0],
        '╷' => [0, 0, 1, 0],
        _ => return None,
    })
}

/// The rectangles for `ch` in a cell `w` by `h` pixels, or None when the font should draw it.
/// Lines are whole pixels thick and sit on whole pixels, so neighbours join without a seam.
pub fn rects(ch: char, w: f32, h: f32) -> Option<Vec<Part>> {
    if let Some(parts) = block(ch) {
        return Some(parts);
    }
    let [up, right, down, left] = arms(ch)?;
    let thick = |weight: u8| -> f32 { if weight == 2 { 2.0 } else { 1.0 } };
    let cx = (w / 2.0).floor();
    let cy = (h / 2.0).floor();
    let mut out = Vec::new();
    // in pixels first, then as fractions of the cell
    let mut px = |x: f32, y: f32, pw: f32, ph: f32| out.push(part(x / w, y / h, pw / w, ph / h));
    if up > 0 {
        let t = thick(up);
        px(cx - (t / 2.0).floor(), 0.0, t, cy + t);
    }
    if down > 0 {
        let t = thick(down);
        px(cx - (t / 2.0).floor(), cy, t, h - cy);
    }
    if left > 0 {
        let t = thick(left);
        px(0.0, cy - (t / 2.0).floor(), cx + t, t);
    }
    if right > 0 {
        let t = thick(right);
        px(cx, cy - (t / 2.0).floor(), w - cx, t);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_fill_their_share_of_the_cell() {
        assert_eq!(rects('█', 8., 18.), Some(vec![part(0., 0., 1., 1.)]));
        assert_eq!(rects('▀', 8., 18.), Some(vec![part(0., 0., 1., 0.5)]));
        assert_eq!(rects('▁', 8., 18.), Some(vec![part(0., 0.875, 1., 0.125)]));
        assert_eq!(rects('▌', 8., 18.), Some(vec![part(0., 0., 0.5, 1.)]));
        assert_eq!(rects('▒', 8., 18.).unwrap()[0].alpha, 0.5);
        assert_eq!(rects('▛', 8., 18.).unwrap().len(), 2);
    }

    #[test]
    fn lines_run_to_the_cell_edges_so_rows_join() {
        let v = rects('│', 8., 18.).unwrap();
        let top = v.iter().map(|p| p.y).fold(1.0, f32::min);
        let bottom = v.iter().map(|p| p.y + p.h).fold(0.0, f32::max);
        assert_eq!((top, bottom), (0.0, 1.0));
        let h = rects('─', 8., 18.).unwrap();
        let left = h.iter().map(|p| p.x).fold(1.0, f32::min);
        let right = h.iter().map(|p| p.x + p.w).fold(0.0, f32::max);
        assert_eq!((left, right), (0.0, 1.0));
        assert_eq!(rects('a', 8., 18.), None);
        assert_eq!(rects('┌', 8., 18.).unwrap().len(), 2);
        // rounded corners stay with the font, which draws the curve
        assert_eq!(rects('╭', 8., 18.), None);
    }
}
