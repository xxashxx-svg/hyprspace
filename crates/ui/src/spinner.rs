// Three dots that bounce in turn while something runs, after loading.dev's bouncing dots.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ElementId, Hsla, IntoElement, div, ease_in_out,
    prelude::*, px,
};

const DOT: f32 = 3.5;
const LIFT: f32 = 3.;

/// Sized to sit on a 12.5px line. `id` must be unique among its siblings.
pub fn dots(id: impl Into<ElementId>, color: Hsla) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .h(px(12.5))
        .flex()
        .items_center()
        .gap(px(3.))
        .children((0..3usize).map(move |i| {
            div()
                .relative()
                .size(px(DOT))
                .rounded_full()
                .bg(color)
                .with_animation(
                    ("dot", i),
                    Animation::new(Duration::from_millis(500)).repeat(),
                    move |d, t| {
                        // Each dot runs a third of a cycle behind the one before it.
                        let p = (t - i as f32 / 3.).rem_euclid(1.);
                        let up = ease_in_out(1. - (2. * p - 1.).abs());
                        d.top(px(LIFT / 2. - LIFT * up))
                    },
                )
        }))
        .into_any_element()
}
