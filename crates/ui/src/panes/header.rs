// A pane's title bar, after the Tauri app's .pane-tabs: a grip to drag it onto another pane, the
// agent's mark, the name, and a close button. Double-click it to fill the space, again to go
// back. A viewer pane carries its own actions here too, so it needs no second bar.

use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, SharedString, Window,
    div, img, prelude::*, px,
};
use hyprspace_proto::{Agent, Command, Pane, ThreadKind};
use hyprspace_theme::MONO;

use crate::assets::{icon, mark};
use crate::colors;
use crate::root::Root;
use crate::transcript::Status;
use crate::widgets;

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

/// A pane picked up by its header, or a thread picked up by its sidebar row.
pub struct PaneDrag {
    pub space: u64,
    pub pane: Pane,
    pub(crate) title: SharedString,
    pub(crate) agent: Option<Agent>,
}

impl PaneDrag {
    /// What follows the cursor: the pane card, or a one-line chip for a sidebar row.
    pub(crate) fn ghost(&self, grab: Point<Pixels>, compact: bool) -> Ghost {
        Ghost {
            title: self.title.clone(),
            agent: self.agent,
            grab,
            compact,
        }
    }
}

/// What follows the cursor while a pane is dragged: a small card with the pane's name, not the
/// pane itself, so the drop targets under it stay visible.
pub struct Ghost {
    title: SharedString,
    agent: Option<Agent>,
    /// Where the header was grabbed. The drag view is drawn from the header's corner, so the
    /// card is pushed over by this much to sit under the cursor.
    grab: Point<Pixels>,
    /// A sidebar row's chip rather than a pane's card.
    compact: bool,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let badge = match self.agent {
            Some(a) => mark(a, 12., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 12., colors::text2()).into_any_element(),
        };
        if self.compact {
            return div()
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
                .into_any_element();
        }
        div()
            .pl((self.grab.x - px(24.)).max(px(0.)))
            .pt((self.grab.y - px(12.)).max(px(0.)))
            .child(
                div()
                    .w(px(240.))
                    .h(px(150.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(colors::border2())
                    .bg(colors::surface2())
                    .shadow(colors::shadow())
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(9.))
                            .py(px(6.))
                            .bg(colors::surface3())
                            .border_b_1()
                            .border_color(colors::border1())
                            .text_size(px(11.))
                            .text_color(colors::text1())
                            .child(badge)
                            .child(div().truncate().child(self.title.clone())),
                    )
                    .child(div().flex_1().bg(colors::bg().opacity(0.85))),
            )
            .into_any_element()
    }
}

