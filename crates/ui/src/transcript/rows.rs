use std::collections::HashSet;
use std::ops::Range;

use hyprspace_proto::RunStatus;

use super::ask;
use super::model::{AgentState, Item};
use crate::markdown::{Block, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Loading,
    Empty,
    Item {
        ix: usize,
        user: bool,
    },
    /// A later piece of a long reply, whose first piece is its `Item` row.
    Part {
        ix: usize,
        part: usize,
    },
    Quiet(usize),
    Fold {
        prompt: usize,
        ms: Option<u64>,
    },
    Working,
    /// Where what came while the thread was off screen starts.
    New,
}

fn quiet(item: &Item) -> bool {
    matches!(item, Item::Tool { .. } | Item::Thinking { .. }) && !ask::special(item)
}

fn todo(item: &Item) -> bool {
    matches!(item, Item::Tool { tool, .. } if ask::named(tool).is_some_and(|(n, _)| n == ask::TODO))
}

pub fn quiet_end(items: &[Item], start: usize) -> usize {
    start + items[start..].iter().take_while(|i| quiet(i)).count()
}

// a run's changes are worked out after it ends, so they can land after the next prompt
fn late(all: &[Item], ix: usize) -> Option<usize> {
    if !matches!(all[ix], Item::Changes { .. }) {
        return None;
    }
    let run = all[..ix]
        .iter()
        .rposition(|i| matches!(i, Item::Finished { .. }))?;
    (run + 1..ix).find(|&j| matches!(all[j], Item::User { .. }))
}

/// The most pieces of a reply in one row. GPUI's list lays out whole rows, so a long reply in one
/// row is laid out in full every frame it shows, however little of it is on screen.
const PIECES: usize = 8;

/// A reply's pieces, in the runs drawn as rows.
pub fn stretches(units: &[Unit]) -> Vec<Range<usize>> {
    let size = if units.len() <= PIECES {
        units.len().max(1)
    } else {
        PIECES
    };
    (0..units.len().max(1))
        .step_by(size)
        .map(|s| s..(s + size).min(units.len()))
        .collect()
}

/// How many rows `stretches` gives a reply of these blocks.
pub fn rows_for(blocks: &[Block]) -> usize {
    let units: usize = blocks
        .iter()
        .map(|b| match b {
            Block::List { items, .. } => items.len(),
            _ => 1,
        })
        .sum();
    if units <= PIECES {
        1
    } else {
        units.div_ceil(PIECES)
    }
}

/// `rows` with the divider before the first row of item `from` or later, when one follows it.
pub fn mark_new(mut rows: Vec<Row>, from: Option<usize>) -> Vec<Row> {
    let Some(from) = from else {
        return rows;
    };
    let at = rows.iter().position(|r| match r {
        Row::Item { ix, .. } | Row::Part { ix, .. } | Row::Quiet(ix) => *ix >= from,
        Row::Fold { prompt, .. } => *prompt >= from,
        _ => false,
    });
    if let Some(at) = at {
        rows.insert(at, Row::New);
    }
    rows
}

/// `rows` with a row after each reply for each of its later pieces.
pub fn split(rows: Vec<Row>, parts: impl Fn(usize) -> usize) -> Vec<Row> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row);
        if let Row::Item { ix, user: false } = row {
            out.extend((1..parts(ix)).map(|part| Row::Part { ix, part }));
        }
    }
    out
}

pub struct Fold {
    pub prompt: usize,
    pub first: usize,
    pub ms: Option<u64>,
    pub hide: Vec<usize>,
}

fn foldable(item: &Item) -> bool {
    match item {
        Item::Thinking { .. } | Item::Text { .. } => true,
        Item::Tool { tool, .. } | Item::Approval { tool, .. } => {
            !matches!(ask::named(tool), Some((ask::QUESTION | ask::PLAN, _)))
        }
        Item::Agent(a) => a.state != AgentState::Working,
        _ => false,
    }
}

