// The window's top row, drawn by us in place of the system's title bar the way zeron does it:
// the sidebar toggle, search (the palette) and New thread over the sidebar, the space's bar over
// the rest, and on Windows the caption buttons. macOS keeps its traffic lights, inset at the left end.
// Windows drags and snaps the window through the row's hit-test areas; macOS gets the drag from
// `start_window_move`.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, MouseButton, Window, WindowControlArea, div,
    prelude::*, px, svg,
};

use super::{Root, Screen};
use crate::sidebar::{MAX_WIDTH, MIN_WIDTH};
use crate::slide::slide;
use crate::workbench::ToggleSidebar;
use crate::{colors, widgets};

pub const HEIGHT: f32 = 40.;

/// The sidebar buttons' width with their padding: three 28px buttons, the gaps between, and
/// 8px before and 4px after.
const NAV: f32 = 8. + 3. * 28. + 2. * 2. + 4.;

/// Room the traffic lights take at the row's left end on macOS.
const LIGHTS: f32 = if cfg!(target_os = "macos") { 78. } else { 0. };

impl Root {
    /// Called after the main area is drawn, so the bar sees what the workbench settled.
    pub(crate) fn titlebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Settings takes the whole window, without the sidebar or its buttons
        let sidebar = self.screen != Screen::Settings;
        let open = sidebar && !self.state.sidebar_hidden;
        let left = sidebar.then(|| {
            let width = self.state.sidebar_width.clamp(MIN_WIDTH, MAX_WIDTH);
            let body = div()
                .flex_none()
                .w(px(width))
                .h_full()
                .flex()
                .items_center()
                .pl(px(LIGHTS + 8.))
                .when(open, |d| d.border_r_1().border_color(colors::border0()))
                .child(self.nav(cx));
            // shut, the row keeps just the buttons
            slide(
                "titlebar-sidebar",
                self.sidebar_flips.count(),
                open,
                (LIGHTS + NAV, width),
                false,
                body,
            )
        });
        let space = match self.screen {
            Screen::Thread(id) => self.state.thread(id).map(|(s, _)| s.id),
            Screen::Compose(space) => space.filter(|s| self.state.space(*s).is_some()),
            Screen::Settings => None,
        };
        // a structured thread runs edge to edge up to the row, the way zeron's does
        let line =
            matches!(self.screen, Screen::Thread(_)) && self.structured_on_screen().is_none();
        let bar = match space {
            Some(s) => self.bar(s, cx),
            None => div().flex_1().into_any_element(),
        };
        div()
            .id("titlebar")
            .flex_none()
            .flex()
            .w_full()
            .h(px(HEIGHT))
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|r, _, _, _| r.moving = cfg!(target_os = "macos")),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|r, _, _, _| r.moving = false),
            )
            .on_mouse_move(cx.listener(|r, _, window, _| {
                if r.moving {
                    r.moving = false;
                    window.start_window_move();
                }
            }))
            .when(cfg!(target_os = "macos"), |d| {
                d.on_click(|e, window, _| {
                    if e.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
            })
            .children(left)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .when(!sidebar, |d| d.pl(px(LIGHTS)))
                    .when(line, |d| d.border_b_1().border_color(colors::border0()))
                    .child(bar)
                    .when(cfg!(target_os = "windows"), |d| d.child(caption(window))),
            )
            .into_any_element()
    }
}

impl Root {
    /// The logo and the button that hides or shows the sidebar, as the Tauri app had them. Search
    /// and New thread live at the top of the sidebar itself. The button occludes the row so a
    /// click doesn't drag the window.
    fn nav(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("sidebar-nav")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .pl(px(4.))
            .child(crate::assets::logo(14.))
            .child(
                widgets::icon_button("sidebar-toggle", "panel-left", 28.)
                    .occlude()
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                        r.toggle_sidebar(&ToggleSidebar, window, cx)
                    })),
            )
            .into_any_element()
    }
}

/// Minimize, maximize or restore, and close, as Windows draws them: flat 46px cells, close
/// going red on hover. The glyphs are thin 10px marks in the theme's text color.
fn caption(window: &Window) -> AnyElement {
    let cell = |id: &'static str, glyph: &'static str, area: WindowControlArea, close: bool| {
        let group = id;
        div()
            .id(id)
            .group(group)
            .occlude()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(46.))
            .h_full()
            .window_control_area(area)
            .hover(|s| {
                s.bg(if close {
                    colors::error()
                } else {
                    colors::ink(0.08)
                })
            })
            .child(
                svg()
                    .path(format!("caption/{glyph}.svg"))
                    .size(px(10.))
                    .text_color(colors::text2())
                    .group_hover(group, |s| s.text_color(colors::text1())),
            )
    };
    let max = if window.is_maximized() {
        "restore"
    } else {
        "max"
    };
    div()
        .flex()
        .flex_none()
        .h_full()
        .child(cell("caption-min", "min", WindowControlArea::Min, false))
        .child(cell("caption-max", max, WindowControlArea::Max, false))
        .child(cell(
            "caption-close",
            "close",
            WindowControlArea::Close,
            true,
        ))
        .into_any_element()
}
