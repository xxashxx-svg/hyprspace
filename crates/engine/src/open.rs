// Opening a file outside the app, for a path ctrl+clicked in a terminal. Until the app has its own
// file viewer (REWRITE.md phase 6), code goes to the user's editor at its line, and anything else
// (an image, a PDF) to the OS default.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Editors that take `--goto file:line:col`, in the order they are tried. The Tauri app offered
/// the same two.
const EDITORS: &[&str] = &["code", "cursor"];

/// Files an editor is the wrong place for.
const MEDIA: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico", "pdf", "mp4", "mov", "webm", "mp3",
    "wav",
];

fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
}

/// `path:line:col` for `--goto`, with whatever position is known.
fn goto(path: &Path, line: Option<u32>, col: Option<u32>) -> String {
    let mut out = path.display().to_string();
    if let Some(l) = line {
        out.push_str(&format!(":{l}"));
        if let Some(c) = col {
            out.push_str(&format!(":{c}"));
        }
    }
    out
}

/// The first of `names` on PATH. Windows editors install `.cmd` shims, which Rust runs through
/// cmd with its own safe quoting.
fn on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: &[&str] = if cfg!(windows) {
        &[".cmd", ".exe"]
    } else {
        &[""]
    };
    for name in names {
        for dir in std::env::split_paths(&path) {
            for ext in exts {
                let full = dir.join(format!("{name}{ext}"));
                if full.is_file() {
                    return Some(full);
                }
            }
        }
    }
    None
}

/// A real file, made absolute. This is the guard against handing a launcher a URL, a flag or a
/// folder: anything that is not an existing file never reaches one.
fn checked(path: &Path) -> std::io::Result<PathBuf> {
    let full = path.canonicalize()?;
    if !full.is_file() {
        return Err(std::io::Error::other("not a file"));
    }
    let s = full.to_string_lossy();
    // canonicalize adds \\?\ on Windows, which editors carry into their tab titles
    Ok(match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => full,
    })
}

pub fn open_file(path: &Path, line: Option<u32>, col: Option<u32>) -> std::io::Result<()> {
    let path = checked(path)?;
    if !is_media(&path)
        && let Some(editor) = on_path(EDITORS)
    {
        let mut cmd = Command::new(editor);
        cmd.arg("--goto").arg(goto(&path, line, col));
        crate::util::no_window(&mut cmd).spawn()?;
        return Ok(());
    }
    system_open(&path)
}

#[cfg(windows)]
fn system_open(path: &Path) -> std::io::Result<()> {
    // explorer hands a file to its default app; it exits nonzero even on success, so no wait
    Command::new("explorer").arg(path).spawn().map(drop)
}

#[cfg(not(windows))]
fn system_open(path: &Path) -> std::io::Result<()> {
    Command::new("open").arg(path).spawn().map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goto_carries_what_is_known() {
        let p = Path::new("src/a.rs");
        assert_eq!(goto(p, None, Some(4)), "src/a.rs");
        assert_eq!(goto(p, Some(12), None), "src/a.rs:12");
        assert_eq!(goto(p, Some(12), Some(4)), "src/a.rs:12:4");
    }

    #[test]
    fn only_existing_files_get_through() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "x").unwrap();
        assert!(checked(&file).unwrap().is_absolute());
        assert!(checked(dir.path()).is_err());
        assert!(checked(Path::new("https://example.com")).is_err());
        assert!(checked(&dir.path().join("missing.rs")).is_err());
        assert!(is_media(Path::new("shot.PNG")) && !is_media(&file));
    }
}
