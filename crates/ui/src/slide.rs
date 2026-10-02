// Side panels slide open and shut along their width instead of popping. The panel keeps its own
// width inside a clipping frame, so its contents don't squash while the frame moves.

use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, IntoElement, div, ease_in_out, prelude::*, px};

const LENGTH: Duration = Duration::from_millis(180);

/// A panel's open state as last drawn and how often it changed. Each change replays the slide,
/// whichever code flipped the state.
#[derive(Default)]
pub struct Flips {
    open: Option<bool>,
    count: usize,
}

impl Flips {
    /// Notes the state this draw shows and returns the change count. The first look counts as
    /// no change, so a panel saved shut stays shut at launch instead of sliding away.
    pub fn see(&mut self, open: bool) -> usize {
        if self.open.is_some_and(|o| o != open) {
            self.count += 1;
        }
        self.open = Some(open);
        self.count
    }

    /// The change count as of the last `see`.
    pub fn count(&self) -> usize {
        self.count
    }
}

/// `body` in a frame that is `full` wide when open and `shut` wide when not, sliding between the
/// two after a change. `end` pins the body to the frame's right edge, for a panel on the right.
pub fn slide(
    id: &'static str,
    flips: usize,
    open: bool,
    (shut, full): (f32, f32),
    end: bool,
    body: impl IntoElement,
) -> AnyElement {
    let frame = div()
        .flex_none()
        .h_full()
        .flex()
        .overflow_hidden()
        .when(end, |d| d.justify_end())
        .child(body);
    if flips == 0 {
        return frame
            .w(px(if open { full } else { shut }))
            .into_any_element();
    }
    let (from, to) = if open { (shut, full) } else { (full, shut) };
    frame
        .with_animation(
            (id, flips),
            Animation::new(LENGTH).with_easing(ease_in_out),
            move |d, t| d.w(px(from + (to - from) * t)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_change_counts() {
        let mut f = Flips::default();
        assert_eq!(f.see(false), 0);
        assert_eq!(f.see(false), 0);
        assert_eq!(f.see(true), 1);
        assert_eq!(f.see(true), 1);
        assert_eq!(f.see(false), 2);
    }
}
