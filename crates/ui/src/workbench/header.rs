// A terminal thread's title bar, after the Tauri app's .pane-tabs: the agent's mark, the name, a
// quiet tag for a model picked on purpose or a folder other than the space's, and a close button
// that shows the space's composer. A structured thread has none; the bar above carries its name.

use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, SharedString, Window,
    div, img, prelude::*, px,
};
use hyprspace_proto::{Agent, Pane, ThreadKind};

use crate::assets::{icon, mark};
use crate::colors;
use crate::root::Root;

/// The last two parts of a folder, which is what tells them apart in practice.
pub fn short(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    match parts.len() {
        0 => path.display().to_string(),
        1 => parts[0].clone(),
        n => format!("{}/{}", parts[n - 2], parts[n - 1]),
    }
}

/// A thread picked up by its sidebar row: dropped on another row it moves there, dropped on the
/// Settled shelf it settles.
pub struct PaneDrag {
    pub pane: Pane,
    pub(crate) title: SharedString,
    pub(crate) agent: Option<Agent>,
}

impl PaneDrag {
    /// What follows the cursor: a one-line chip with the thread's name.
    pub(crate) fn ghost(&self, grab: Point<Pixels>) -> Ghost {
        Ghost {
            title: self.title.clone(),
            agent: self.agent,
            grab,
        }
    }
}

/// What follows the cursor while a row is dragged: a chip with its name, not the row itself, so
/// the drop targets under it stay visible.
pub struct Ghost {
    title: SharedString,
    agent: Option<Agent>,
    /// Where the row was grabbed. The drag view is drawn from the row's corner, so the chip is
    /// pushed over by this much to sit under the cursor.
    grab: Point<Pixels>,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let badge = match self.agent {
            Some(a) => mark(a, 12., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 12., colors::text2()).into_any_element(),
        };
        div()
            .pl((self.grab.x - px(16.)).max(px(0.)))
            .pt((self.grab.y - px(14.)).max(px(0.)))
            .child(
                div()
                    .w(px(220.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border2())
                    .bg(colors::surface3())
                    .shadow(colors::shadow())
                    .text_size(px(12.5))
                    .text_color(colors::text1())
                    .child(badge)
                    .child(div().truncate().child(self.title.clone())),
            )
    }
}

/// A square button in a header: quiet until hovered. Close turns red under the pointer.
pub(super) fn head_button(
    id: (&'static str, usize),
    name: &str,
    danger: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |s| {
            if danger {
                s.bg(colors::error().opacity(0.22))
            } else {
                s.bg(colors::surface3())
            }
        })
        .child(icon(name, 13., colors::text3()))
}

impl Root {
    pub(crate) fn pane_header(
        &mut self,
        space: u64,
        thread: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some((_, t)) = self.state.thread(thread) else {
            return div().into_any_element();
        };
        // the mark already says which agent and the title bar which folder, so the detail only
        // names a model picked on purpose, and a folder other than the space's
        let home = self.state.space(space).and_then(|s| s.cwd.as_ref());
        let away = |cwd: &Path| (home.map(|h| h.as_path()) != Some(cwd)).then(|| short(cwd));
        let (badge, detail) = match &t.kind {
            ThreadKind::Terminal {
                cwd,
                run: Some(launch),
            } => (
                mark(launch.agent, 13., colors::brand(launch.agent).0).into_any_element(),
                [
                    launch.model.as_ref().map(|_| self.model_label(launch)),
                    away(cwd),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
            ),
            ThreadKind::Terminal { cwd, run: None } => (
                icon("terminal", 12., colors::text3()).into_any_element(),
                away(cwd).unwrap_or_default(),
            ),
            ThreadKind::Structured { .. } => return div().into_any_element(),
        };
        let title: SharedString = t.title.clone().into();
        div()
            .id("pane-head")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(30.))
            .pl(px(8.))
            .pr(px(6.))
            .bg(colors::surface1())
            .child(div().flex().flex_none().items_center().child(badge))
            .child(
                div()
                    .ml(px(3.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(8.))
                    .flex()
                    .pl(px(6.))
                    .when(!detail.is_empty(), |d| {
                        d.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(5.))
                                .bg(colors::ink(0.05))
                                .text_size(px(11.))
                                .text_color(colors::text3())
                                .child(detail),
                        )
                    }),
            )
            .child(
                head_button(("pane-close", 0), "x", true).on_click(cx.listener(
                    move |r, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        r.compose(Some(space), window, cx)
                    },
                )),
            )
            .into_any_element()
    }
}

/// The logo for an app in the Open menu: the editor's own mark, or the file manager's.
pub fn opener_logo(opener: hyprspace_proto::Opener, size: f32) -> AnyElement {
    use hyprspace_proto::Opener;
    match opener {
        Opener::VsCode => img("brand/vscode.svg").size(px(size)).into_any_element(),
        // Cursor's mark is one color, so it takes the text color and reads on both sides
        Opener::Cursor => gpui::svg()
            .path("brand/cursor.svg")
            .size(px(size))
            .text_color(colors::text1())
            .into_any_element(),
        Opener::Files if cfg!(target_os = "macos") => {
            img("brand/finder.svg").size(px(size)).into_any_element()
        }
        Opener::Files => img("brand/explorer.png").size(px(size)).into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paths_keep_the_last_two_parts() {
        assert_eq!(short(std::path::Path::new("/a/b/c")), "b/c");
        assert_eq!(short(std::path::Path::new("/c")), "c");
    }
}
