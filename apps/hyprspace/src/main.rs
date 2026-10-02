// The HyprSpace binary: starts the engine, opens the window, and wires the two together.
// `hyprspace [folder]` opens with that folder as a space, the way `code .` does.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use hyprspace_engine::Engine;
use hyprspace_ui::Root;

/// The folder to open: the first argument that is not a flag, made absolute.
fn folder(args: impl Iterator<Item = String>, cwd: PathBuf) -> Option<PathBuf> {
    let arg = args.into_iter().find(|a| !a.starts_with('-'))?;
    let path = PathBuf::from(arg);
    Some(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

/// `hyprspace agent-hook <port> <session>` and `hyprspace status-line <port> <session>`: Claude
/// runs these from a terminal session's hooks (engine/src/hooks.rs). They pass one payload to the
/// running app and exit, without opening a window.
fn hook(args: &[String]) -> bool {
    let [cmd, port, session] = args else {
        return false;
    };
    let Ok(port) = port.parse() else {
        return false;
    };
    match cmd.as_str() {
        "agent-hook" => hyprspace_engine::hooks::run_agent_hook(port, session),
        "status-line" => hyprspace_engine::hooks::run_status_line(port, session),
        _ => return false,
    }
    true
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if hook(&args) {
        return;
    }
    // SAFETY: first thing in main, before the engine or GPUI start any thread.
    unsafe { hyprspace_engine::env::prepare() };

    let cwd = std::env::current_dir().unwrap_or_default();
    let open = folder(args.into_iter(), cwd).map(|p| dunce(&p));
    let (engine, client, events) = Engine::start().expect("start the engine");

    gpui_platform::application()
        .with_assets(hyprspace_ui::Assets)
        .run(move |cx: &mut App| {
            // the bundle id the Tauri app shipped under: the installer stamps it on the Start
            // menu shortcut, so the window groups with a pinned HyprSpace on the taskbar
            cx.set_app_identity("com.hyprspace.app", "HyprSpace");
            hyprspace_ui::init(cx);
            cx.on_app_quit(move |_| {
                engine.shutdown();
                async {}
            })
            .detach();
            cx.on_window_closed(|cx, _| cx.quit()).detach();

            let bounds = Bounds::centered(None, size(px(1400.), px(860.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // the UI draws its own title row (ui/src/root/titlebar.rs)
                titlebar: Some(TitlebarOptions {
                    title: Some("HyprSpace".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.), px(13.))),
                }),
                ..Default::default()
            };
            cx.open_window(options, |window, cx| {
                cx.new(|cx| Root::new(client, events, open, window, cx))
            })
            .expect("open the window");
            cx.activate(true);
        });
}

/// `.` and `..` resolved, without the `\\?\` prefix `canonicalize` adds on Windows, which the
/// CLIs would carry into their saved paths.
fn dunce(path: &std::path::Path) -> PathBuf {
    let full = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = full.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Option<PathBuf> {
        folder(args.iter().map(|s| s.to_string()), PathBuf::from("/w"))
    }

    #[test]
    fn takes_the_first_folder_and_skips_flags() {
        assert_eq!(parse(&[]), None);
        assert_eq!(
            parse(&["--x", "app"]),
            Some(PathBuf::from("/w").join("app"))
        );
        let abs = std::env::temp_dir();
        assert_eq!(parse(&[abs.to_str().unwrap()]), Some(abs));
        assert!(dunce(std::path::Path::new(".")).is_absolute());
    }

    #[test]
    fn only_a_full_hook_call_is_a_hook() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(!hook(&args(&["agent-hook", "x", "s"])));
        assert!(!hook(&args(&["agent-hook", "1"])));
        assert!(!hook(&args(&["app", "1", "s"])));
    }
}
