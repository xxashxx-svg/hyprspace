// Finding what is clickable in a line of terminal output: URLs, and file paths with an optional
// `:line:col` the way compilers print them (`src/main.rs:12:5`, `C:\a\b.rs:3`). Pure text in,
// char ranges out; the view checks a path exists before it underlines it.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Url(String),
    File {
        path: String,
        line: Option<u32>,
        col: Option<u32>,
    },
}

/// A link and the chars it covers in the line, `start..end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub start: usize,
    pub end: usize,
    pub target: Target,
}

const SCHEMES: &[&str] = &["https://", "http://", "file://"];
// what a bare path or URL can't hold
const STOP: &[char] = &['"', '\'', '<', '>', '`', '|', '\u{2502}'];

/// The link covering char `at` of `text`, if there is one.
pub fn at(text: &str, at: usize) -> Option<Link> {
    all(text)
        .into_iter()
        .find(|l| (l.start..l.end).contains(&at))
}

pub fn all(text: &str) -> Vec<Link> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Link> = Vec::new();
    // quoted paths first: a Windows profile folder often has a space ("C:\Users\First Last")
    let mut i = 0;
    while i < chars.len() {
        if (chars[i] == '"' || chars[i] == '\'')
            && let Some(len) = chars[i + 1..].iter().position(|c| *c == chars[i])
        {
            let inner: String = chars[i + 1..i + 1 + len].iter().collect();
            if let Some(target) = file(&inner) {
                out.push(Link {
                    start: i + 1,
                    end: i + 1 + len,
                    target,
                });
                i += len + 2;
                continue;
            }
        }
        i += 1;
    }
    // then whitespace-separated runs
    let mut start = 0;
    while start < chars.len() {
        if chars[start].is_whitespace() || STOP.contains(&chars[start]) {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < chars.len() && !chars[end].is_whitespace() && !STOP.contains(&chars[end]) {
            end += 1;
        }
        if !out.iter().any(|l| l.start < end && start < l.end) {
            out.extend(token(&chars[start..end], start));
        }
        start = end;
    }
    out.sort_by_key(|l| l.start);
    out
}

/// One run of non-space chars: a URL inside it, or the whole run as a path.
fn token(chars: &[char], offset: usize) -> Option<Link> {
    let s: String = chars.iter().collect();
    if let Some((byte, _)) = SCHEMES
        .iter()
        .filter_map(|sch| s.find(sch).map(|b| (b, sch)))
        .min_by_key(|(b, _)| *b)
    {
        let lead = s[..byte].chars().count();
        let url = trim_url(&chars[lead..]);
        return (url.len() > 8).then(|| Link {
            start: offset + lead,
            end: offset + lead + url.chars().count(),
            target: Target::Url(url),
        });
    }
    // `![alt](docs/x.png)`: the target is after the last `](`
    let (mut lead, mut body) = (0, chars);
    if let Some(md) = s.rfind("](") {
        lead = s[..md].chars().count() + 2;
        body = &chars[lead..];
    }
    // brackets, quotes and sentence punctuation around a path are not part of it
    let skip = body
        .iter()
        .take_while(|c| "([{<*_,;!".contains(**c))
        .count();
    lead += skip;
    body = &body[skip..];
    let mut len = body.len();
    while len > 0 && ")]}>*_,;.!?:".contains(body[len - 1]) {
        len -= 1;
    }
    let text: String = body[..len].iter().collect();
    let target = file(&text)?;
    Some(Link {
        start: offset + lead,
        end: offset + lead + len,
        target,
    })
}

/// A URL with the sentence punctuation after it cut off, and a closing bracket only when it
/// closes one the URL opened (`(see https://x.y/a_(b))`).
fn trim_url(chars: &[char]) -> String {
    let mut len = chars.len();
    while let Some(&last) = chars[..len].last() {
        let open = match last {
            ')' => Some('('),
            ']' => Some('['),
            '}' => Some('{'),
            _ => None,
        };
        let unbalanced = open.is_some_and(|o| {
            let opens = chars[..len].iter().filter(|c| **c == o).count();
            let closes = chars[..len].iter().filter(|c| **c == last).count();
            closes > opens
        });
        if ".,;:!?".contains(last) || unbalanced {
            len -= 1;
        } else {
            break;
        }
    }
    chars[..len].iter().collect()
}

