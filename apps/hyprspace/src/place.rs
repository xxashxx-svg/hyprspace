// Where the window was: saved shortly after it moves or resizes, and opened there again. Bounds are
// in logical pixels of one display, so that display goes with them.

use std::time::Duration;

use gpui::{App, Bounds, Context, DisplayId, Task, Window, WindowBounds, point, px, size};
use hyprspace_engine::persist::{Store, state_dir};
use serde::{Deserialize, Serialize};

const FILE: &str = "window";
const SETTLE: Duration = Duration::from_millis(500);

#[derive(Serialize, Deserialize)]
struct Place {
    display: Option<String>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    maximized: bool,
}

/// Where to open: the last place, or centered on the main display the first time. GPUI moves a
/// place that no longer fits its display back onto it.
pub fn last(cx: &App) -> (WindowBounds, Option<DisplayId>) {
    let saved = Store::open(state_dir())
        .ok()
        .and_then(|s| s.load(FILE).ok().flatten())
        .and_then(|json| serde_json::from_str::<Place>(&json).ok());
    let Some(p) = saved else {
        let bounds = Bounds::centered(None, size(px(1400.), px(860.)), cx);
        return (WindowBounds::Windowed(bounds), None);
    };
    let display = cx
        .displays()
        .into_iter()
        .find(|d| d.uuid().ok().map(|u| u.to_string()) == p.display)
        .map(|d| d.id());
    let bounds = Bounds::new(point(px(p.x), px(p.y)), size(px(p.w), px(p.h)));
    let bounds = match p.maximized {
        true => WindowBounds::Maximized(bounds),
        false => WindowBounds::Windowed(bounds),
    };
    (bounds, display)
}

/// Saves the window's place each time it settles after a move or resize.
pub fn keep<T: 'static>(window: &mut Window, cx: &mut Context<T>) {
    let mut pending: Option<Task<()>> = None;
    cx.observe_window_bounds(window, move |_, window, cx| {
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Maximized(b) => (b, true),
            WindowBounds::Windowed(b) | WindowBounds::Fullscreen(b) => (b, false),
        };
        let place = Place {
            display: window
                .display(cx)
                .and_then(|d| d.uuid().ok())
                .map(|u| u.to_string()),
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            w: bounds.size.width.into(),
            h: bounds.size.height.into(),
            maximized,
        };
        let pool = cx.background_executor().clone();
        // a newer move drops the save still waiting
        pending.replace(pool.spawn({
            let pool = pool.clone();
            async move {
                pool.timer(SETTLE).await;
                if let (Ok(store), Ok(json)) =
                    (Store::open(state_dir()), serde_json::to_string(&place))
                {
                    let _ = store.save(FILE, &json);
                }
            }
        }));
    })
    .detach();
}
