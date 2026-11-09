// A file a reply names, as a link or a path in code, shows as its icon and its own name, and still
// opens the whole path. Find and the renderer both read text through here, so their matches agree.

use super::{Inline, links};

/// Room for the file's icon. Figure spaces don't break, so the icon stays on its name's row.
pub const GAP: &str = "\u{2007}\u{2007}";
/// Air inside a code chip, on each side of its text.
pub const PAD: char = '\u{202F}';

/// `i` as drawn, and where each file's icon goes with the name it's for.
pub fn label(i: &Inline) -> (Inline, Vec<(usize, String)>) {
    let h = links::current();
    let mut out = Inline::default();
    let mut icons = Vec::new();
    let mut at = 0;
    for (r, s) in &i.spans {
        out.text.push_str(&i.text[at..r.start]);
        let words = &i.text[r.clone()];
        let start = out.text.len();
        let mut s = s.clone();
        if (s.code || s.link.is_some()) && h.as_ref().is_some_and(|h| (h.file)(words).is_some()) {
            let name = short(words);
            icons.push((start, name.to_string()));
            out.text.push_str(GAP);
            out.text.push_str(name);
            s.code = false;
            s.link.get_or_insert_with(|| words.to_string());
        } else if s.code && s.link.is_none() {
            out.text.push(PAD);
            out.text.push_str(words);
            out.text.push(PAD);
        } else {
            out.text.push_str(words);
        }
        out.spans.push((start..out.text.len(), s));
        at = r.end;
    }
    out.text.push_str(&i.text[at..]);
    (out, icons)
}

fn short(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_shows_its_own_name() {
        assert_eq!(short("crates/ui/src/mod.rs:12"), "mod.rs:12");
        assert_eq!(short(r"C:\a\b.rs"), "b.rs");
        assert_eq!(short("file:///C:/x/y.kt"), "y.kt");
        assert_eq!(short("a.rs"), "a.rs");
    }
}
