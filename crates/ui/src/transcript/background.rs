use gpui::{
    ClickEvent, Context, Div, ElementId, InteractiveElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, div,
};
use hyprspace_proto::{Task, TaskKind, Tool};
use hyprspace_theme::MONO;

use super::TranscriptView;
use super::model::Item;
use super::rows::Row;
use crate::zoom::px;
use crate::{colors, widgets};

const LINES: usize = 16;

pub(super) fn panel(
    v: &TranscriptView,
    task: &Task,
    ix: usize,
    cx: &mut Context<TranscriptView>,
) -> Stateful<Div> {
    let call = task.tool.as_deref();
    let command = call.and_then(|id| {
        v.model.items.iter().find_map(|i| match i {
            Item::Tool {
                id: t,
                tool: Tool::Command { command },
                ..
            } if t == id => Some(command.clone()),
            _ => None,
        })
    });
    let shown = call
        .filter(|id| row_of(v, id).is_some())
        .map(|id| (id.to_string(), call_open(v, id)));
    let (title, lines) = if reads(task) {
        let lines = match &v.task_log {
            None => vec!["Reading its output".to_string()],
            Some(out) if out.trim().is_empty() => vec!["No output yet".to_string()],
            Some(out) => tail(&plain(out)),
        };
        ("Latest output", lines)
    } else {
        ("Latest steps", steps(v, call))
    };
    let head = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(MONO)
                .text_size(px(12.))
                .text_color(colors::text2())
                .children(command.map(|c| format!("$ {c}"))),
        )
        .children(shown.map(|(call, open)| {
            let (label, tip) = if open {
                ("Hide in thread", "Close the call that started it")
            } else {
                ("Show in thread", "Open the call that started it")
            };
            widgets::button(("task-show", ix), label)
                .h(px(24.))
                .px(px(9.))
                .text_size(px(12.))
                .tooltip(widgets::tip(tip))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    toggle_call(v, &call, cx);
                    cx.notify();
                }))
        }));
    div()
        .id(("task-log", ix))
        .flex()
        .flex_col()
        .gap(px(8.))
        .pt(px(10.))
        .pb(px(10.))
        .child(head)
        .child(
            div()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .bg(colors::ink(0.04))
                .font_family(MONO)
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(colors::text2())
                .children(lines.into_iter().map(|l| {
                    div()
                        .truncate()
                        .child(if l.is_empty() { " ".to_string() } else { l })
                })),
        )
}

/// Whether the CLI hands over the task's output. It does for commands and monitors only.
pub(super) fn reads(task: &Task) -> bool {
    matches!(task.kind, TaskKind::Command | TaskKind::Monitor)
}

/// A subagent's last calls, from its card in the thread.
fn steps(v: &TranscriptView, call: Option<&str>) -> Vec<String> {
    let agent = call.and_then(|id| {
        v.model.items.iter().find_map(|i| match i {
            Item::Agent(a) if a.id == id => Some(a),
            _ => None,
        })
    });
    let Some(agent) = agent else {
        return vec!["Nothing to show for this task".to_string()];
    };
    let mut lines: Vec<String> = agent
        .calls
        .iter()
        .map(|c| super::tool::label(&c.tool))
        .collect();
    if lines.is_empty() {
        return vec!["No steps yet".to_string()];
    }
    lines.split_off(lines.len().saturating_sub(LINES))
}

fn call_open(v: &TranscriptView, call: &str) -> bool {
    v.model
        .items
        .iter()
        .any(|i| matches!(i, Item::Tool { id, open: true, .. } if id == call))
}

fn toggle_call(v: &mut TranscriptView, call: &str, cx: &mut Context<TranscriptView>) {
    let at = v
        .model
        .items
        .iter()
        .position(|i| matches!(i, Item::Tool { id, .. } if id == call));
    if let Some(prompt) = at.and_then(|at| super::rows::folded_in(&v.model.items, at))
        && v.unfolded.insert(prompt)
    {
        super::render::replan(v);
    }
    let Some(row) = row_of(v, call) else {
        return;
    };
    let open = !call_open(v, call);
    let items = &mut v.model.items;
    let Some(at) = items
        .iter()
        .position(|i| matches!(i, Item::Tool { id, .. } if id == call))
    else {
        return;
    };
    let tool = |i: &Item| matches!(i, Item::Tool { .. });
    let start = (0..at)
        .rev()
        .take_while(|&j| tool(&items[j]))
        .last()
        .unwrap_or(at);
    let end = (at..items.len())
        .take_while(|&j| tool(&items[j]))
        .last()
        .unwrap_or(at);
    if let Some(Item::Tool { open: o, .. }) = items.get_mut(at) {
        *o = open;
    }
    let card = ElementId::from((SharedString::from(format!("tool-{at}")), 0usize));
    crate::slide::flip(super::render::fold_key(cx, &card));
    super::render::settle_fold(cx);
    if !open {
        return;
    }
    // consecutive calls fold into one line that opens on its own
    if end > start {
        v.open_runs.insert(start);
    }
    v.follow.seek(&v.list, row);
}

/// The row a tool call sits in: its own, or the group of quiet calls it was folded into.
fn row_of(v: &TranscriptView, call: &str) -> Option<usize> {
    let at = v
        .model
        .items
        .iter()
        .position(|i| matches!(i, Item::Tool { id, .. } if id == call))?;
    v.rows.iter().rposition(|r| match r {
        Row::Item { ix, .. } | Row::Quiet(ix) => *ix <= at,
        _ => false,
    })
}

fn tail(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let from = lines.len().saturating_sub(LINES);
    lines[from..].iter().map(|l| l.to_string()).collect()
}

pub(super) fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => match out.rfind('\n') {
                Some(at) => out.truncate(at + 1),
                None => out.clear(),
            },
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_reads_as_plain_text() {
        assert_eq!(
            plain("\u{1b}[32mVITE\u{1b}[0m ready\r\nbuild 10%\rbuild 100%\n"),
            "VITE ready\nbuild 100%\n"
        );
        assert_eq!(plain("\u{1b}]0;title\u{7}ok"), "ok");
    }
}
