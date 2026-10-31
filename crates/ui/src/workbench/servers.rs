// The ports a thread's processes listen on: quiet addresses under the composer or in a terminal's
// header, each opening a menu to open, copy or stop it.

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, MouseButton, Pixels, Point, Stateful, Window,
    div, prelude::*,
};
use hyprspace_proto::Command;
use hyprspace_proto::resources::Server;
use hyprspace_theme::MONO;

use crate::assets::icon;
use crate::colors;
use crate::root::Root;
use crate::widgets;
use crate::zoom::px;

pub fn link(server: &Server) -> Stateful<gpui::Div> {
    div()
        .id(("server", server.port as usize))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .h(px(20.))
        .px(px(5.))
        .rounded(px(5.))
        .cursor_pointer()
        .text_color(colors::text3())
        .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
        .child(icon("globe", 12., colors::text3()))
        .child(
            div()
                .font_family(MONO)
                .text_size(px(11.))
                .child(format!("localhost:{}", server.port)),
        )
}

impl Root {
    pub(crate) fn open_server(
        &mut self,
        at: Point<Pixels>,
        server: Server,
        cx: &mut Context<Self>,
    ) {
        self.work.server_menu = Some((at, server));
        cx.notify();
    }

    pub(crate) fn server_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (at, server) = self.work.server_menu?;
        let url = format!("http://localhost:{}", server.port);
        let close = cx.listener(|r, _: &(), _, cx| {
            r.work.server_menu = None;
            cx.notify();
        });
        let row = |id: &'static str, name: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .px(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(colors::text1())
                .hover(|s| s.bg(colors::ink(0.06)))
                .child(icon(name, 13., colors::text2()))
                .child(label)
        };
        let open = url.clone();
        let body = div()
            .w(px(200.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(8.))
                    .pt(px(3.))
                    .pb(px(5.))
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(11.5))
                            .text_color(colors::text2())
                            .child(format!("localhost:{}", server.port)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(colors::text3())
                            .child(format!("pid {}", server.pid)),
                    ),
            )
            .child(
                row("server-open", "external-link", "Open in browser").on_click(cx.listener(
                    move |r, _: &ClickEvent, _, cx| {
                        r.work.server_menu = None;
                        cx.open_url(&open);
                        cx.notify();
                    },
                )),
            )
            .child(
                row("server-copy", "copy", "Copy address").on_click(cx.listener(
                    move |r, _: &ClickEvent, _, cx| {
                        r.work.server_menu = None;
                        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                        cx.notify();
                    },
                )),
            )
            .child(widgets::menu_rule())
            .child(
                row("server-stop", "square", "Stop server").on_click(cx.listener(
                    move |r, _: &ClickEvent, _, cx| {
                        r.work.server_menu = None;
                        r.client.send(Command::StopServer { pid: server.pid });
                        cx.notify();
                    },
                )),
            );
        let (at, open) = if at.y > window.viewport_size().height / 2. {
            (at, widgets::Open::Up)
        } else {
            (gpui::point(at.x, at.y + px(12.)), widgets::Open::Down)
        };
        Some(widgets::popup(
            at,
            open,
            window,
            move |w, cx| close(&(), w, cx),
            body,
        ))
    }
}
