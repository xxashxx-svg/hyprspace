// The spinner shown while something runs, after loading.dev's Ring: an arc turning over a faint
// circle.

use std::f32::consts::TAU;

use gpui::{AnyElement, ElementId, Hsla, IntoElement, Transformation, div, prelude::*, radians};

use crate::assets::icon;
use crate::pace::Looping;
use crate::zoom::px;

/// An arc turning over a faint ring, sized to sit on a 12.5px line. `id` must be unique among
/// its siblings.
pub fn ring(id: impl Into<ElementId>, color: Hsla) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .size(px(12.5))
        .flex()
        .items_center()
        .justify_center()
        .child(icon("spin", 12., color).looping(800, |s, t| {
            s.with_transformation(Transformation::rotate(radians(t * TAU)))
        }))
        .into_any_element()
}
