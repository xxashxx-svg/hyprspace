// GPUI rewrite spike (docs/REWRITE.md, step 1): structured Claude sessions beside terminal
// sessions in one window, chats first, laid out as a grid.
// `hyprspace [--chats N] [--terms N] [--prompt TEXT] [--launch CMD]`, default one of each.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod chat;
mod claude;
mod pty;
mod term;
mod theme;

use gpui::{
    AnyView, App, Bounds, Context, Focusable, IntoElement, Render, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};

use chat::ChatView;
use pty::PtyManager;
use term::TerminalView;

struct Args {
    chats: usize,
    terms: usize,
    prompt: String,
    /// Typed into each terminal's shell.
    launch: String,
}

fn args() -> Args {
    let mut out = Args {
        chats: 1,
        terms: 1,
        prompt: "In two short sentences, say hello and name the model you are.".into(),
        launch: "claude".into(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().unwrap_or_default();
        match flag.as_str() {
            "--chats" => out.chats = value.parse().unwrap_or(1),
            "--terms" => out.terms = value.parse().unwrap_or(1),
            "--prompt" => out.prompt = value,
            "--launch" => out.launch = value,
            _ => {}
        }
    }
    out
}

struct Root {
    panes: Vec<AnyView>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let cols = (self.panes.len() as f32).sqrt().ceil().max(1.0) as usize;
        let rows = self.panes.chunks(cols).map(|row| {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .children(row.iter().map(|pane| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_1()
                        .border_color(theme::border())
                        .child(pane.clone())
                }))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::bg())
            .font_family(".SystemUIFont")
            .text_color(theme::text())
            .children(rows)
    }
}

fn main() {
    let args = args();
    let cwd = std::env::current_dir().unwrap_or_default();
    let ptys = PtyManager::default();

    gpui_platform::application().run(move |cx: &mut App| {
        gpui_tokio::init(cx);

        let kill = ptys.clone();
        cx.on_app_quit(move |_| {
            kill.kill_all();
            async {}
        })
        .detach();
        cx.on_window_closed(|cx, _| cx.quit()).detach();

        let bounds = Bounds::centered(None, size(px(1400.), px(860.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("HyprSpace".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let mut panes = Vec::new();
            for id in 0..args.chats as u64 {
                let chat = cx.new(|cx| ChatView::new(id, &args.prompt, cwd.clone(), cx));
                panes.push(AnyView::from(chat));
            }
            let dir = cwd.to_string_lossy();
            for id in 0..args.terms as u64 {
                let ptys = ptys.clone();
                let term = cx.new(|cx| TerminalView::new(id, ptys, &dir, &args.launch, cx));
                if id == 0 {
                    window.focus(&term.focus_handle(cx), cx);
                }
                panes.push(AnyView::from(term));
            }
            cx.new(|_| Root { panes })
        })
        .expect("open the window");
        cx.activate(true);
    });
}
