// The HyprSpace binary: starts the engine, opens the window, and wires the two together.
// `hyprspace [--structured N] [--agent claude|codex] [--terms N] [--prompt TEXT] [--launch CMD]`,
// default one of each with Claude.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use hyprspace_engine::Engine;
use hyprspace_proto::Agent;
use hyprspace_ui::{Layout, Root};

fn layout(args: impl Iterator<Item = String>, cwd: PathBuf) -> Layout {
    let mut out = Layout {
        structured: 1,
        agent: Agent::Claude,
        terms: 1,
        prompt: "In two short sentences, say hello and name the model you are.".into(),
        launch: "claude".into(),
        cwd,
    };
    let mut it = args;
    while let Some(flag) = it.next() {
        let value = it.next().unwrap_or_default();
        match flag.as_str() {
            "--structured" => out.structured = value.parse().unwrap_or(1),
            "--agent" if value == "codex" => out.agent = Agent::Codex,
            "--agent" => out.agent = Agent::Claude,
            "--terms" => out.terms = value.parse().unwrap_or(1),
            "--prompt" => out.prompt = value,
            "--launch" => out.launch = value,
            _ => {}
        }
    }
    out
}

fn main() {
    // SAFETY: first thing in main, before the engine or GPUI start any thread.
    unsafe { hyprspace_engine::env::prepare() };

    let cwd = std::env::current_dir().unwrap_or_default();
    let layout = layout(std::env::args().skip(1), cwd);
    let (engine, client, events) = Engine::start().expect("start the engine");

    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_app_quit(move |_| {
            engine.shutdown();
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
            cx.new(|cx| Root::new(layout, client, events, window, cx))
        })
        .expect("open the window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Layout {
        layout(args.iter().map(|s| s.to_string()), PathBuf::from("/w"))
    }

    #[test]
    fn defaults_to_one_of_each() {
        let l = parse(&[]);
        assert_eq!((l.structured, l.terms), (1, 1));
        assert_eq!(l.launch, "claude");
        assert_eq!(l.agent, Agent::Claude);
        assert_eq!(l.cwd, PathBuf::from("/w"));
    }

    #[test]
    fn reads_flags_and_ignores_unknown_ones() {
        let l = parse(&[
            "--terms",
            "4",
            "--structured",
            "0",
            "--launch",
            "",
            "--what",
            "x",
        ]);
        assert_eq!((l.structured, l.terms), (0, 4));
        assert_eq!(l.launch, "");
        assert_eq!(parse(&["--terms", "many"]).terms, 1);
        assert_eq!(parse(&["--agent", "codex"]).agent, Agent::Codex);
    }
}