/// `path[:line[:col]]` if `text` looks like a file path at all: it has a folder separator, or a
/// name with an extension. Whether it exists is the caller's question.
fn file(text: &str) -> Option<Target> {
    let mut rest = text;
    let mut nums: Vec<u32> = Vec::new();
    // up to two trailing `:N`, read from the end so a drive letter's colon stays put
    while nums.len() < 2 {
        let Some((head, tail)) = rest.rsplit_once(':') else {
            break;
        };
        match tail.parse::<u32>() {
            Ok(n) if !head.is_empty() => {
                nums.insert(0, n);
                rest = head;
            }
            _ => break,
        }
    }
    let path = rest;
    if path.is_empty() || path.contains("://") || path.chars().any(char::is_control) {
        return None;
    }
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let has_sep = path.contains('/') || path.contains('\\');
    let has_ext = name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && (1..=8).contains(&ext.len())
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
            && ext.chars().any(|c| c.is_ascii_alphabetic())
    });
    // a lone separator or a number with a dot is not worth a disk check
    let has_word = path.chars().any(|c| c.is_alphabetic());
    if !(has_sep || has_ext) || !has_word || name == "." || name == ".." {
        return None;
    }
    Some(Target::File {
        path: path.to_string(),
        line: nums.first().copied(),
        col: nums.get(1).copied(),
    })
}

/// A path from the output, against the folder the terminal started in.
pub fn resolve(path: &str, cwd: &Path) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() || path.starts_with(['/', '\\']) {
        return p.to_path_buf();
    }
    let rel = path
        .strip_prefix("./")
        .or_else(|| path.strip_prefix(".\\"))
        .unwrap_or(path);
    cwd.join(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_at(text: &str, at_char: usize) -> Option<(String, Option<u32>, Option<u32>)> {
        match at(text, at_char)?.target {
            Target::File { path, line, col } => Some((path, line, col)),
            Target::Url(_) => None,
        }
    }

    fn url_at(text: &str, at_char: usize) -> Option<String> {
        match at(text, at_char)?.target {
            Target::Url(u) => Some(u),
            Target::File { .. } => None,
        }
    }

    #[test]
    fn urls_lose_trailing_punctuation_but_keep_their_own_brackets() {
        assert_eq!(
            url_at("see https://example.com/a?b=1.", 6).as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(
            url_at(
                "(docs at https://en.wikipedia.org/wiki/Rust_(language))",
                12
            )
            .as_deref(),
            Some("https://en.wikipedia.org/wiki/Rust_(language)")
        );
        assert_eq!(
            url_at("<http://localhost:3000/x>", 3).as_deref(),
            Some("http://localhost:3000/x")
        );
        assert_eq!(url_at("plain words here", 3), None);
    }

    #[test]
    fn compiler_paths_carry_line_and_column() {
        let line = "  --> src/main.rs:12:5";
        assert_eq!(
            file_at(line, 8),
            Some(("src/main.rs".into(), Some(12), Some(5)))
        );
        assert_eq!(
            file_at(r"error at C:\work\app\lib.rs:3: oops", 12),
            Some((r"C:\work\app\lib.rs".into(), Some(3), None))
        );
        assert_eq!(
            file_at("Cargo.toml changed", 2),
            Some(("Cargo.toml".into(), None, None))
        );
        // the link spans exactly the path and its position
        let l = at(line, 8).unwrap();
        assert_eq!((l.start, l.end), (6, 22));
    }

    #[test]
    fn wrapping_punctuation_and_markdown_are_not_part_of_a_path() {
        assert_eq!(
            file_at("(see docs/a.md).", 6),
            Some(("docs/a.md".into(), None, None))
        );
        assert_eq!(
            file_at("![shot](docs/x.png)", 10),
            Some(("docs/x.png".into(), None, None))
        );
        assert_eq!(
            file_at(r#"saved "C:\Users\First Last\shot.png" ok"#, 12),
            Some((r"C:\Users\First Last\shot.png".into(), None, None))
        );
    }

    #[test]
    fn words_numbers_and_versions_are_not_paths() {
        for text in ["hello", "1.5", "/", "..", "3:4", "v2"] {
            assert_eq!(at(text, 0), None, "{text}");
        }
    }

    #[test]
    fn relative_paths_resolve_against_the_folder() {
        let cwd = Path::new("/w/app");
        assert_eq!(resolve("./src/a.rs", cwd), cwd.join("src/a.rs"));
        assert_eq!(resolve("src/a.rs", cwd), cwd.join("src/a.rs"));
        assert_eq!(resolve("/etc/hosts", cwd), PathBuf::from("/etc/hosts"));
    }
}
