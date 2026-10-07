// Copy and paste. Ctrl+C copies when there is a selection and interrupts when there is not;
// Ctrl+V, Ctrl+Shift+V and a right-click paste. Text goes in first; with no text on the clipboard
// a pasted image is saved to a temp file and its path typed in, since the agents read images by
// path (the Tauri app's TerminalPane does the same).

use gpui::{ClipboardEntry, ClipboardItem, Context, ExternalPaths, Window};

use super::TerminalView;
use crate::attach;

/// Pasted text as PTY bytes. Line breaks become carriage returns, as a typed Enter would be, and
/// a program that asked for bracketed paste gets the markers, so a multi-line paste into a
/// prompt does not run line by line. A paste can't close the bracket early.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let clean = text
        .replace("\x1b[201~", "")
        .replace("\r\n", "\r")
        .replace('\n', "\r");
    if bracketed {
        let mut out = b"\x1b[200~".to_vec();
        out.extend_from_slice(clean.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        clean.into_bytes()
    }
}

impl TerminalView {
    /// Files dropped from File Explorer or Finder: their paths typed in, the way a terminal does,
    /// and images kept so their `[Image #N]` previews before the prompt goes.
    pub(super) fn drop_paths(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = paths.paths();
        if paths.is_empty() {
            return;
        }
        window.focus(&self.focus, cx);
        let before = self.before_paste();
        let text: String = paths.iter().map(|p| attach::path_text(p)).collect();
        self.paste_text(&text, cx);
        for p in paths.iter().filter(|p| attach::is_image(p)) {
            self.pasted(p.clone(), before.clone(), cx);
        }
    }

    /// Copies the selection. False when nothing is selected.
    pub(super) fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.emu.selection_text() else {
            return false;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    pub(super) fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        match item.text().filter(|t| !t.is_empty()) {
            Some(text) => self.paste_text(&text, cx),
            None => {
                self.paste_image(&item, cx);
            }
        }
    }

    /// Alt+V: the image on the clipboard, or the key itself for a program that binds it.
    pub(super) fn paste_image_or_key(&mut self, cx: &mut Context<Self>) {
        let pasted = cx
            .read_from_clipboard()
            .is_some_and(|item| self.paste_image(&item, cx));
        if !pasted {
            self.input(b"\x1bv".to_vec(), cx);
        }
    }

    fn paste_image(&mut self, item: &ClipboardItem, cx: &mut Context<Self>) -> bool {
        let saved = item.entries().iter().find_map(|e| match e {
            ClipboardEntry::Image(image) => attach::save(image).ok(),
            _ => None,
        });
        match saved {
            Some(path) => {
                let before = self.before_paste();
                self.paste_text(&attach::path_text(&path), cx);
                self.pasted(path, before, cx);
                true
            }
            None => false,
        }
    }

    pub(super) fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let bytes = paste_bytes(text, self.emu.bracketed_paste());
        self.input(bytes, cx);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn pastes_are_bracketed_when_asked_and_cannot_break_out() {
        assert_eq!(paste_bytes("a\nb", false), b"a\rb");
        assert_eq!(
            paste_bytes("x\x1b[201~rm -rf\r\n", true),
            b"\x1b[200~xrm -rf\r\x1b[201~"
        );
    }
}
