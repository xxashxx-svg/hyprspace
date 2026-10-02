// The window's layout: sidebar, then the thread on screen under a one-line header, or the
// composer. Context menus open from here so they float over everything.

use gpui::{
    AnyElement, ClickEvent, Context, DragMoveEvent, FontWeight, IntoElement, Render, Window, div,
    prelude::*, px,
};
use hyprspace_proto::ThreadKind;
use hyprspace_theme::MONO;

use super::{Action, Root, Screen, SidebarDrag, View};
use crate::assets::{icon, mark};
use crate::sidebar::{MAX_WIDTH, MIN_WIDTH};
use crate::transcript::Status;
use crate::{colors, widgets};

/// The last two parts of a folder, which is what tells them apart in practice.
fn short(path: &std::path::Path) -> String {
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

impl Root {
    fn header(&self, id: u64) -> AnyElement {
        let Some((space, t)) = self.state.thread(id) else {
            return div().into_any_element();
        };
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let (badge, detail) = match &t.kind {
            ThreadKind::Structured { launch } => (
                mark(launch.agent, 14., colors::brand(launch.agent).0).into_any_element(),
                format!("{} · {}", self.model_label(launch), short(&launch.cwd)),
            ),
            ThreadKind::Terminal {
                cwd,
                run: Some(launch),
            } => (
                mark(launch.agent, 14., colors::brand(launch.agent).0).into_any_element(),
                format!(
                    "{} in a terminal · {}",
                    self.model_label(launch),
                    short(cwd)
                ),
            ),
            ThreadKind::Terminal { cwd, run: None } => (
                icon("terminal", 13., colors::text3()).into_any_element(),
                format!("Terminal · {}", short(cwd)),
            ),
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .h(px(40.))
            .border_b_1()
            .border_color(colors::border0())
            .child(badge)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t.title.clone()),
            )
            .child(widgets::status_dot(status))
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(format!("{} · {detail}", space.name)),
            )
            .into_any_element()
    }

    fn menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (at, items) = self.menu.clone()?;
        let rows = items.into_iter().enumerate().map(|(i, (label, action))| {
            let (glyph, danger) = match action {
                Action::NewThread(_) => (Some("square-pen"), false),
                Action::NewTerminal(_) => (Some("terminal"), false),
                Action::Rename(_) => (Some("pencil"), false),
                Action::ArchiveSpace(_, true) | Action::ArchiveThread(_, true) => {
                    (Some("archive"), false)
                }
                Action::ArchiveSpace(_, false) | Action::ArchiveThread(_, false) => {
                    (Some("archive-restore"), false)
                }
                Action::RemoveSpace(_) | Action::RemoveThread(_) => (Some("trash-2"), true),
                Action::AddProject => (Some("folder-open"), false),
                Action::NewOpenSpace => (Some("plus"), false),
            };
            widgets::menu_row(("menu", i), glyph, label, danger).on_click(
                cx.listener(move |r, _: &ClickEvent, window, cx| r.act(action, window, cx)),
            )
        });
        let close = cx.listener(|r, _: &(), _, cx| {
            r.menu = None;
            cx.notify();
        });
        Some(widgets::popup(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            div().flex().flex_col().gap(px(1.)).children(rows),
        ))
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main: AnyElement = if !self.loaded {
            div().into_any_element()
        } else {
            match self.screen {
                Screen::Thread(id) => {
                    let view: AnyElement = match self.views.get(&id) {
                        Some(View::Structured(v)) => v.clone().into_any_element(),
                        Some(View::Terminal(v)) => v.clone().into_any_element(),
                        None => div().into_any_element(),
                    };
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .child(self.header(id))
                        .child(div().flex_1().min_h_0().child(view))
                        .into_any_element()
                }
                Screen::Compose(_) => self.composer.clone().into_any_element(),
            }
        };
        let menu = self.menu(window, cx);
        let appearance = self
            .appearance_at
            .map(|at| self.appearance_menu(at, window, cx));
        div()
            .id("root")
            .size_full()
            .flex()
            .bg(colors::bg())
            .text_color(colors::text1())
            .font_family(hyprspace_theme::SANS)
            .on_drag_move(cx.listener(|r, e: &DragMoveEvent<SidebarDrag>, _, cx| {
                let x: f32 = e.event.position.x.into();
                r.state.sidebar_width = x.clamp(MIN_WIDTH, MAX_WIDTH);
                cx.notify();
            }))
            .on_drop(cx.listener(|r, _: &SidebarDrag, _, _| r.save()))
            .child(self.sidebar(window, cx))
            .child(div().flex_1().min_w_0().h_full().child(main))
            .children(menu)
            .children(appearance)
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
