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
    "brand/claude.svg",
    "brand/openai.svg",
);

/// DM Sans as static weights cut from the Tauri app's variable font, and its JetBrains Mono
/// Nerd Font, converted from woff2 since GPUI loads TrueType.
const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/DMSans-Regular.ttf"),
    include_bytes!("../assets/fonts/DMSans-Medium.ttf"),
    include_bytes!("../assets/fonts/DMSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/DMSans-Bold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Bold.ttf"),
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

/// An agent's mark in `color`, usually its brand color.
pub fn mark(agent: Agent, size: f32, color: Hsla) -> Svg {
    let file = match agent {
        Agent::Claude => "brand/claude.svg",
        Agent::Codex => "brand/openai.svg",
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
        for (path, bytes) in FILES {
            assert!(
                std::str::from_utf8(bytes).unwrap().contains("<svg"),
                "{path}"
            );
            assert!(Assets.load(path).unwrap().is_some());
        }
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert_eq!(Assets.list("brand/").unwrap().len(), 2);
    }

    #[test]
    fn fonts_are_truetype() {
        for font in FONTS {
            assert_eq!(&font[..4], &[0, 1, 0, 0]);
        }
    }
}
