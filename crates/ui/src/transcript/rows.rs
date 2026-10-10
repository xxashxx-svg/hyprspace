use hyprspace_proto::RunStatus;

use super::ask;
use super::model::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Loading,
    Empty,
    Item { ix: usize, user: bool },
    Quiet(usize),
    Working,
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

pub fn plan(all: &[Item], loading: bool, live: bool) -> Vec<Row> {
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
        if ask::special(item) {
            if !todo(item) || Some(ix) == last_todo {
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
            if !done {
                let user = matches!(item, Item::User { .. });
                rows.push(Row::Item { ix, user });
            }
            ix += 1;
        }
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
            plan(&items, false, true),
            [
                Row::Item { ix: 0, user: true },
                Row::Quiet(1),
                Row::Item { ix: 4, user: false },
                Row::Item { ix: 6, user: false },
                Row::Working,
            ]
        );
        assert_eq!(quiet_end(&items, 1), 4);
    }

    #[test]
    fn only_the_last_task_list_shows() {
        let items = [user(), todo_list(), call("ls"), todo_list()];
        assert_eq!(
            plan(&items, false, false),
            [
                Row::Item { ix: 0, user: true },
                Row::Quiet(2),
                Row::Item { ix: 3, user: false },
            ]
        );
        assert_eq!(plan(&[], false, false), [Row::Empty]);
        assert_eq!(plan(&[], true, false), [Row::Loading]);
    }
}
