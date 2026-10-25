use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::HighlightStyle;

use super::{Block, Inline};

const FADE: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct Fade {
    ix: Option<usize>,
    solid: usize,
    arrived: Vec<(usize, Instant)>,
}

impl Fade {
    pub fn grow(&mut self, ix: usize, blocks: &[Block]) {
        let total = size(blocks);
        if self.ix != Some(ix) {
            *self = Fade {
                ix: Some(ix),
                solid: total,
                arrived: Vec::new(),
            };
            return;
        }
        while self.arrived.last().is_some_and(|(t, _)| *t > total) {
            self.arrived.pop();
        }
        self.solid = self.solid.min(total);
        let last = self.arrived.last().map_or(self.solid, |(t, _)| *t);
        if total > last {
            self.arrived.push((total, Instant::now()));
        }
    }

    pub fn tick(&mut self) -> bool {
        let done = self
            .arrived
            .iter()
            .take_while(|(_, at)| at.elapsed() >= FADE)
            .count();
        if done > 0 {
            self.solid = self.arrived[done - 1].0;
            self.arrived.drain(..done);
        }
        !self.arrived.is_empty()
    }

    pub fn at(&self, ix: usize) -> Vec<(Range<usize>, f32)> {
        if self.ix != Some(ix) {
            return Vec::new();
        }
        let mut from = self.solid;
        self.arrived
            .iter()
            .map(|&(to, at)| {
                let x = (at.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.);
                let r = (from..to, x * x * (3. - 2. * x));
                from = to;
                r
            })
            .collect()
    }
}

pub fn size(blocks: &[Block]) -> usize {
    blocks.iter().map(block_size).sum()
}

pub(super) fn block_size(b: &Block) -> usize {
    let chars = |i: &Inline| i.text.chars().count();
    match b {
        Block::Paragraph(i) | Block::Heading(_, i) => chars(i),
        Block::Code { text, .. } => text.trim_end().chars().count(),
        Block::List { items, .. } => items.iter().map(|i| size(i)).sum(),
        Block::Quote(inner) => size(inner),
        Block::Rule | Block::Image { .. } => 0,
        Block::Table { head, rows } => head.iter().chain(rows.iter().flatten()).map(chars).sum(),
    }
}

pub(super) fn local(
    text: &str,
    start: usize,
    fade: &[(Range<usize>, f32)],
) -> Vec<(Range<usize>, f32)> {
    let end = start + text.chars().count();
    if fade.iter().all(|(r, _)| r.end <= start || r.start >= end) {
        return Vec::new();
    }
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain([text.len()])
        .collect();
    fade.iter()
        .filter(|(r, _)| r.end > start && r.start < end)
        .map(|(r, a)| {
            let at = |c: usize| bytes[c.clamp(start, end) - start];
            (at(r.start)..at(r.end), *a)
        })
        .filter(|(r, _)| !r.is_empty())
        .collect()
}

pub(super) fn merge(
    len: usize,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    fade: &[(Range<usize>, f32)],
) -> Vec<(Range<usize>, HighlightStyle)> {
    if fade.is_empty() {
        return highlights;
    }
    let mut cuts: Vec<usize> = highlights
        .iter()
        .map(|(r, _)| r)
        .chain(fade.iter().map(|(r, _)| r))
        .flat_map(|r| [r.start, r.end])
        .chain([0, len])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter_map(|w| {
            let (a, b) = (w[0], w[1]);
            let covers = |r: &Range<usize>| r.start <= a && b <= r.end;
            let base = highlights.iter().find(|(r, _)| covers(r)).map(|(_, s)| *s);
            let alpha = fade.iter().find(|(r, _)| covers(r)).map(|(_, x)| *x);
            let style = match (base, alpha) {
                (base, None) => base?,
                (base, Some(x)) => {
                    let mut s = base.unwrap_or_default();
                    s.fade_out = Some(1. - x);
                    if let Some(bg) = s.background_color.as_mut() {
                        bg.a *= x;
                    }
                    s
                }
            };
            Some((a..b, style))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fading_cuts_styled_runs_without_overlap() {
        let bold = HighlightStyle {
            font_weight: Some(gpui::FontWeight::BOLD),
            ..Default::default()
        };
        let text = "héllo world";
        let fade = local(text, 10, &[(10..13, 0.5), (16..30, 0.0)]);
        assert_eq!(fade, [(0..4, 0.5), (7..12, 0.0)]);
        let out = merge(text.len(), vec![(2..9, bold)], &fade);
        let ranges: Vec<_> = out.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(ranges, [0..2, 2..4, 4..7, 7..9, 9..12]);
        assert_eq!(out[1].1.font_weight, bold.font_weight);
        assert_eq!(out[1].1.fade_out, Some(0.5));
        assert_eq!(out[2].1.fade_out, None);
        assert_eq!(out[4].1.font_weight, None);
        assert!(local(text, 30, &[(0..10, 0.5)]).is_empty());
    }
}
