// The window's layout: sidebar, then the thread on screen under a one-line header, the composer,
// or settings. Context menus open from here so they float over everything.

use gpui::{
    AnyElement, ClickEvent, Context, DragMoveEvent, FontWeight, IntoElement, Render, Window, div,
    prelude::*, px,
};

use super::threads::folder_name;
use super::{Action, Root, Screen, SidebarDrag, View};
use crate::assets::{icon, mark};
use crate::sidebar::{MAX_WIDTH, MIN_WIDTH};
use crate::{colors, widgets};

impl Root {
    /// The agent's mark and the thread's title, then the folder it runs in, like zeron's.
    fn header(&self, id: u64) -> AnyElement {
        let Some((_, t)) = self.state.thread(id) else {
            return div().into_any_element();
        };
        let badge = match t.agent() {
            Some(launch) => {
                mark(launch.agent, 14., colors::brand(launch.agent).0).into_any_element()
            }
            None => icon("terminal", 13., colors::text3()).into_any_element(),
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .h(px(44.))
            .border_b_1()
            .border_color(colors::border0())
            .child(badge)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(t.title.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(folder_name(t.cwd())),
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
                Screen::Settings => self.settings(cx),
            }
        };
        let menu = self.menu(window, cx);
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
    }
}
