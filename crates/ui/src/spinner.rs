// The spinner shown while something runs, after loading.dev's Eclipse: two dots trading places,
// one passing behind the other.

use std::f32::consts::TAU;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ElementId, Hsla, IntoElement, div, prelude::*, px,
};

/// Two dots circling each other on an ellipse seen edge on: the one going behind shrinks and
/// fades, so they read as passing. Sized to sit on a 12.5px line. `id` must be unique among its
/// siblings.
pub fn eclipse(id: impl Into<ElementId>, color: Hsla) -> AnyElement {
    const W: f32 = 14.;
    const H: f32 = 12.5;
    const DOT: f32 = 5.;
    const REACH: f32 = 3.5;
    div()
        .id(id)
        .flex_none()
        .relative()
        .w(px(W))
        .h(px(H))
        .children([0., 0.5].map(|offset: f32| {
            div().absolute().rounded_full().bg(color).with_animation(
                ("eclipse", (offset * 2.) as usize),
                Animation::new(Duration::from_millis(1200)).repeat(),
                move |d, t| {
                    let a = (t + offset) * TAU;
                    // depth: 1 nearest, -1 furthest
                    let near = a.sin();
                    let size = DOT * (1. + 0.2 * near);
                    d.size(px(size))
                        .left(px(W / 2. + REACH * a.cos() - size / 2.))
                        .top(px((H - size) / 2.))
                        .opacity(0.65 + 0.35 * near)
                },
            )
        }))
        .into_any_element()
}
