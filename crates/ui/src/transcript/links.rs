// A reply's links: a file it names opens in the viewer at its line, a web address in the browser.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{App, Context, Window};
use hyprspace_proto::Pane;

use super::{TranscriptEvent, TranscriptView};
use crate::markdown::links::Links;

/// How long a file's being there is trusted before it is looked at again.
const FRESH: Duration = Duration::from_secs(2);

thread_local! {
    static FILES: RefCell<HashMap<PathBuf, (bool, Instant)>> = RefCell::new(HashMap::new());
}

fn is_file(path: &Path) -> bool {
    FILES.with(|f| {
        let mut f = f.borrow_mut();
        if let Some((there, at)) = f.get(path)
            && at.elapsed() < FRESH
        {
            return *there;
        }
        let there = path.is_file();
        f.insert(path.to_path_buf(), (there, Instant::now()));
        there
    })
}

/// The file `text` names, from `cwd`, and the line and column it points at: `src/a.rs:12`,
/// `src/a.rs:12:4`, `src/a.rs#L12` or a `file://` address.
pub fn target(cwd: &Path, text: &str) -> Option<(PathBuf, Option<u32>, Option<u32>)> {
    let text = text.trim().trim_start_matches('<').trim_end_matches('>');
    let text = text
        .strip_prefix("file:///")
        .or_else(|| text.strip_prefix("file://"))
        .unwrap_or(text);
    if text.is_empty()
        || text.len() > 400
        || text.contains("://")
        || text.starts_with("mailto:")
        || text.starts_with('#')
        || text.contains('\n')
    {
        return None;
    }
    let text = text.replace("%20", " ");
    let (mut path, fragment) = match text.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (text.as_str(), None),
    };
    let (mut line, mut col) = (None, None);
    if let Some(f) = fragment.and_then(|f| f.strip_prefix('L')) {
        let digits = |s: &str| {
            s.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        };
        line = digits(f).parse().ok();
        col = f.split_once('C').and_then(|(_, c)| digits(c).parse().ok());
    }
    for _ in 0..2 {
        match path.rsplit_once(':') {
            Some((head, tail))
                if !head.is_empty()
                    && !tail.is_empty()
                    && tail.chars().all(|c| c.is_ascii_digit()) =>
            {
                col = line;
                line = tail.parse().ok();
                path = head;
            }
            _ => break,
        }
    }
    let p = Path::new(path);
    let full = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    is_file(&full).then_some((full, line, col))
}

/// What a click on a link in this thread's replies does.
pub(super) fn handler(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Rc<Links> {
    let cwd = v.launch.cwd.clone();
    let local = !v.remote;
    let me = cx.entity().downgrade();
    let file = {
        let cwd = cwd.clone();
        Rc::new(move |s: &str| {
            local
                .then(|| target(&cwd, s))
                .flatten()
                .map(|(path, ..)| path)
        })
    };
    let open = {
        let me = me.clone();
        Rc::new(move |url: &str, _: &mut Window, cx: &mut App| {
            if let Some((path, line, col)) = local.then(|| target(&cwd, url)).flatten() {
                let pane = Pane::File { path, line, col };
                let _ = me.update(cx, |_, cx| cx.emit(TranscriptEvent::Open(pane)));
            } else if url.contains("://") || url.starts_with("mailto:") {
                cx.open_url(url);
            }
        })
    };
    let show = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
        let _ = me.update(cx, |_, cx| cx.emit(TranscriptEvent::ShowMedia(path)));
    });
    Rc::new(Links { open, file, show })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_file_opens_at_its_line() {
        let dir = std::env::temp_dir().join(format!("hs-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src").join("a.rs"), "fn a() {}").unwrap();
        let a = dir.join("src").join("a.rs");
        assert_eq!(target(&dir, "src/a.rs"), Some((a.clone(), None, None)));
        assert_eq!(
            target(&dir, "src/a.rs:12"),
            Some((a.clone(), Some(12), None))
        );
        assert_eq!(
            target(&dir, "src/a.rs:12:4"),
            Some((a.clone(), Some(12), Some(4)))
        );
        assert_eq!(
            target(&dir, "src/a.rs#L7"),
            Some((a.clone(), Some(7), None))
        );
        let absolute = a.to_string_lossy().to_string();
        assert_eq!(
            target(&dir, &format!("{absolute}:3")),
            Some((a, Some(3), None))
        );
        assert_eq!(target(&dir, "src/missing.rs"), None);
        assert_eq!(target(&dir, "cargo test"), None);
        assert_eq!(target(&dir, "https://x.dev/src/a.rs"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
