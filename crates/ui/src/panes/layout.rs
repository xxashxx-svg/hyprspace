// The pane layouts and their geometry, after the Tauri app's src/lib/grid.ts: curated presets for
// two to six panes, and for one or seven and up a balanced tiling that splits the panes into about
// sqrt(n) rows, the bigger rows on top, so seven is 4 + 3 and never 3 + 3 + 1.
//
// A layout is a grid of tracks; each pane covers a span of columns and rows. Track sizes are
// weights the user drags. A boundary that some pane spans across can't be dragged, because moving
// it would cut that pane in two.

use std::ops::Range;

/// One pane's place: column and row tracks, 0-based, end exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub cols: Range<usize>,
    pub rows: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub id: &'static str,
    pub label: &'static str,
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Col,
    Row,
}

/// A cell from CSS grid lines (1-based, end exclusive), so the presets read like grid.ts.
fn c(c0: usize, c1: usize, r0: usize, r1: usize) -> Cell {
    Cell {
        cols: c0 - 1..c1 - 1,
        rows: r0 - 1..r1 - 1,
    }
}

/// `cols` x `rows` equal cells, filled row by row.
fn even(id: &'static str, label: &'static str, cols: usize, rows: usize) -> Layout {
    let cells = (0..rows)
        .flat_map(|r| (0..cols).map(move |col| (col, r)))
        .map(|(col, r)| Cell {
            cols: col..col + 1,
            rows: r..r + 1,
        })
        .collect();
    Layout {
        id,
        label,
        cols,
        rows,
        cells,
    }
}

fn shaped(
    id: &'static str,
    label: &'static str,
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
) -> Layout {
    Layout {
        id,
        label,
        cols,
        rows,
        cells,
    }
}

