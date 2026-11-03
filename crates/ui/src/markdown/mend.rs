// The end of a reply that is still coming in, made to read as it will once it is whole: a bold,
// code or struck span the agent has opened is closed for now, a marker with nothing after it yet
// waits, and so do a half-written link and table lines before the table's rule arrives.

/// `text` with its unfinished end mended. A finished reply is never passed through this: a lone
/// `*` in it may be meant.
pub fn mend(text: &str) -> String {
    let text = hold_marker(text);
    if fenced(text) {
        return text.to_string();
    }
    let text = hold_table(text);
    let (head, tail) = text.split_at(last_block(text));
    let tail = hold_link(tail);
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(head);
    out.push_str(&close(tail));
    // a span dropped for having nothing in it can leave a bare marker, as `- ` from "- `"
    let kept = hold_marker(&out).len();
    out.truncate(kept);
    out
}

/// Whether the text ends inside a code fence.
fn fenced(text: &str) -> bool {
    text.lines()
        .filter(|l| {
            let l = l.trim_start();
            l.starts_with("```") || l.starts_with("~~~")
        })
        .count()
        % 2
        == 1
}

/// A last line that is only a marker so far, like `2.`, `-`, `#` or a fence's backticks, waits
/// for what follows it. Drawn now it would be a stray number on the line above, a heading
/// underline or an empty block.
fn hold_marker(text: &str) -> &str {
    let start = text.rfind('\n').map_or(0, |i| i + 1);
    let whole = text[start..].trim();
    // inside a quote the marker comes after its `>`s
    let line = whole.trim_start_matches(['>', ' ']);
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let bare = ((1..=9).contains(&digits) && matches!(&line[digits..], "" | "." | ")"))
        || matches!(
            line,
            "-" | "--" | "*" | "+" | "_" | "__" | ">" | "`" | "``" | "~" | "~~" | "!"
        )
        || ((1..=6).contains(&line.len()) && line.bytes().all(|b| b == b'#'))
        || line.starts_with("```")
        || line.starts_with("~~~")
        // a tag still being written, like `</detail`
        || (line.starts_with('<') && !line.contains('>'))
        || (line.is_empty() && !whole.is_empty());
    if bare { &text[..start] } else { text }
}

/// Where the last block starts: just past the last blank line.
fn last_block(text: &str) -> usize {
    text.rfind("\n\n").map_or(0, |i| i + 2)
}

/// Table lines at the end wait until the header has its rule under it, and a half-written row
/// waits for the rest of its cells.
fn hold_table(text: &str) -> &str {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let rows: Vec<&str> = lines
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with('|'))
        .copied()
        .collect();
    let Some(&last) = rows.first() else {
        return text;
    };
    let taken: usize = rows.iter().map(|r| r.len()).sum();
    if !(rows.len() >= 2 && rule(rows[rows.len() - 2])) {
        return &text[..text.len() - taken];
    }
    if last.ends_with('\n') {
        text
    } else {
        &text[..text.len() - last.len()]
    }
}

/// A table's rule line, `|---|:--|`, once it is whole.
fn rule(line: &str) -> bool {
    let l = line.trim();
    line.ends_with('\n')
        && l.starts_with('|')
        && l.contains('-')
        && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// A link waits until it is whole: from an unclosed `[`, a `]` that may yet have `(` after it,
/// or a `](` with no `)` after it yet. A task's box, `[ ]` or `[x]`, shows as it is.
fn hold_link(tail: &str) -> &str {
    let Some(open) = tail.rfind('[') else {
        return tail;
    };
    let after = &tail[open..];
    // a picture's `!` waits with its link
    let from = if tail[..open].ends_with('!') {
        open - 1
    } else {
        open
    };
    match after.find(']') {
        None => &tail[..from],
        Some(_) if matches!(after, "[ ]" | "[x]" | "[X]") => tail,
        Some(close) if after.len() == close + 1 => &tail[..from],
        Some(close) if after[close..].starts_with("](") && !after[close..].contains(')') => {
            &tail[..from]
        }
        _ => tail,
    }
}

/// Closes the spans left open at the end, innermost first. One opened with nothing after it is
/// dropped instead, so the reader never sees a bare marker.
fn close(tail: &str) -> String {
    let mut open: Vec<(&str, usize)> = Vec::new();
    let mut code = false;
    let mut loose = false;
    let mut i = 0;
    while i < tail.len() {
        let rest = &tail[i..];
        let step = rest.chars().next().map_or(1, char::len_utf8);
        if rest.starts_with('\\') {
            i += step + rest[step..].chars().next().map_or(0, char::len_utf8);
            continue;
        }
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|&b| b == b'`').count();
            if run == 1 {
                code = !code;
                toggle(&mut open, "`", i);
            }
            i += run;
            continue;
        }
        if code {
            i += step;
            continue;
        }
        let before = tail[..i].chars().next_back();
        // an underscore inside a word, as in snake_case, is never emphasis
        let in_word = before.is_some_and(char::is_alphanumeric);
        if let Some(m) = ["**", "~~", "__"].into_iter().find(|m| rest.starts_with(m)) {
            if m != "__" || !in_word || open.iter().any(|(o, _)| *o == m) {
                toggle(&mut open, m, i);
            }
            i += 2;
            continue;
        }
        if let Some(m) = ["*", "_"].into_iter().find(|m| rest.starts_with(m)) {
            let after = rest[1..].chars().next();
            let opens = after.is_some_and(|c| !c.is_whitespace())
                && before.is_none_or(|c| c.is_whitespace() || c.is_ascii_punctuation());
            let closes = before.is_some_and(|c| !c.is_whitespace());
            if open.iter().any(|(o, _)| *o == m) && closes {
                toggle(&mut open, m, i);
            } else if opens && !(m == "_" && in_word) {
                open.push((m, i));
            } else {
                loose = i + 1 == tail.len()
                    && (m == "*" || !in_word || open.iter().any(|(o, _)| *o == "__"));
            }
        } else if rest.starts_with('~') {
            loose = i + 1 == tail.len();
        }
        i += step;
    }
    let mut out = tail.to_string();
    // a `*` or `~` at the very end that opens or closes nothing yet, like the first half of `**`
    if loose {
        out.pop();
    }
    for (m, at) in open.iter().rev() {
        let body = out[at + m.len()..].trim_end().len();
        if body == 0 {
            out.truncate(*at);
        } else {
            // a closer after a space doesn't close, so it goes before the space
            out.insert_str(at + m.len() + body, m);
        }
    }
    out
}

