// Tool calls in the transcript: a one-line summary that opens to show the call's input, its
// output and, for file edits, the diff. A run of calls in a row folds into one line that counts
// them, after zeron's "Ran 4 commands · read 1 file". Approval prompts reuse the same summary.

use gpui::{AnyElement, FontWeight, IntoElement, SharedString, div, prelude::*, px};
use hyprspace_proto::Tool;
use hyprspace_proto::run::{ChangeKind, FileChange};
use hyprspace_theme::MONO;

use crate::colors;

/// One line saying what a tool does or did.
pub fn label(tool: &Tool) -> String {
    match tool {
        Tool::Command { command } => format!("Run {}", first_line(without_cd(command))),
        Tool::Read { path } => format!("Read {}", short(path)),
        Tool::Edit { changes } => {
            let names: Vec<String> = changes.iter().map(|c| short(&c.path)).collect();
            let verb = match changes.first().map(|c| c.kind) {
                Some(ChangeKind::Add) if changes.len() == 1 => "Create",
                Some(ChangeKind::Delete) if changes.len() == 1 => "Delete",
                _ => "Edit",
            };
            if names.is_empty() {
                "Edit files".into()
            } else {
                format!("{verb} {}", names.join(", "))
            }
        }
        Tool::Search { pattern, path } => match path {
            Some(p) => format!("Search {} for {pattern}", short(p)),
            None => format!("Search for {pattern}"),
        },
        Tool::Web { target } => format!("Look up {target}"),
        Tool::Mcp { server, tool, .. } => format!("{server}: {tool}"),
        Tool::Agent { description, .. } if !description.is_empty() => description.clone(),
        Tool::Agent { .. } => "Subagent".into(),
        Tool::Other { name, .. } => name.clone(),
    }
}

/// Agents often open a command by moving into the thread's own folder (`cd "C:\..." && git
/// status`). That prefix is the same every time and pushes the real command off the line, so the
/// label drops it; the full command still shows when the call is opened.
fn without_cd(cmd: &str) -> &str {
    let Some(rest) = cmd.trim_start().strip_prefix("cd ") else {
        return cmd;
    };
    let rest = rest.trim_start();
    let after = match rest.strip_prefix('"') {
        Some(quoted) => quoted.find('"').map(|i| &quoted[i + 1..]),
        None => rest
            .find(|c: char| c.is_whitespace() || c == ';' || c == '&')
            .map(|i| &rest[i..]),
    };
    let Some(after) = after.map(str::trim_start) else {
        return cmd;
    };
    for sep in ["&&", ";"] {
        if let Some(next) = after.strip_prefix(sep).map(str::trim_start)
            && !next.is_empty()
        {
            return next;
        }
    }
    cmd
}

/// One line counting a run of calls by kind: "Ran 4 commands · read 1 file · called 1 tool".
pub fn summary<'a>(tools: impl IntoIterator<Item = &'a Tool>) -> String {
    // commands, files read, files edited, searches, lookups, other tools
    let mut n = [0usize; 6];
    for t in tools {
        match t {
            Tool::Command { .. } => n[0] += 1,
            Tool::Read { .. } => n[1] += 1,
            Tool::Edit { changes } => n[2] += changes.len().max(1),
            Tool::Search { .. } => n[3] += 1,
            Tool::Web { .. } => n[4] += 1,
            Tool::Mcp { .. } | Tool::Agent { .. } | Tool::Other { .. } => n[5] += 1,
        }
    }
    let plural = |n: usize, one: &str, many: &str| if n == 1 { one } else { many }.to_string();
    let parts: Vec<String> = [
        (
            n[0],
            format!("ran {} {}", n[0], plural(n[0], "command", "commands")),
        ),
        (
            n[1],
            format!("read {} {}", n[1], plural(n[1], "file", "files")),
        ),
        (
            n[2],
            format!("edited {} {}", n[2], plural(n[2], "file", "files")),
        ),
        (
            n[3],
            format!("searched {} {}", n[3], plural(n[3], "time", "times")),
        ),
        (
            n[4],
            format!("looked up {} {}", n[4], plural(n[4], "page", "pages")),
        ),
        (
            n[5],
            format!("called {} {}", n[5], plural(n[5], "tool", "tools")),
        ),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(_, s)| s)
    .collect();
    let line = parts.join(" · ");
    let mut chars = line.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => line,
    }
}

/// Lines added and removed across an edit's diffs.
pub fn counts(changes: &[FileChange]) -> (usize, usize) {
    let mut add = 0;
    let mut del = 0;
    for line in changes.iter().flat_map(|c| c.diff.lines()) {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            add += 1;
        } else if line.starts_with('-') {
            del += 1;
        }
    }
    (add, del)
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or_default();
    if line.chars().count() > 90 {
        format!("{}...", line.chars().take(90).collect::<String>())
    } else {
        line.to_string()
    }
}

