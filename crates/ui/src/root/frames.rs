// A dev build's frame readout: how many frames the window drew in the last second, how long
// each took to build, and how evenly they came. It only reads frames that happen anyway, so it
// never draws one of its own.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, IntoElement, Window, anchored, canvas, deferred, div, point, prelude::*,
};
use hyprspace_theme::MONO;

use super::Root;
use crate::colors;
use crate::zoom::px;

/// A frame later than this after the one before it is one the eye can catch.
const SLOW: Duration = Duration::from_millis(25);
/// Further apart than this, frames are not one animation but separate redraws.
const APART: Duration = Duration::from_millis(250);

#[derive(Default)]
pub(crate) struct Meter {
    pub(crate) on: bool,
    begun: Option<Instant>,
    /// Each drawn frame of the last second: when it began and how long it took to build.
    frames: VecDeque<(Instant, Duration)>,
}

impl Meter {
    fn begin(&mut self) {
        self.begun = Some(Instant::now());
    }

    fn end(&mut self) {
        let Some(at) = self.begun.take() else {
            return;
        };
        self.frames.push_back((at, at.elapsed()));
        while self
            .frames
            .front()
            .is_some_and(|(t, _)| t.elapsed() > Duration::from_secs(1))
        {
            self.frames.pop_front();
        }
    }

    fn line(&self) -> String {
        let n = self.frames.len();
        if n == 0 {
            return "no frames".into();
        }
        let cost: Vec<f32> = self.frames.iter().map(|(_, d)| ms(*d)).collect();
        let avg = cost.iter().sum::<f32>() / n as f32;
        let worst = cost.iter().copied().fold(0., f32::max);
        let gaps: Vec<Duration> = self
            .frames
            .iter()
            .zip(self.frames.iter().skip(1))
            .map(|((a, _), (b, _))| *b - *a)
            .filter(|g| *g < APART)
            .collect();
        let slow = gaps.iter().filter(|g| **g > SLOW).count();
        let spread = if gaps.len() > 1 {
            let mean = gaps.iter().map(|g| ms(*g)).sum::<f32>() / gaps.len() as f32;
            let var = gaps.iter().map(|g| (ms(*g) - mean).powi(2)).sum::<f32>() / gaps.len() as f32;
            var.sqrt()
        } else {
            0.
        };
        format!(
            "{n} fps · build {avg:.1} ms, worst {worst:.1} · {slow} late · gaps ±{spread:.1} ms"
        )
    }
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.
}

impl Root {
    /// The readout, and the marks that time this frame: one now, at the start of the draw, and
    /// one when its last element paints.
    pub(crate) fn frame_meter(
        &mut self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.frames.borrow().on {
            return None;
        }
        self.frames.borrow_mut().begin();
        let line = self.frames.borrow().line();
        let meter = self.frames.clone();
        let done = canvas(|_, _, _| {}, move |_, _, _, _| meter.borrow_mut().end()).size_0();
        Some(
            deferred(
                anchored().position(point(px(12.), px(44.))).child(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(6.))
                        .bg(colors::surface2())
                        .border_1()
                        .border_color(colors::border1())
                        .font_family(MONO)
                        .text_size(px(11.))
                        .text_color(colors::text2())
                        .child(line)
                        .child(done),
                ),
            )
            .with_priority(9)
            .into_any_element(),
        )
    }

    pub(crate) fn toggle_frame_meter(&mut self, cx: &mut Context<Self>) {
        let mut m = self.frames.borrow_mut();
        m.on = !m.on;
        m.frames.clear();
        cx.notify();
    }
}

pub(crate) type Frames = Rc<RefCell<Meter>>;
