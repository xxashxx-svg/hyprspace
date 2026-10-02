// Opening things outside the app: a file in the user's code editor at its line (the viewer's
// "Open in editor" and paths the viewer can't show), and a space's folder in an editor or the
// system's file manager (the Open button). Only paths that exist on disk ever reach a launcher.
//
// Setting HYPRSPACE_OPEN_LOG to a file makes every launch append its command line there instead
// of running, so the app can be checked end to end without opening anyone's editor.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use hyprspace_proto::Opener;

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

/// The first of `names` in the folders of `path` (a PATH value). Windows editors install `.cmd`
/// shims, which Rust runs through cmd with its own safe quoting.
fn find_in(path: &OsStr, names: &[&str]) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".cmd", ".exe"]
    } else {
        &[""]
    };
    names.iter().find_map(|name| {
        std::env::split_paths(path).find_map(|dir| {
            exts.iter()
                .map(|ext| dir.join(format!("{name}{ext}")))
                .find(|full| full.is_file())
        })
    })
}

fn on_path(names: &[&str]) -> Option<PathBuf> {
    find_in(&std::env::var_os("PATH")?, names)
}

/// A path made absolute, without the `\\?\` that canonicalize adds on Windows (editors carry it
/// into their tab titles, and Explorer refuses it).
fn real(path: &Path) -> std::io::Result<PathBuf> {
    let full = path.canonicalize()?;
    let s = full.to_string_lossy();
    Ok(match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => full,
    })
}

/// A real file, made absolute. This is the guard against handing a launcher a URL, a flag or a
/// folder: anything that is not an existing file never reaches one.
fn checked(path: &Path) -> std::io::Result<PathBuf> {
    let full = real(path)?;
    if !full.is_file() {
        return Err(std::io::Error::other("not a file"));
    }
    Ok(full)
}

/// The same guard for a folder.
fn checked_dir(path: &Path) -> std::io::Result<PathBuf> {
    let full = real(path)?;
    if !full.is_dir() {
        return Err(std::io::Error::other("not a folder"));
    }
    Ok(full)
}

/// Starts `cmd` and lets it go: editors and file managers outlive us, and Explorer exits nonzero
/// even when it worked.
fn launch(mut cmd: Command) -> std::io::Result<()> {
    if let Some(log) = std::env::var_os("HYPRSPACE_OPEN_LOG") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)?;
        return writeln!(f, "{}", shown(&cmd));
    }
    cmd.spawn().map(drop)
}

/// A command line as text, for the open log and for tests.
fn shown(cmd: &Command) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| a.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn open_file(path: &Path, line: Option<u32>, col: Option<u32>) -> std::io::Result<()> {
    let path = checked(path)?;
    if !is_media(&path)
        && let Some(editor) = on_path(EDITORS)
    {
        let mut cmd = Command::new(editor);
        cmd.arg("--goto").arg(goto(&path, line, col));
        crate::util::no_window(&mut cmd);
        return launch(cmd);
    }
    launch(files_command(&path))
}

/// The system's file manager on `path`: a folder opens, a file opens in its default app.
fn files_command(path: &Path) -> Command {
    let mut cmd = Command::new(if cfg!(windows) { "explorer" } else { "open" });
    cmd.arg(path);
    cmd
}

/// The editor's command line tool, and its app's name for macOS's `open -a`.
fn editor(opener: Opener) -> Option<(&'static str, &'static str)> {
    match opener {
        Opener::VsCode => Some(("code", "Visual Studio Code")),
        Opener::Cursor => Some(("cursor", "Cursor")),
        Opener::Files => None,
    }
}

/// Where an editor is installed, if it is. On macOS an app launched from the Dock gets no shell
/// PATH, so the app bundle is what counts there, the way the Tauri app asked Launch Services.
fn installed(opener: Opener) -> Option<PathBuf> {
    let (cli, app) = editor(opener)?;
    if cfg!(target_os = "macos") {
        let home = crate::home_dir();
        [PathBuf::from("/Applications"), home.join("Applications")]
            .into_iter()
            .map(|d| d.join(format!("{app}.app")))
            .find(|p| p.is_dir())
    } else {
        on_path(&[cli])
    }
}

/// The apps that can open a folder on this machine, editors first.
pub fn openers() -> Vec<Opener> {
    Opener::ALL
        .into_iter()
        .filter(|o| *o == Opener::Files || installed(*o).is_some())
        .collect()
}

/// The command that opens `dir` in `opener`, given where the opener is installed.
fn folder_command(opener: Opener, at: Option<&Path>, dir: &Path) -> Option<Command> {
    let Some((_, app)) = editor(opener) else {
        return Some(files_command(dir));
    };
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.args(["-a", app]);
        c
    } else {
        let mut c = Command::new(at?);
        crate::util::no_window(&mut c);
        c
    };
    cmd.arg(dir);
    Some(cmd)
}

/// Opens a folder in an editor or the file manager. Errors are user-facing.
pub fn open_in(opener: Opener, path: &Path) -> Result<(), String> {
    let dir = checked_dir(path).map_err(|_| format!("{} is not a folder.", path.display()))?;
    let at = installed(opener);
    let cmd = folder_command(opener, at.as_deref(), &dir)
        .ok_or_else(|| format!("{} is not installed.", opener.name()))?;
    launch(cmd).map_err(|e| format!("Could not open {}: {e}", opener.name()))
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
        assert!(checked_dir(dir.path()).is_ok());
        assert!(checked_dir(&file).is_err());
        assert!(checked_dir(Path::new("--help")).is_err());
        assert!(is_media(Path::new("shot.PNG")) && !is_media(&file));
    }

    #[test]
    fn editors_are_found_on_the_given_path() {
        let dir = tempfile::tempdir().unwrap();
        let shim = if cfg!(windows) {
            "cursor.cmd"
        } else {
            "cursor"
        };
        std::fs::write(dir.path().join(shim), "").unwrap();
        let path = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in(&path, &["code"]), None);
        assert_eq!(
            find_in(&path, &["code", "cursor"]),
            Some(dir.path().join(shim))
        );
    }

    #[test]
    fn folder_commands_name_the_app_and_the_folder() {
        let dir = Path::new("/w/app");
        let files = folder_command(Opener::Files, None, dir).unwrap();
        let fm = if cfg!(windows) { "explorer" } else { "open" };
        assert_eq!(shown(&files), format!("{fm} {}", dir.display()));

        let code = folder_command(Opener::VsCode, Some(Path::new("/bin/code")), dir).unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(shown(&code), "open -a Visual Studio Code /w/app");
        } else {
            assert_eq!(shown(&code), format!("/bin/code {}", dir.display()));
            // an editor that is not installed has no command
            assert!(folder_command(Opener::Cursor, None, dir).is_none());
        }
    }

    #[test]
    fn opening_a_missing_folder_says_so() {
        let err = open_in(Opener::Files, Path::new("/no/such/folder")).unwrap_err();
        assert!(err.contains("not a folder"), "{err}");
    }
}