fn toggle(open: &mut Vec<(&'static str, usize)>, m: &'static str, at: usize) {
    match open.iter().rposition(|(o, _)| *o == m) {
        Some(p) => open.truncate(p),
        None => open.push((m, at)),
    }
}

#[cfg(test)]
mod tests {
    use super::mend;

    #[test]
    fn open_spans_close_and_bare_markers_wait() {
        assert_eq!(mend("- **Mark Zu"), "- **Mark Zu**");
        assert_eq!(mend("run `cargo te"), "run `cargo te`");
        assert_eq!(mend("it was ~~gone"), "it was ~~gone~~");
        assert_eq!(mend("a *quiet* and *loud"), "a *quiet* and *loud*");
        assert_eq!(mend("**bold `code"), "**bold `code`**");
        assert_eq!(mend("Then **"), "Then ");
        assert_eq!(mend("and `"), "and ");
        assert_eq!(mend("**Tested*"), "**Tested**");
        assert_eq!(mend("**Not "), "**Not** ");
        assert_eq!(mend("~~gone~"), "~~gone~~");
        assert_eq!(mend("**a \\*"), "**a \\***");
    }

    #[test]
    fn a_bare_marker_line_waits() {
        assert_eq!(mend("What changed:\n-"), "What changed:\n");
        assert_eq!(mend("Steps:\n1"), "Steps:\n");
        assert_eq!(mend("1. one\n2."), "1. one\n");
        assert_eq!(mend("1. one\n   - "), "1. one\n");
        assert_eq!(mend("Intro\n\n## "), "Intro\n\n");
        assert_eq!(mend("Code:\n\n```ru"), "Code:\n\n");
        assert_eq!(mend("```\nlet a;\n``"), "```\nlet a;\n");
        assert_eq!(mend("In 2024 it"), "In 2024 it");
        assert_eq!(mend("a\n2024 was"), "a\n2024 was");
        assert_eq!(mend("> quoted\n> 2."), "> quoted\n");
        assert_eq!(mend("log\n</detail"), "log\n");
        assert_eq!(mend("See:\n\n![flow](docs/fl"), "See:\n\n");
        assert_eq!(mend("Wow!"), "Wow!");
    }

    #[test]
    fn plain_stars_and_code_stay_as_they_are() {
        assert_eq!(mend("5 * 3 is 15"), "5 * 3 is 15");
        assert_eq!(mend("- one\n- tw"), "- one\n- tw");
        assert_eq!(mend("snake_case and `a*b`"), "snake_case and `a*b`");
        assert_eq!(mend("```rust\nlet x = **y"), "```rust\nlet x = **y");
        assert_eq!(mend("done **twice** \\*not"), "done **twice** \\*not");
        assert_eq!(mend("- [ ] a task"), "- [ ] a task");
    }

    #[test]
    fn links_wait_until_whole() {
        assert_eq!(mend("see [the docs"), "see ");
        assert_eq!(mend("see [the docs]"), "see ");
        assert_eq!(mend("see [the docs](https://exa"), "see ");
        assert_eq!(
            mend("see [docs](https://a.b) now"),
            "see [docs](https://a.b) now"
        );
    }

    #[test]
    fn a_table_waits_for_its_rule_and_whole_rows() {
        assert_eq!(mend("Sizes:\n\n| a | b |\n"), "Sizes:\n\n");
        assert_eq!(mend("| a | b |\n|--"), "");
        assert_eq!(
            mend("Sizes:\n\n| a | b |\n|---|---|\n| 1 |"),
            "Sizes:\n\n| a | b |\n|---|---|\n"
        );
        assert_eq!(mend("```\n| a |\n"), "```\n| a |\n");
        let whole = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(mend(whole), whole);
    }
}