fn runs(all: &[Item]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut open = None;
    for (ix, item) in all.iter().enumerate() {
        match item {
            Item::User { steer: false, .. } => open = Some((ix, ix + 1)),
            Item::Finished { .. } => {
                if let Some((anchor, first)) = open.take() {
                    out.push((anchor, first, ix));
                }
            }
            _ if open.is_none() && (foldable(item) || matches!(item, Item::Error(_))) => {
                open = Some((ix, ix));
            }
            _ => {}
        }
    }
    out
}

pub fn folds(all: &[Item]) -> Vec<Fold> {
    let mut out = Vec::new();
    for (anchor, first, end) in runs(all) {
        let Item::Finished {
            status: RunStatus::Done,
            ms,
            ..
        } = all[end]
        else {
            continue;
        };
        let run = first..end;
        if all[run.clone()].iter().any(|i| matches!(i, Item::Error(_))) {
            continue;
        }
        let reply = run
            .clone()
            .rev()
            .find(|&j| matches!(all[j], Item::Text { .. }));
        let mut cuts = vec![(anchor, first)];
        cuts.extend(
            run.filter(|&j| matches!(all[j], Item::User { .. }))
                .map(|j| (j, j + 1)),
        );
        for (i, &(prompt, first)) in cuts.iter().enumerate() {
            let to = cuts.get(i + 1).map_or(end, |c| c.0);
            let hide: Vec<usize> = (first..to)
                .filter(|&j| Some(j) != reply && foldable(&all[j]))
                .collect();
            if !hide.is_empty() {
                out.push(Fold {
                    prompt,
                    first,
                    ms: (i == 0).then_some(ms),
                    hide,
                });
            }
        }
    }
    out
}

pub fn folded_in(all: &[Item], ix: usize) -> Option<usize> {
    folds(all)
        .into_iter()
        .find(|f| f.hide.contains(&ix))
        .map(|f| f.prompt)
}

