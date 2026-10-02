// Process environment fixes the app makes once at startup, before any thread starts, so every
// child it spawns (shells, CLIs, git) inherits them. Copied from src-tauri/src/lib.rs.

use hyprspace_harness::SESSION_ENV;

/// Adopt the user's real PATH and drop a parent Claude session's markers.
///
/// # Safety
/// Mutates the process environment, which is only sound while no other thread reads it. Call it
/// first thing in `main`, before the engine or the UI starts.
pub unsafe fn prepare() {
    let path = rebuilt_path();
    // SAFETY: the caller guarantees no other thread is running yet.
    unsafe {
        if let Some(path) = path {
            std::env::set_var("PATH", path);
        }
        drop_claude_session_env();
    }
    take_ctrl_c();
}

// A process started with Ctrl+C ignored (CREATE_NEW_PROCESS_GROUP, which tools that launch
// detached children use) passes that on to everything it spawns, so Ctrl+C in a terminal session
// would reach PowerShell's prompt but never stop the command running under it. Undo it here, so
// the shells inherit normal Ctrl+C however the app was started.
#[cfg(windows)]
fn take_ctrl_c() {
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }
    // SAFETY: a null handler with FALSE only clears the process's ignore-Ctrl+C flag.
    unsafe {
        SetConsoleCtrlHandler(None, 0);
    }
}

#[cfg(not(windows))]
fn take_ctrl_c() {}

// When HyprSpace is started from inside a Claude Code session (a dev build run by an agent, or the
// app opened from a claude terminal), it inherits that session's own markers, and every session
// would pass them on. Claude then treats each one as a sub-session of the one that launched us:
// it turns transcript saving off (so nothing can be resumed, and [Image #N] can't be read back),
// and each gets that session's id and messaging token. Sessions are independent, so drop them.
unsafe fn drop_claude_session_env() {
    for k in SESSION_ENV {
        // SAFETY: see `prepare`.
        unsafe { std::env::remove_var(k) };
    }
}

// Windows: an app started by the installer, the updater's relaunch, or a browser's download bar
// inherits a trimmed PATH (system dirs and little else), so `git`, `code` and the agent CLIs
// vanish for every child process, terminals included. Rebuild it the way a fresh login does:
// the machine PATH, then the user PATH, from the registry, appended to whatever we were given.
#[cfg(windows)]
fn rebuilt_path() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let read = |hive, sub: &str| -> String {
        RegKey::predef(hive)
            .open_subkey(sub)
            .and_then(|k| k.get_value::<String, _>("Path"))
            .unwrap_or_default()
    };
    let machine = read(
        HKEY_LOCAL_MACHINE,
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    );
    let user = read(HKEY_CURRENT_USER, "Environment");
    let current = std::env::var("PATH").unwrap_or_default();
    Some(merge_windows(&current, &machine, &user, |k| {
        std::env::var(k).ok()
    }))
}

#[cfg(any(windows, test))]
fn merge_windows(
    current: &str,
    machine: &str,
    user: &str,
    var: impl Fn(&str) -> Option<String>,
) -> String {
    let mut merged: Vec<String> = current
        .split(';')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    for entry in machine.split(';').chain(user.split(';')) {
        let entry = expand_env(entry.trim(), &var);
        if entry.is_empty() || merged.iter().any(|m| m.eq_ignore_ascii_case(&entry)) {
            continue;
        }
        merged.push(entry);
    }
    merged.join(";")
}

// registry PATH entries are REG_EXPAND_SZ, so `%SystemRoot%\system32` style references are literal
#[cfg(any(windows, test))]
fn expand_env(s: &str, var: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('%') else {
            out.push_str(&rest[i..]);
            return out;
        };
        match var(&after[..j]) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[i..=i + 1 + j]),
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    out
}

// macOS GUI launches (Finder, Dock) hand the app a minimal PATH with no Homebrew, npm globals or
// nvm, so `claude` and `codex` look "not installed" even when they're there. Resolve the user's
// real login-shell PATH once and adopt it.
#[cfg(not(windows))]
fn rebuilt_path() -> Option<String> {
    use std::process::Command;
    use std::sync::mpsc;
    use std::time::Duration;

    // $SHELL is set for any normal desktop session; zsh is macOS's default since Catalina
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let (tx, rx) = mpsc::channel();
    // This thread only runs the shell; it never touches the environment.
    std::thread::spawn(move || {
        // -ilc so both the login file (.zprofile, brew) and the interactive rc (.zshrc, nvm) get
        // sourced. The markers fence our value off from any banner the rc files might print.
        let out = Command::new(&shell)
            .args(["-ilc", "printf '__HP__%s__HP__' \"$PATH\""])
            .output();
        let _ = tx.send(out);
    });
    // a slow or wedged rc file shouldn't stall launch, so give up after a few seconds
    let out = rx.recv_timeout(Duration::from_secs(5)).ok()?.ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let resolved = match (s.find("__HP__"), s.rfind("__HP__")) {
        (Some(a), Some(b)) if b > a => s[a + 6..b].trim(),
        _ => return None,
    };
    if resolved.is_empty() {
        return None;
    }
    Some(merge_unix(
        resolved,
        &std::env::var("PATH").unwrap_or_default(),
    ))
}

// keep whatever we already had too, in case the shell PATH somehow drops a system dir
#[cfg(any(not(windows), test))]
fn merge_unix(resolved: &str, current: &str) -> String {
    let mut seen: std::collections::HashSet<&str> = resolved.split(':').collect();
    let mut merged = resolved.to_string();
    for p in current.split(':') {
        if !p.is_empty() && seen.insert(p) {
            merged.push(':');
            merged.push_str(p);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(k: &str) -> Option<String> {
        (k == "SystemRoot").then(|| r"C:\Windows".to_string())
    }

    #[test]
    fn expands_known_vars_and_keeps_unknown_ones() {
        assert_eq!(
            expand_env(r"%SystemRoot%\system32", vars),
            r"C:\Windows\system32"
        );
        assert_eq!(expand_env(r"%Nope%\x", vars), r"%Nope%\x");
        assert_eq!(expand_env("50% off", vars), "50% off");
    }

    #[test]
    fn windows_merge_appends_registry_entries_once() {
        let merged = merge_windows(
            r"C:\Windows\system32;C:\keep",
            r"%SystemRoot%\system32;C:\Program Files\Git\cmd",
            r"C:\Users\a\.local\bin;c:\keep",
            vars,
        );
        assert_eq!(
            merged,
            r"C:\Windows\system32;C:\keep;C:\Program Files\Git\cmd;C:\Users\a\.local\bin"
        );
    }

    #[test]
    fn unix_merge_puts_the_login_path_first() {
        assert_eq!(
            merge_unix("/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin"),
            "/opt/homebrew/bin:/usr/bin:/bin"
        );
    }

    #[test]
    fn session_markers_include_the_nesting_flag() {
        assert!(SESSION_ENV.contains(&"CLAUDECODE"));
        assert!(
            !SESSION_ENV
                .iter()
                .any(|k| k.starts_with("CLAUDE_CODE_USE_"))
        );
    }
}