/// The layouts on offer for `n` panes, the default first. Empty when there is no choice.
pub fn presets(n: usize) -> Vec<Layout> {
    match n {
        2 => vec![
            even("cols", "Side by side", 2, 1),
            even("rows", "Stacked", 1, 2),
        ],
        3 => vec![
            even("cols", "Columns", 3, 1),
            shaped(
                "2top",
                "2 top, 1 bottom",
                2,
                2,
                vec![c(1, 2, 1, 2), c(2, 3, 1, 2), c(1, 3, 2, 3)],
            ),
            shaped(
                "1top",
                "1 top, 2 bottom",
                2,
                2,
                vec![c(1, 3, 1, 2), c(1, 2, 2, 3), c(2, 3, 2, 3)],
            ),
            shaped(
                "1left",
                "1 left, 2 right",
                2,
                2,
                vec![c(1, 2, 1, 3), c(2, 3, 1, 2), c(2, 3, 2, 3)],
            ),
            shaped(
                "1right",
                "2 left, 1 right",
                2,
                2,
                vec![c(1, 2, 1, 2), c(1, 2, 2, 3), c(2, 3, 1, 3)],
            ),
            even("rows", "Rows", 1, 3),
        ],
        4 => vec![
            even("grid", "2 by 2", 2, 2),
            even("cols", "Columns", 4, 1),
            even("rows", "Rows", 1, 4),
            shaped(
                "1left",
                "1 left, 3 right",
                2,
                3,
                vec![c(1, 2, 1, 4), c(2, 3, 1, 2), c(2, 3, 2, 3), c(2, 3, 3, 4)],
            ),
            shaped(
                "1top",
                "1 top, 3 bottom",
                3,
                2,
                vec![c(1, 4, 1, 2), c(1, 2, 2, 3), c(2, 3, 2, 3), c(3, 4, 2, 3)],
            ),
        ],
        5 => vec![
            shaped(
                "auto",
                "3 top, 2 bottom",
                6,
                2,
                vec![
                    c(1, 3, 1, 2),
                    c(3, 5, 1, 2),
                    c(5, 7, 1, 2),
                    c(1, 4, 2, 3),
                    c(4, 7, 2, 3),
                ],
            ),
            shaped(
                "2top",
                "2 top, 3 bottom",
                6,
                2,
                vec![
                    c(1, 4, 1, 2),
                    c(4, 7, 1, 2),
                    c(1, 3, 2, 3),
                    c(3, 5, 2, 3),
                    c(5, 7, 2, 3),
                ],
            ),
            shaped(
                "1left",
                "1 left, 4 right",
                3,
                2,
                vec![
                    c(1, 2, 1, 3),
                    c(2, 3, 1, 2),
                    c(3, 4, 1, 2),
                    c(2, 3, 2, 3),
                    c(3, 4, 2, 3),
                ],
            ),
            even("cols", "Columns", 5, 1),
        ],
        6 => vec![
            even("grid", "3 by 2", 3, 2),
            even("grid2", "2 by 3", 2, 3),
            even("cols", "Columns", 6, 1),
        ],
        _ => vec![],
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The balanced tiling for any count.
fn auto(n: usize) -> Layout {
    let n = n.max(1);
    let rows = (n as f64).sqrt().floor().max(1.0) as usize;
    let sizes: Vec<usize> = (0..rows)
        .map(|r| n / rows + usize::from(r < n % rows))
        .collect();
    let cols = sizes.iter().fold(1, |acc, &s| acc / gcd(acc, s) * s);
    let mut cells = Vec::with_capacity(n);
    for (r, &size) in sizes.iter().enumerate() {
        let span = cols / size;
        cells.extend((0..size).map(|k| Cell {
            cols: k * span..(k + 1) * span,
            rows: r..r + 1,
        }));
    }
    Layout {
        id: "auto",
        label: "Balanced",
        cols,
        rows,
        cells,
    }
}

/// The layout for `n` panes: the picked preset when it exists, else the default.
pub fn resolve(n: usize, picked: Option<&str>) -> Layout {
    let mut all = presets(n);
    if all.is_empty() {
        return auto(n);
    }
    let ix = picked
        .and_then(|id| all.iter().position(|l| l.id == id))
        .unwrap_or(0);
    all.swap_remove(ix)
}

/// The key a layout's dragged sizes are saved under.
pub fn key(n: usize, layout: &Layout) -> String {
    format!("{n}:{}", layout.id)
}

impl Layout {
    pub fn tracks(&self, axis: Axis) -> usize {
        match axis {
            Axis::Col => self.cols,
            Axis::Row => self.rows,
        }
    }

    /// The boundaries (1..tracks) on `axis` a user may drag: those no cell spans across.
    pub fn resizable(&self, axis: Axis) -> Vec<usize> {
        (1..self.tracks(axis))
            .filter(|&b| {
                !self.cells.iter().any(|c| {
                    let span = match axis {
                        Axis::Col => &c.cols,
                        Axis::Row => &c.rows,
                    };
                    span.start < b && span.end > b
                })
            })
            .collect()
    }

    /// Saved weights when they fit this layout, else equal ones.
    pub fn weights(&self, axis: Axis, saved: &[f32]) -> Vec<f32> {
        let n = self.tracks(axis);
        if saved.len() == n && saved.iter().all(|w| *w > 0.0) {
            saved.to_vec()
        } else {
            vec![1.0; n]
        }
    }
}

/// Where boundary `b` sits, as a fraction of the whole: the share of the tracks before it.
pub fn edge(weights: &[f32], b: usize) -> f32 {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    weights[..b.min(weights.len())].iter().sum::<f32>() / total
}

/// Moves boundary `b` by `delta`, a fraction of the whole. The two tracks beside it trade size
/// and the rest stay put; neither may shrink below 12% of the whole, as in the Tauri app.
pub fn drag(weights: &[f32], b: usize, delta: f32) -> Vec<f32> {
    let mut out = weights.to_vec();
    if b == 0 || b >= weights.len() {
        return out;
    }
    let total: f32 = weights.iter().sum();
    let min = total * 0.12;
    let pair = weights[b - 1] + weights[b];
    let a = (weights[b - 1] + delta * total).clamp(min, (pair - min).max(min));
    out[b - 1] = a;
    out[b] = pair - a;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_tiles_bigger_rows_on_top() {
        let l = auto(7);
        assert_eq!((l.cols, l.rows), (12, 2));
        let first_row = l.cells.iter().filter(|c| c.rows.start == 0).count();
        assert_eq!(first_row, 4);
        assert_eq!(l.cells[4].cols, 0..4);
        let one = auto(1);
        assert_eq!((one.cols, one.rows, one.cells.len()), (1, 1, 1));
        assert_eq!(auto(10).cells.len(), 10);
    }

    #[test]
    fn every_preset_covers_its_grid_once() {
        for n in 2..=6 {
            for l in presets(n) {
                assert_eq!(l.cells.len(), n, "{n} {}", l.id);
                let mut seen = vec![0; l.cols * l.rows];
                for c in &l.cells {
                    for r in c.rows.clone() {
                        for col in c.cols.clone() {
                            seen[r * l.cols + col] += 1;
                        }
                    }
                }
                assert!(seen.iter().all(|&s| s == 1), "{n} {}", l.id);
            }
        }
    }

    #[test]
    fn picks_fall_back_to_the_default() {
        assert_eq!(resolve(3, Some("1left")).id, "1left");
        assert_eq!(resolve(3, Some("nope")).id, "cols");
        assert_eq!(resolve(3, None).id, "cols");
        assert_eq!(resolve(8, Some("cols")).id, "auto");
        assert_eq!(key(3, &resolve(3, Some("rows"))), "3:rows");
    }

    #[test]
    fn spanned_boundaries_stay_fixed() {
        let l = resolve(3, Some("2top"));
        assert_eq!(l.resizable(Axis::Col), Vec::<usize>::new());
        assert_eq!(l.resizable(Axis::Row), vec![1]);
        let g = resolve(4, Some("grid"));
        assert_eq!(g.resizable(Axis::Col), vec![1]);
        assert_eq!(resolve(5, None).resizable(Axis::Col), Vec::<usize>::new());
    }

    #[test]
    fn dragging_trades_between_neighbours_only() {
        let w = vec![1.0, 1.0, 1.0];
        assert!((edge(&w, 1) - 1.0 / 3.0).abs() < 1e-6);
        let d = drag(&w, 1, 0.1);
        assert!((d[0] - 1.3).abs() < 1e-5 && (d[1] - 0.7).abs() < 1e-5);
        assert_eq!(d[2], 1.0);
        let clamped = drag(&w, 1, -1.0);
        assert!((clamped[0] - 0.36).abs() < 1e-5);
        assert_eq!(drag(&w, 0, 0.5), w);
        let l = resolve(2, None);
        assert_eq!(l.weights(Axis::Col, &[2.0, 1.0]), vec![2.0, 1.0]);
        assert_eq!(l.weights(Axis::Col, &[2.0]), vec![1.0, 1.0]);
    }
}