pub fn plan(all: &[Item], loading: bool, live: bool, open: &HashSet<usize>) -> Vec<Row> {
    let mut hidden = vec![false; all.len()];
    let mut heads = std::collections::HashMap::new();
    for f in folds(all) {
        if !open.contains(&f.prompt) {
            for &j in &f.hide {
                hidden[j] = true;
            }
        }
        heads.insert(f.first, (f.prompt, f.ms));
    }
    let mut rows = Vec::new();
    if loading {
        rows.push(Row::Loading);
    } else if all.is_empty() {
        rows.push(Row::Empty);
    }
    let last_todo = all.iter().rposition(todo);
    let mut ix = 0;
    while ix < all.len() {
        let item = &all[ix];
        if let Some(&(prompt, ms)) = heads.get(&ix) {
            rows.push(Row::Fold { prompt, ms });
        }
        if hidden[ix] {
            ix += 1;
        } else if ask::special(item) {
            if (!todo(item) || Some(ix) == last_todo) && !ask::carded(all, ix) {
                rows.push(Row::Item { ix, user: false });
            }
            ix += 1;
        } else if quiet(item) {
            rows.push(Row::Quiet(ix));
            ix = quiet_end(all, ix);
        } else {
            let done = matches!(
                item,
                Item::Finished {
                    status: RunStatus::Done,
                    ..
                }
            );
            if !done && late(all, ix).is_none() {
                let user = matches!(item, Item::User { .. });
                rows.push(Row::Item { ix, user });
            }
            ix += 1;
        }
    }
    for (ix, prompt) in (0..all.len()).filter_map(|ix| Some((ix, late(all, ix)?))) {
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Item { ix, .. } if *ix == prompt))
            .unwrap_or(rows.len());
        rows.insert(at, Row::Item { ix, user: false });
    }
    if live {
        rows.push(Row::Working);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::Tool;

    fn user() -> Item {
        Item::User {
            text: "go".into(),
            images: Vec::new(),
            steer: false,
        }
    }

    fn call(name: &str) -> Item {
        Item::Tool {
            id: name.into(),
            tool: Tool::Command {
                command: name.into(),
            },
            done: None,
            open: false,
            images: Vec::new(),
            live: String::new(),
        }
    }

    fn todo_list() -> Item {
        Item::Tool {
            id: "t".into(),
            tool: Tool::Other {
                name: ask::TODO.into(),
                input: "{}".into(),
            },
            done: None,
            open: false,
            images: Vec::new(),
            live: String::new(),
        }
    }

    fn finished(status: RunStatus) -> Item {
        Item::Finished {
            status,
            ms: 1,
            error: None,
        }
    }

    #[test]
    fn only_a_run_that_did_not_finish_keeps_its_end_line() {
        let open = HashSet::new();
        assert_eq!(
            plan(&[user(), finished(RunStatus::Done)], false, false, &open),
            [Row::Item { ix: 0, user: true }]
        );
        assert_eq!(
            plan(
                &[user(), finished(RunStatus::Interrupted)],
                false,
                false,
                &open
            ),
            [
                Row::Item { ix: 0, user: true },
                Row::Item { ix: 1, user: false },
            ]
        );
    }

    #[test]
    fn a_run_a_background_task_woke_folds_too() {
        let items = [
            user(),
            reply("started the scan"),
            finished(RunStatus::Done),
            call("ls"),
            reply("scan is done"),
            finished(RunStatus::Done),
        ];
        assert_eq!(
            plan(&items, false, false, &HashSet::new()),
            [
                Row::Item { ix: 0, user: true },
                Row::Item { ix: 1, user: false },
                Row::Fold {
                    prompt: 3,
                    ms: Some(1)
                },
                Row::Item { ix: 4, user: false },
            ]
        );
    }

    #[test]
    fn quiet_calls_share_a_row_and_a_clean_finish_has_none() {
        let items = [
            user(),
            Item::Thinking {
                text: "hm".into(),
                open: false,
            },
            call("ls"),
            call("pwd"),
            Item::Note("n".into()),
            finished(RunStatus::Done),
            finished(RunStatus::Interrupted),
        ];
        assert_eq!(
            plan(&items, false, true, &HashSet::from([0])),
            [
                Row::Item { ix: 0, user: true },
                Row::Fold {
                    prompt: 0,
                    ms: Some(1)
                },
                Row::Quiet(1),
                Row::Item { ix: 4, user: false },
                Row::Item { ix: 6, user: false },
                Row::Working,
            ]
        );
        assert_eq!(quiet_end(&items, 1), 4);
    }

    #[test]
    fn a_run_s_changes_stay_above_the_prompt_sent_before_they_came() {
        let changes = Item::Changes {
            before: "a".into(),
            after: "b".into(),
            files: Vec::new(),
            all: false,
            undo: super::super::model::Undo::Ready,
        };
        let long = Item::Finished {
            status: RunStatus::Done,
            ms: 95_000,
            error: None,
        };
        let items = [user(), long, user(), changes];
        assert_eq!(
            plan(&items, false, true, &HashSet::new()),
            [
                Row::Item { ix: 0, user: true },
                Row::Item { ix: 3, user: false },
                Row::Item { ix: 2, user: true },
                Row::Working,
            ]
        );
    }

    #[test]
    fn a_long_reply_takes_a_row_per_stretch() {
        let units: Vec<Unit> = (0..20).map(|n| Unit::Entry(0, n)).collect();
        assert_eq!(stretches(&units[..8]).len(), 1);
        assert_eq!(stretches(&[]).len(), 1);
        for n in [0, 1, 8, 9, 16, 17, 40] {
            let list = Block::List {
                start: Some(1),
                items: vec![Vec::new(); n],
            };
            let blocks = [Block::Rule, list];
            let units = crate::markdown::units(&blocks);
            assert_eq!(rows_for(&blocks), stretches(&units).len());
        }
        assert_eq!(stretches(&units), [0..8, 8..16, 16..20]);
        let rows = [
            Row::Item { ix: 0, user: true },
            Row::Item { ix: 1, user: false },
        ];
        assert_eq!(
            split(rows.to_vec(), |ix| if ix == 1 { 3 } else { 1 }),
            [
                Row::Item { ix: 0, user: true },
                Row::Item { ix: 1, user: false },
                Row::Part { ix: 1, part: 1 },
                Row::Part { ix: 1, part: 2 },
            ]
        );
    }

    #[test]
    fn the_new_divider_goes_before_what_came_after() {
        let rows = vec![
            Row::Item { ix: 0, user: true },
            Row::Quiet(1),
            Row::Item { ix: 3, user: false },
            Row::Working,
        ];
        assert_eq!(mark_new(rows.clone(), None), rows);
        assert_eq!(
            mark_new(rows.clone(), Some(2)),
            [
                Row::Item { ix: 0, user: true },
                Row::Quiet(1),
                Row::New,
                Row::Item { ix: 3, user: false },
                Row::Working,
            ]
        );
        assert_eq!(mark_new(rows.clone(), Some(9)), rows);
    }

    fn reply(text: &str) -> Item {
        Item::Text {
            source: text.into(),
            blocks: None,
        }
    }

    fn steer() -> Item {
        Item::User {
            text: "also".into(),
            images: Vec::new(),
            steer: true,
        }
    }

    #[test]
    fn a_settled_run_folds_its_work_above_the_reply() {
        let items = [
            user(),
            Item::Thinking {
                text: "hm".into(),
                open: false,
            },
            reply("checking"),
            call("ls"),
            Item::Note("n".into()),
            reply("done"),
            Item::Finished {
                status: RunStatus::Done,
                ms: 21_000,
                error: None,
            },
        ];
        let head = Row::Fold {
            prompt: 0,
            ms: Some(21_000),
        };
        assert_eq!(
            plan(&items, false, false, &HashSet::new()),
            [
                Row::Item { ix: 0, user: true },
                head,
                Row::Item { ix: 4, user: false },
                Row::Item { ix: 5, user: false },
            ]
        );
        assert_eq!(
            plan(&items, false, false, &HashSet::from([0])),
            [
                Row::Item { ix: 0, user: true },
                head,
                Row::Quiet(1),
                Row::Item { ix: 2, user: false },
                Row::Quiet(3),
                Row::Item { ix: 4, user: false },
                Row::Item { ix: 5, user: false },
            ]
        );
        assert_eq!(folded_in(&items, 2), Some(0));
        assert_eq!(folded_in(&items, 5), None);
    }

    #[test]
    fn live_failed_and_stopped_runs_stay_open_and_steers_split_the_fold() {
        let open = HashSet::new();
        let live = [user(), call("ls"), reply("done")];
        assert!(folds(&live).is_empty());
        for status in [RunStatus::Failed, RunStatus::Interrupted] {
            assert!(folds(&[user(), call("ls"), finished(status)]).is_empty());
        }
        let errored = [
            user(),
            call("ls"),
            Item::Error("x".into()),
            finished(RunStatus::Done),
        ];
        assert!(folds(&errored).is_empty());
        let steered = [
            user(),
            call("ls"),
            steer(),
            call("pwd"),
            reply("done"),
            finished(RunStatus::Done),
        ];
        assert_eq!(
            plan(&steered, false, false, &open),
            [
                Row::Item { ix: 0, user: true },
                Row::Fold {
                    prompt: 0,
                    ms: Some(1)
                },
                Row::Item { ix: 2, user: true },
                Row::Fold {
                    prompt: 2,
                    ms: None
                },
                Row::Item { ix: 4, user: false },
            ]
        );
    }

    #[test]
    fn only_the_last_task_list_shows() {
        let items = [user(), todo_list(), call("ls"), todo_list()];
        assert_eq!(
            plan(&items, false, false, &HashSet::new()),
            [
                Row::Item { ix: 0, user: true },
                Row::Quiet(2),
                Row::Item { ix: 3, user: false },
            ]
        );
        assert_eq!(plan(&[], false, false, &HashSet::new()), [Row::Empty]);
        assert_eq!(plan(&[], true, false, &HashSet::new()), [Row::Loading]);
    }
}
