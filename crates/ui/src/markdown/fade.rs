use std::time::{Duration, Instant};

use super::Block;

const FADE: Duration = Duration::from_millis(600);

/// Streaming replies' top-level blocks, each fading in once as it first shows. Text that joins a
/// block already there shows as it is. A reply keeps fading after the next one starts.
#[derive(Default)]
pub struct Fade {
    replies: Vec<(usize, Vec<Option<Instant>>)>,
}

impl Fade {
    /// `fresh` is a reply that started while on screen; one already there keeps its blocks solid.
    pub fn grow(&mut self, ix: usize, blocks: &[Block], fresh: bool) {
        let at = match self.replies.iter().position(|(i, _)| *i == ix) {
            Some(at) => at,
            None => {
                let old = if fresh { 0 } else { blocks.len() };
                self.replies.push((ix, vec![None; old]));
                self.replies.len() - 1
            }
        };
        let born = &mut self.replies[at].1;
        born.truncate(blocks.len());
        born.resize(blocks.len(), Some(Instant::now()));
    }

    /// Whether the reply at `ix` is fading in.
    pub fn has(&self, ix: usize) -> bool {
        self.replies.iter().any(|(i, _)| *i == ix)
    }

    pub fn tick(&mut self) -> bool {
        for (_, born) in &mut self.replies {
            for at in born.iter_mut() {
                if at.is_some_and(|t| t.elapsed() >= FADE) {
                    *at = None;
                }
            }
        }
        // the reply still coming in stays, so a block it adds later counts as new
        let last = self.replies.last().map(|(i, _)| *i);
        self.replies
            .retain(|(i, born)| Some(*i) == last || born.iter().any(Option::is_some));
        self.replies
            .iter()
            .any(|(_, b)| b.iter().any(Option::is_some))
    }

    /// Each top-level block's opacity, for the reply at `ix`.
    pub fn at(&self, ix: usize) -> Vec<f32> {
        let Some((_, born)) = self.replies.iter().find(|(i, _)| *i == ix) else {
            return Vec::new();
        };
        born.iter()
            .map(|at| {
                at.map_or(1., |t| {
                    ease_out((t.elapsed().as_secs_f32() / FADE.as_secs_f32()).min(1.))
                })
            })
            .collect()
    }
}

/// CSS's `ease-out`, cubic-bezier(0, 0, 0.58, 1).
fn ease_out(x: f32) -> f32 {
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..24 {
        let s = (lo + hi) / 2.;
        if 3. * (1. - s) * s * s * 0.58 + s * s * s < x {
            lo = s;
        } else {
            hi = s;
        }
    }
    let s = (lo + hi) / 2.;
    3. * (1. - s) * s * s + s * s * s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_new_blocks_fade() {
        let fading = |f: &Fade| {
            f.at(3)
                .iter()
                .enumerate()
                .filter(|(_, a)| **a < 1.)
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        let parse = super::super::parse;
        let mut f = Fade::default();
        f.grow(3, &parse("Already here."), false);
        assert!(fading(&f).is_empty());
        f.grow(3, &parse("Already here, and more."), false);
        assert!(fading(&f).is_empty());
        f.grow(3, &parse("Already here, and more.\n\n- new\n- list"), false);
        assert_eq!(fading(&f), [1]);
        assert!(f.at(4).is_empty());

        let mut f = Fade::default();
        f.grow(3, &parse("Fresh"), true);
        assert_eq!(fading(&f), [0]);
        f.grow(3, &parse("Fresh reply"), true);
        assert_eq!(fading(&f), [0]);
        f.grow(5, &parse("The next reply"), true);
        assert_eq!(fading(&f), [0], "the first keeps fading");
        assert!(f.has(5));
    }

    #[test]
    fn eases_out_like_css() {
        assert!(ease_out(0.) < 1e-4);
        assert!((ease_out(1.) - 1.).abs() < 1e-4);
        assert!((ease_out(0.5) - 0.69).abs() < 0.02);
    }
}
