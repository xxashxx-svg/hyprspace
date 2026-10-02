// Copy and paste. Ctrl+C copies when there is a selection and interrupts when there is not;
// Ctrl+V, Ctrl+Shift+V and a right-click paste. Text goes in first; with no text on the clipboard
// a pasted image is saved to a temp file and its path typed in, since the agents read images by
// path (the Tauri app's TerminalPane does the same).

use gpui::{ClipboardEntry, ClipboardItem, Context};

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

/// What gets typed for a pasted image: its path, quoted when it holds a space (Windows profile
/// folders often do) so the agent reads it as one argument, then a space to keep typing after.
/// Forward slashes on Windows: the CLIs take them, and a bash in the pane won't eat them.
pub fn image_text(path: &std::path::Path) -> String {
    let mut p = path.display().to_string();
    if cfg!(windows) {
        p = p.replace('\\', "/");
    }
    if p.contains(' ') {
        format!("\"{p}\" ")
    } else {
        format!("{p} ")
    }
}

impl TerminalView {
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
                self.paste_text(&image_text(&path), cx);
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
    use std::path::Path;

    use super::*;

    #[test]
    fn pastes_are_bracketed_when_asked_and_cannot_break_out() {
        assert_eq!(paste_bytes("a\nb", false), b"a\rb");
        assert_eq!(
            paste_bytes("x\x1b[201~rm -rf\r\n", true),
            b"\x1b[200~xrm -rf\r\x1b[201~"
        );
    }

    #[test]
    fn image_paths_stay_one_argument() {
        assert_eq!(image_text(Path::new("/tmp/a.png")), "/tmp/a.png ");
        assert_eq!(
            image_text(Path::new("C:/Users/First Last/a.png")),
            "\"C:/Users/First Last/a.png\" "
        );
    }
}