/// A square button in a pane header: quiet until hovered. Close turns red under the pointer.
fn head_button(id: (&'static str, usize), name: &str, danger: bool) -> gpui::Stateful<gpui::Div> {
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
    /// The mark, title and dim detail a pane shows, and the agent it runs if any.
    fn pane_title(
        &self,
        space: u64,
        pane: &Pane,
        cx: &mut Context<Self>,
    ) -> (AnyElement, SharedString, String, Option<Agent>) {
        match pane {
            Pane::Thread { id } => {
                let Some((_, t)) = self.state.thread(*id) else {
                    return (div().into_any_element(), "".into(), String::new(), None);
                };
                // the mark already says which agent and the title bar which folder, so the detail
                // only names a model picked on purpose, and a folder other than the space's
                let home = self.state.space(space).and_then(|s| s.cwd.as_ref());
                let away =
                    |cwd: &Path| (home.map(|h| h.as_path()) != Some(cwd)).then(|| short(cwd));
                let (badge, detail, agent) = match &t.kind {
                    ThreadKind::Structured { launch }
                    | ThreadKind::Terminal {
                        run: Some(launch), ..
                    } => {
                        let model = launch.model.as_ref().map(|_| self.model_label(launch));
                        let cwd = match &t.kind {
                            ThreadKind::Terminal { cwd, .. } => cwd,
                            _ => &launch.cwd,
                        };
                        (
                            mark(launch.agent, 13., colors::brand(launch.agent).0)
                                .into_any_element(),
                            [model, away(cwd)]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" · "),
                            Some(launch.agent),
                        )
                    }
                    ThreadKind::Terminal { cwd, run: None } => (
                        icon("terminal", 12., colors::text3()).into_any_element(),
                        away(cwd).unwrap_or_default(),
                        None,
                    ),
                };
                (badge, t.title.clone().into(), detail, agent)
            }
            Pane::File { path, .. } => self.file_title(space, "file-code", path),
            Pane::Diff { cwd, path } => {
                let root = self
                    .work
                    .viewers
                    .get(&space)
                    .and_then(|v| v.read(cx).root().map(Path::to_path_buf));
                let full = root.unwrap_or_else(|| cwd.clone()).join(path);
                self.file_title(space, "file-diff", &full)
            }
        }
    }

    /// A file's name, and its folder relative to the space's.
    fn file_title(
        &self,
        space: u64,
        glyph: &str,
        file: &Path,
    ) -> (AnyElement, SharedString, String, Option<Agent>) {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let base = self.state.space(space).and_then(|s| s.cwd.clone());
        let dir = file.parent().map(|p| match &base {
            Some(b) => p
                .strip_prefix(b)
                .map(|r| r.display().to_string())
                .unwrap_or_else(|_| short(p)),
            None => short(p),
        });
        (
            icon(glyph, 12., colors::text3()).into_any_element(),
            name.into(),
            dir.unwrap_or_default().replace('\\', "/"),
            None,
        )
    }

    pub(crate) fn pane_header(
        &mut self,
        space: u64,
        pane: &Pane,
        focused: bool,
        maxed: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (badge, title, detail, agent) = self.pane_title(space, pane, cx);
        let is_thread = matches!(pane, Pane::Thread { .. });
        let status = pane
            .thread()
            .map(|id| self.status.get(&id).copied().unwrap_or(Status::Idle));
        let actions = self.viewer_actions(space, pane, cx);
        let drag = PaneDrag {
            space,
            pane: pane.clone(),
            title: title.clone(),
            agent,
        };
        let (max_pane, close_pane) = (pane.clone(), pane.clone());
        div()
            .id("pane-head")
            .group("pane-head")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(30.))
            .pl(px(4.))
            .pr(px(6.))
            .bg(colors::surface1())
            .cursor_grab()
            .on_drag(drag, |d, offset, _, cx| cx.new(|_| d.ghost(offset, false)))
            .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 {
                    r.toggle_max(space, max_pane.clone(), cx);
                }
            }))
            .child(
                div()
                    .flex_none()
                    .opacity(0.)
                    .group_hover("pane-head", |s| s.opacity(0.8))
                    .child(icon("grip-vertical", 12., colors::text3())),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .opacity(if focused { 1. } else { 0.8 })
                    .child(badge),
            )
            .child(
                div()
                    .ml(px(3.))
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if focused {
                        colors::text1()
                    } else {
                        colors::text2()
                    })
                    .child(title),
            )
            .children(status.map(|s| div().ml(px(2.)).child(widgets::status_dot(s))))
            .child(
                div()
                    .flex_1()
                    .min_w(px(8.))
                    .flex()
                    .pl(px(6.))
                    .when(!detail.is_empty(), |d| {
                        // a thread's detail is a quiet tag; a file's folder stays a path
                        let tag = div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(colors::text3());
                        d.child(if is_thread {
                            tag.px(px(6.))
                                .py(px(1.))
                                .rounded(px(5.))
                                .bg(colors::ink(0.05))
                                .child(detail)
                        } else {
                            tag.font_family(MONO).child(detail)
                        })
                    }),
            )
            .children(actions)
            .child(
                head_button(("pane-close", 0), "x", true)
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        r.close_pane(space, close_pane.clone(), window, cx)
                    })),
            )
            .when(maxed, |d| d.border_b_1().border_color(colors::border1()))
            .into_any_element()
    }

    /// A viewer pane's own buttons: its line counts for a diff, open the file from a diff, and
    /// open it in the user's editor.
    fn viewer_actions(
        &mut self,
        space: u64,
        pane: &Pane,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let file = match pane {
            Pane::File { path, line, col } => Some((path.clone(), *line, *col)),
            Pane::Diff { cwd, path } => {
                let root = self
                    .work
                    .viewers
                    .get(&space)
                    .and_then(|v| v.read(cx).root().map(Path::to_path_buf))
                    .unwrap_or_else(|| cwd.clone());
                Some((root.join(path), None, None))
            }
            Pane::Thread { .. } => None,
        };
        let Some((path, line, col)) = file else {
            return vec![];
        };
        let mut out = Vec::new();
        if let Pane::Diff { .. } = pane {
            if let Some((add, del)) = self
                .work
                .viewers
                .get(&space)
                .and_then(|v| v.read(cx).counts())
            {
                out.push(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(6.))
                        .mr(px(4.))
                        .font_family(MONO)
                        .text_size(px(11.))
                        .when(add > 0, |d| {
                            d.child(
                                div()
                                    .text_color(colors::diff_add())
                                    .child(format!("+{add}")),
                            )
                        })
                        .when(del > 0, |d| {
                            d.child(
                                div()
                                    .text_color(colors::diff_del())
                                    .child(format!("-{del}")),
                            )
                        })
                        .into_any_element(),
                );
            }
            let open = path.clone();
            out.push(
                head_button(("pane-view-file", 0), "file-code", false)
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.open_path(open.clone(), None, None, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let client = self.client.clone();
        out.push(
            head_button(("pane-editor", 0), "external-link", false)
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, _, _| {
                    client.send(Command::OpenFile {
                        path: path.clone(),
                        line,
                        col,
                    })
                })
                .into_any_element(),
        );
        out
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
