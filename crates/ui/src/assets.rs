// The images the UI draws, compiled in: Lucide icons (ISC) and the agents' marks, the same ones
// the Tauri app shows. GPUI tints an `svg()` with the text color, so one file serves every theme.

use std::borrow::Cow;

use gpui::{AssetSource, Hsla, SharedString, Svg, prelude::*, px, svg};
use hyprspace_proto::Agent;

macro_rules! files {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../assets/", $path)))),*]
    };
}

const FILES: &[(&str, &[u8])] = files!(
    "icons/search.svg",
    "icons/paperclip.svg",
    "icons/arrow-down.svg",
    "icons/square-pen.svg",
    "icons/plus.svg",
    "icons/chevron-right.svg",
    "icons/chevron-down.svg",
    "icons/trash-2.svg",
    "icons/pencil.svg",
    "icons/archive-restore.svg",
    "icons/palette.svg",
    "icons/arrow-up.svg",
    "icons/folder.svg",
    "icons/folder-open.svg",
    "icons/settings.svg",
    "icons/archive.svg",
    "icons/terminal.svg",
    "icons/image-plus.svg",
    "icons/x.svg",
    "icons/check.svg",
    "icons/git-branch.svg",
    "icons/circle-alert.svg",
    "icons/square.svg",
    "icons/sparkles.svg",
    "icons/circle-check.svg",
    "icons/ring.svg",
    "icons/sliders-horizontal.svg",
    "icons/bot.svg",
    "icons/arrow-left.svg",
    "icons/sun.svg",
    "icons/moon.svg",
    "icons/monitor.svg",
    "icons/list-checks.svg",
    "icons/hand.svg",
    "icons/file-pen-line.svg",
    "icons/shield-off.svg",
    "brand/claude.svg",
    "brand/openai.svg",
    "brand/gemini.svg",
    // panes, the dock, the viewer and the Open button
    "icons/grip-vertical.svg",
    "icons/layout-grid.svg",
    "icons/panel-left.svg",
    "icons/panel-right.svg",
    "icons/folder-tree.svg",
    "icons/refresh-cw.svg",
    "icons/file.svg",
    "icons/file-code.svg",
    "icons/file-diff.svg",
    "icons/external-link.svg",
    "brand/vscode.svg",
    "brand/cursor.svg",
    "brand/finder.svg",
    "brand/explorer.png",
    // the usage meter, the command palette, Settings' Usage and Skills, and the intro
    "icons/gauge.svg",
    "icons/zap.svg",
    "icons/maximize-2.svg",
    "icons/square-terminal.svg",
    "icons/text-search.svg",
    "icons/rotate-cw.svg",
    "icons/rotate-ccw.svg",
    "icons/clock.svg",
    "icons/triangle-alert.svg",
    "icons/square-slash.svg",
    "icons/arrow-right.svg",
    "icons/folder-plus.svg",
    "icons/copy.svg",
    "icons/mouse-pointer-click.svg",
    "brand/opencode.svg",
    "brand/grok.svg",
    // updates, and the logo's three faces (Logo.tsx), each tinted on its own
    "icons/download.svg",
    "icons/circle-play.svg",
    "logo/top.svg",
    "logo/left.svg",
    "logo/right.svg",
    // Settings: the Shortcuts and About tabs, and the terminal font size stepper
    "icons/keyboard.svg",
    "icons/info.svg",
    "icons/minus.svg",
    "icons/ellipsis.svg",
    // the title row's caption buttons on Windows
    "caption/min.svg",
    "caption/max.svg",
    "caption/restore.svg",
    "caption/close.svg",
);

/// Geist and Geist Mono as zeron ships them, and the Tauri app's JetBrains Mono Nerd Font for
/// the terminal, converted from woff2 since GPUI loads TrueType.
const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Geist.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Geist-Bold.ttf"),
    include_bytes!("../assets/fonts/Geist-Italic.ttf"),
    include_bytes!("../assets/fonts/GeistMono.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../assets/fonts/GeistMono-SemiBold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Bold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Italic.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Bold.ttf"),
    // agents print italics (Claude's recaps); without the face they fall back to upright
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Italic.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-BoldItalic.ttf"),
];

/// Hands the bundled fonts to GPUI. A font that fails to load falls back to the system's.
pub fn load_fonts(cx: &mut gpui::App) {
    let fonts = FONTS.iter().map(|f| Cow::Borrowed(*f)).collect();
    let _ = cx.text_system().add_fonts(fonts);
}

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

/// A Lucide icon by name, `size` pixels square. GPUI paints an svg in its own text color, never
/// an inherited one, so the color comes with the call.
pub fn icon(name: &str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// The HyprSpace cube, `size` pixels square: the accent on top, the text colors on its sides,
/// like the Tauri app's Logo.tsx.
pub fn logo(size: f32) -> gpui::Div {
    let face = |name: &str, color: Hsla| {
        svg()
            .path(SharedString::from(format!("logo/{name}.svg")))
            .absolute()
            .size(px(size))
            .text_color(color)
    };
    gpui::div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(face("top", crate::colors::accent()))
        .child(face("left", crate::colors::text1()))
        .child(face("right", crate::colors::text3()))
}

/// A provider's mark by its CLI name, for the agents the app can't start yet (OpenCode, Grok)
/// as well as the ones it can.
pub fn provider_mark(id: &str, size: f32, color: Hsla) -> Option<Svg> {
    let file = match id {
        "claude" => "brand/claude.svg",
        "codex" => "brand/openai.svg",
        "gemini" => "brand/gemini.svg",
        "opencode" => "brand/opencode.svg",
        "grok" => "brand/grok.svg",
        _ => return None,
    };
    Some(
        svg()
            .path(file)
            .size(px(size))
            .flex_none()
            .text_color(color),
    )
}

/// An agent's mark in `color`, usually its brand color.
pub fn mark(agent: Agent, size: f32, color: Hsla) -> Svg {
    let file = match agent {
        Agent::Claude => "brand/claude.svg",
        Agent::Codex => "brand/openai.svg",
        Agent::Gemini => "brand/gemini.svg",
    };
    svg()
        .path(file)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_an_svg_and_loads() {
        for (path, bytes) in FILES.iter().filter(|(p, _)| p.ends_with(".svg")) {
            assert!(
                std::str::from_utf8(bytes).unwrap().contains("<svg"),
                "{path}"
            );
            assert!(Assets.load(path).unwrap().is_some());
        }
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert_eq!(Assets.list("brand/").unwrap().len(), 9);
    }

    #[test]
    fn fonts_are_truetype() {
        for font in FONTS {
            assert_eq!(&font[..4], &[0, 1, 0, 0]);
        }
    }
}
