// Tool calls in the transcript: a one-line summary that opens to show the call's input, its
// output and, for file edits, the diff. Approval prompts reuse the same summary.

use gpui::{AnyElement, FontWeight, IntoElement, SharedString, div, prelude::*, px};
use hyprspace_proto::Tool;
use hyprspace_proto::run::{ChangeKind, FileChange};
use hyprspace_theme::MONO;

use crate::colors;

/// One line saying what a tool does or did.
pub fn label(tool: &Tool) -> String {
    match tool {
        Tool::Command { command } => format!("Run {}", first_line(command)),
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
        Tool::Other { name, .. } => name.clone(),
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
        Tool::Mcp { input, .. } | Tool::Other { input, .. } => {
            Some(input.clone()).filter(|s| !s.is_empty() && s != "{}")
        }
        Tool::Edit { .. } => None,
    }
}

/// A block of mono text, for inputs and outputs.
pub fn mono(text: &str) -> AnyElement {
    div()
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(colors::surface2())
        .font_family(MONO)
        .text_xs()
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
    fn counts_skip_file_headers() {
        let c = [edit(
            ChangeKind::Update,
            "a",
            "--- a/x\n+++ b/x\n@@\n-old\n+new\n+more\n same",
        )];
        assert_eq!(counts(&c), (2, 1));
    }
}