/// The last two parts of a path, which is what tells files apart in practice.
fn short(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 => path.to_string(),
        1 | 2 => parts.join("/"),
        n => parts[n - 2..].join("/"),
    }
}

/// What the call was given, shown when it is opened.
pub fn input(tool: &Tool) -> Option<String> {
    match tool {
        Tool::Command { command } => Some(command.clone()),
        Tool::Read { path } => Some(path.clone()),
        Tool::Search { pattern, path } => Some(match path {
            Some(p) => format!("{pattern}\nin {p}"),
            None => pattern.clone(),
        }),
        Tool::Web { target } => Some(target.clone()),
        Tool::Agent { prompt, .. } => Some(prompt.clone()).filter(|p| !p.is_empty()),
        Tool::Mcp { input, .. } | Tool::Other { input, .. } => {
            Some(input.clone()).filter(|s| !s.is_empty() && s != "{}")
        }
        Tool::Edit { .. } => None,
    }
}

/// A block of mono text, for inputs and outputs.
pub fn mono(text: &str) -> AnyElement {
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(8.))
        .bg(colors::surface2())
        .font_family(MONO)
        .text_size(px(11.5))
        .line_height(px(17.))
        .text_color(colors::text2())
        .child(text.trim_end().to_string())
        .into_any_element()
}

/// The diffs of a file edit, one card per file.
pub fn diffs(changes: &[FileChange]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(changes.iter().map(|c| {
            let lines = c
                .diff
                .lines()
                .filter(|l| !l.starts_with("+++") && !l.starts_with("---"));
            div()
                .flex()
                .flex_col()
                .rounded_sm()
                .border_1()
                .border_color(colors::border1())
                .overflow_hidden()
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text2())
                        .bg(colors::surface2())
                        .child(c.path.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .font_family(MONO)
                        .text_xs()
                        .children(lines.map(diff_line)),
                )
        }))
        .into_any_element()
}

fn diff_line(line: &str) -> AnyElement {
    let (bg, fg) = match line.as_bytes().first() {
        Some(b'+') => (Some(colors::diff_add().opacity(0.14)), colors::text1()),
        Some(b'-') => (Some(colors::diff_del().opacity(0.14)), colors::text1()),
        Some(b'@') => (None, colors::text3()),
        _ => (None, colors::text2()),
    };
    // an empty line still needs its height
    let text: SharedString = if line.is_empty() {
        " ".into()
    } else {
        line.to_string().into()
    };
    div()
        .px_2()
        .min_h(px(16.))
        .text_color(fg)
        .when_some(bg, |d, bg| d.bg(bg))
        .child(text)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(kind: ChangeKind, path: &str, diff: &str) -> FileChange {
        FileChange {
            path: path.into(),
            kind,
            diff: diff.into(),
        }
    }

    #[test]
    fn labels_name_the_action_and_the_file() {
        let cmd = Tool::Command {
            command: "ls -la\necho hi".into(),
        };
        assert_eq!(label(&cmd), "Run ls -la");
        let in_folder = |c: &str| label(&Tool::Command { command: c.into() });
        assert_eq!(
            in_folder(r#"cd "C:\a b\repo" && git status"#),
            "Run git status"
        );
        assert_eq!(in_folder("cd /tmp/repo; ls"), "Run ls");
        // a bare cd, or one with nothing after it, stays as it is
        assert_eq!(in_folder("cd repo"), "Run cd repo");
        assert_eq!(in_folder("cd repo &&"), "Run cd repo &&");
        let add = Tool::Edit {
            changes: vec![edit(ChangeKind::Add, r"C:\w\src\new.rs", "+x")],
        };
        assert_eq!(label(&add), "Create src/new.rs");
        let read = Tool::Read {
            path: "/a/b/c/d.txt".into(),
        };
        assert_eq!(label(&read), "Read c/d.txt");
        assert_eq!(
            input(&Tool::Other {
                name: "X".into(),
                input: "{}".into()
            }),
            None
        );
    }

    #[test]
    fn a_run_of_calls_counts_by_kind() {
        let cmd = Tool::Command {
            command: "ls".into(),
        };
        let read = Tool::Read { path: "a".into() };
        let other = Tool::Other {
            name: "X".into(),
            input: String::new(),
        };
        let run = [cmd.clone(), cmd.clone(), cmd.clone(), cmd, read, other];
        assert_eq!(
            summary(&run),
            "Ran 4 commands · read 1 file · called 1 tool"
        );
        let edit = Tool::Edit {
            changes: vec![
                edit(ChangeKind::Update, "a", ""),
                edit(ChangeKind::Add, "b", ""),
            ],
        };
        assert_eq!(summary([&edit]), "Edited 2 files");
    }

    #[test]
    fn counts_skip_file_headers() {
        let c = [edit(
            ChangeKind::Update,
            "a",
            "--- a/x\n+++ b/x\n@@\n-old\n+new\n+more\n same",
        )];
        assert_eq!(counts(&c), (2, 1));
    }
}
