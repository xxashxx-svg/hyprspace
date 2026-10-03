// Keystrokes to PTY bytes. Plain typing does not come through here: it arrives as text through
// the input handler, which is also how IME and dead keys reach the terminal (see `is_text`).
// Adapted from zeron's crates/ui/src/terminal/view.rs (MIT, see THIRD_PARTY_NOTICES.md).

use gpui::Modifiers;

/// Whether the platform will deliver this key as typed text. Ctrl+Alt is AltGr on Windows
/// keyboards (`@` on a German layout), so a char typed with both is text too. On macOS Option
/// types the layout's characters, as Terminal.app does unless told to use it as Meta: a German
/// Mac types `@` with Option+L. Option+arrows and Option+Enter carry no typed char, so they still
/// reach the shell and the agent as Meta.
pub fn is_text(key_char: Option<&str>, mods: &Modifiers) -> bool {
    let typed = key_char.is_some_and(|c| !c.is_empty() && !c.chars().any(char::is_control));
    let option = cfg!(target_os = "macos") && !mods.control;
    typed && !mods.platform && (mods.control == mods.alt || option)
}

/// xterm's modifier parameter: 1 plus shift 1, alt 2, ctrl 4.
fn modifier_param(mods: &Modifiers) -> u8 {
    1 + mods.shift as u8 + 2 * mods.alt as u8 + 4 * mods.control as u8
}

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
    // arrows and friends with a modifier: CSI 1 ; m X
    let letter = match key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(l) = letter
        && (mods.control || mods.alt || mods.shift)
    {
        return Some(format!("\x1b[1;{}{l}", modifier_param(mods)).into_bytes());
    }
    if mods.alt && !mods.control {
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
        "insert" => Some(b"\x1b[2~".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        "f1" => Some(b"\x1bOP".to_vec()),
        "f2" => Some(b"\x1bOQ".to_vec()),
        "f3" => Some(b"\x1bOR".to_vec()),
        "f4" => Some(b"\x1bOS".to_vec()),
        "f5" => Some(b"\x1b[15~".to_vec()),
        "f6" => Some(b"\x1b[17~".to_vec()),
        "f7" => Some(b"\x1b[18~".to_vec()),
        "f8" => Some(b"\x1b[19~".to_vec()),
        "f9" => Some(b"\x1b[20~".to_vec()),
        "f10" => Some(b"\x1b[21~".to_vec()),
        "f11" => Some(b"\x1b[23~".to_vec()),
        "f12" => Some(b"\x1b[24~".to_vec()),
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
        "delete" => return Some(b"\x1b[3;5~".to_vec()),
        _ => {}
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let b = match c {
        'a'..='z' => c as u8 - b'a' + 1,
        '@' | '2' => 0x00,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '/' | '7' => 0x1f,
        '?' | '8' => 0x7f,
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
    fn typed_text_goes_through_the_input_handler() {
        assert!(is_text(Some("a"), &none()));
        assert!(is_text(
            Some("é"),
            &Modifiers {
                shift: true,
                ..none()
            }
        ));
        // AltGr
        assert!(is_text(
            Some("@"),
            &Modifiers {
                control: true,
                alt: true,
                ..none()
            }
        ));
        let ctrl = Modifiers {
            control: true,
            ..none()
        };
        assert!(!is_text(Some("c"), &ctrl));
        assert!(!is_text(None, &none()));
        // Option types characters on macOS, and is Meta elsewhere
        let alt = Modifiers {
            alt: true,
            ..none()
        };
        assert_eq!(is_text(Some("@"), &alt), cfg!(target_os = "macos"));
        assert!(!is_text(Some("\r"), &alt));
    }

    #[test]
    fn printable_uses_the_typed_char() {
        assert_eq!(bytes("a", Some("A"), &none(), false), Some(b"A".to_vec()));
    }

    #[test]
    fn arrows_follow_app_cursor_mode_and_modifiers() {
        assert_eq!(bytes("up", None, &none(), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(bytes("up", None, &none(), true), Some(b"\x1bOA".to_vec()));
        let ctrl = Modifiers {
            control: true,
            ..none()
        };
        assert_eq!(
            bytes("left", None, &ctrl, false),
            Some(b"\x1b[1;5D".to_vec())
        );
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
        assert_eq!(
            bytes("f5", None, &none(), false),
            Some(b"\x1b[15~".to_vec())
        );
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
