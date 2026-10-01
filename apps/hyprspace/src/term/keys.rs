// Keystrokes to PTY bytes. Adapted from zeron's crates/ui/src/terminal/view.rs (MIT, see
// THIRD_PARTY_NOTICES.md).

use gpui::Modifiers;

/// `None` means the key isn't the terminal's and should fall through to the app.
/// `app_cursor` switches arrows, home and end from CSI to SS3 (DECCKM).
pub fn bytes(
    key: &str,
    key_char: Option<&str>,
    mods: &Modifiers,
    app_cursor: bool,
) -> Option<Vec<u8>> {
    // The Windows key and Cmd belong to the app keymap.
    if mods.platform {
        return None;
    }
    if mods.alt {
        let inner = bytes(
            key,
            key_char,
            &Modifiers {
                alt: false,
                ..*mods
            },
            app_cursor,
        )?;
        let mut out = vec![0x1b];
        out.extend(inner);
        return Some(out);
    }
    if mods.control {
        return control(key);
    }
    let seq = |csi: &[u8], ss3: &[u8]| Some(if app_cursor { ss3 } else { csi }.to_vec());
    match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" if mods.shift => Some(b"\x1b[Z".to_vec()),
        "tab" => Some(b"\t".to_vec()),
        "escape" => Some(vec![0x1b]),
        "space" => Some(b" ".to_vec()),
        "up" => seq(b"\x1b[A", b"\x1bOA"),
        "down" => seq(b"\x1b[B", b"\x1bOB"),
        "right" => seq(b"\x1b[C", b"\x1bOC"),
        "left" => seq(b"\x1b[D", b"\x1bOD"),
        "home" => seq(b"\x1b[H", b"\x1bOH"),
        "end" => seq(b"\x1b[F", b"\x1bOF"),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        _ => {
            // Prefer the typed character: it already has shift and the keyboard layout applied.
            let text = key_char
                .filter(|c| !c.is_empty())
                .or((key.chars().count() == 1).then_some(key))?;
            Some(text.as_bytes().to_vec())
        }
    }
}

// Caret notation: ctrl+a is 0x01 and so on.
fn control(key: &str) -> Option<Vec<u8>> {
    match key {
        "space" => return Some(vec![0x00]),
        "backspace" => return Some(vec![0x08]),
        "enter" => return Some(b"\r".to_vec()),
        _ => {}
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let b = match c {
        'a'..='z' => c as u8 - b'a' + 1,
        '@' => 0x00,
        '[' => 0x1b,
        '\\' => 0x1c,
        ']' => 0x1d,
        '^' => 0x1e,
        '_' | '/' => 0x1f,
        '?' => 0x7f,
        _ => return None,
    };
    Some(vec![b])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Modifiers {
        Modifiers::default()
    }

    #[test]
    fn printable_uses_the_typed_char() {
        assert_eq!(bytes("a", Some("A"), &none(), false), Some(b"A".to_vec()));
    }

    #[test]
    fn arrows_follow_app_cursor_mode() {
        assert_eq!(bytes("up", None, &none(), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(bytes("up", None, &none(), true), Some(b"\x1bOA".to_vec()));
    }

    #[test]
    fn ctrl_c_is_etx_and_alt_prefixes_escape() {
        let ctrl = Modifiers {
            control: true,
            ..none()
        };
        assert_eq!(bytes("c", None, &ctrl, false), Some(vec![0x03]));
        let alt = Modifiers {
            alt: true,
            ..none()
        };
        assert_eq!(bytes("b", Some("b"), &alt, false), Some(b"\x1bb".to_vec()));
    }

    #[test]
    fn platform_combos_fall_through() {
        let win = Modifiers {
            platform: true,
            ..none()
        };
        assert_eq!(bytes("v", Some("v"), &win, false), None);
    }
}
