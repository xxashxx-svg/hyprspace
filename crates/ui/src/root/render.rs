// The window's layout: sidebar, then the space on screen (its panes, see `crate::panes`), the
// composer, or settings. Context menus open from here so they float over everything.

use gpui::{
    AnyElement, ClickEvent, Context, DragMoveEvent, IntoElement, Render, Window, div, prelude::*,
    px,
};

use super::{Action, Root, Screen, SidebarDrag};
use crate::sidebar::{MAX_WIDTH, MIN_WIDTH};
use crate::{colors, widgets};

impl Root {
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
                Action::OpenBeside(_) => (Some("panel-right"), false),
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
                Screen::Thread(id) => self.workbench(id, window, cx),
                Screen::Compose(space) => self.compose_screen(space, window, cx),
                Screen::Settings => self.settings(window, cx),
            }
        };
        let menu = self.menu(window, cx);
        // Settings takes the whole window
        let sidebar = (self.screen != Screen::Settings).then(|| self.sidebar(window, cx));
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
            .children(sidebar)
            .child(div().flex_1().min_w_0().h_full().child(main))
            .children(menu)
    }
}
