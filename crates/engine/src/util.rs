// Small helpers several engine modules share.

use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use serde_json::Value;

/// The user's home folder, or an empty path when neither variable is set.
pub fn home_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default(),
    )
}

pub fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// A GUI app spawning a console program flashes a console window without this.
pub fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// "pro" -> "Pro"
pub fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The unverified payload of a JWT. Only display claims are read from it; it is never trusted
/// and never used for auth.
pub fn decode_jwt(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Modified time in unix ms, 0 when unknown.
pub fn mtime_ms(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_case_capitalizes_the_first_letter() {
        assert_eq!(title_case("pro"), "Pro");
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn decodes_a_jwt_payload_without_checking_it() {
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#);
        let token = format!("x.{payload}.sig");
        assert_eq!(decode_jwt(&token).unwrap()["email"], "a@b.c");
        assert!(decode_jwt("garbage").is_none());
    }
}
