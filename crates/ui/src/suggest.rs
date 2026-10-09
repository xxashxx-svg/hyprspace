use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClickEvent, Entity, FontWeight, Global, IntoElement, KeyBinding, MouseButton,
    SharedString, actions, div, prelude::*, px, relative,
};
use hyprspace_proto::agents::SkillItem;
use hyprspace_proto::{Agent, Client, Command, FolderCommand, SkillCommand};

use crate::assets::icon;
use crate::colors;
use crate::input::TextInput;

actions!(suggest, [Prev, Next, Accept, Dismiss]);

pub fn bind_keys(cx: &mut App) {
    let c = Some("Suggest > TextInput");
    cx.bind_keys([
        KeyBinding::new("up", Prev, c),
        KeyBinding::new("down", Next, c),
        KeyBinding::new("tab", Accept, c),
        KeyBinding::new("enter", Accept, c),
        KeyBinding::new("escape", Dismiss, c),
    ]);
}

const SHOWN: usize = 8;
const STALE: Duration = Duration::from_secs(60);
const ASK_AGAIN: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Known {
    commands: HashMap<Agent, Vec<String>>,
    files: HashMap<PathBuf, (Instant, Vec<String>)>,
    skills: HashMap<PathBuf, (Instant, Vec<SkillItem>)>,
    asked: HashMap<(PathBuf, bool), Instant>,
}

impl Global for Known {}

pub fn set_commands(agent: Agent, names: Vec<String>, cx: &mut App) {
    cx.default_global::<Known>().commands.insert(agent, names);
}

pub fn set_files(cwd: PathBuf, files: Vec<String>, cx: &mut App) {
    cx.default_global::<Known>()
        .files
        .insert(cwd, (Instant::now(), files));
}

pub fn set_skills(cwd: PathBuf, items: Vec<SkillItem>, cx: &mut App) {
    cx.default_global::<Known>()
        .skills
        .insert(cwd, (Instant::now(), items));
}

fn want(client: &Client, cwd: &Path, files: bool, cx: &mut App) {
    let known = cx.default_global::<Known>();
    let at = if files {
        known.files.get(cwd).map(|(at, _)| *at)
    } else {
        known.skills.get(cwd).map(|(at, _)| *at)
    };
    let key = (cwd.to_path_buf(), files);
    let asked = known
        .asked
        .get(&key)
        .is_some_and(|at| at.elapsed() < ASK_AGAIN);
    if at.is_some_and(|at| at.elapsed() < STALE) || asked {
        return;
    }
    known.asked.insert(key, Instant::now());
    client.send(if files {
        Command::Folder(FolderCommand::ListFiles {
            cwd: cwd.to_path_buf(),
        })
    } else {
        Command::Skills(SkillCommand::List {
            cwd: cwd.to_path_buf(),
        })
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    File,
}

#[derive(Clone, PartialEq)]
pub struct Item {
    pub label: String,
    pub detail: String,
    pub insert: String,
}

#[derive(Clone)]
pub struct Open {
    pub kind: Kind,
    pub range: Range<usize>,
    pub items: Vec<Item>,
    pub sel: usize,
}

impl Open {
    pub fn step(&mut self, by: isize) {
        let n = self.items.len() as isize;
        if n > 0 {
            self.sel = (self.sel as isize + by).rem_euclid(n) as usize;
        }
    }
}

#[derive(Default)]
pub struct Suggest {
    pub open: Option<Open>,
    muted: Option<usize>,
}

impl Suggest {
    pub fn refresh(
        &mut self,
        input: &Entity<TextInput>,
        agent: Agent,
        cwd: &Path,
        client: &Client,
        cx: &mut App,
    ) {
        let (text, cursor) = {
            let i = input.read(cx);
            (i.text().to_string(), i.cursor_offset())
        };
        let found = detect(&text, cursor, agent, cwd, self.muted, client, cx);
        if found.is_none() {
            self.muted = None;
        }
        let sel = self
            .open
            .as_ref()
            .zip(found.as_ref())
            .filter(|(old, new)| old.kind == new.kind && old.range.start == new.range.start)
            .map(|(old, new)| old.sel.min(new.items.len().saturating_sub(1)))
            .unwrap_or(0);
        self.open = found.map(|mut o| {
            o.sel = sel;
            o
        });
    }

    pub fn accept(&mut self, ix: Option<usize>, input: &Entity<TextInput>, cx: &mut App) {
        let Some(open) = self.open.take() else {
            return;
        };
        if let Some(item) = open.items.get(ix.unwrap_or(open.sel)) {
            input.update(cx, |i, cx| {
                i.replace_range(open.range.clone(), &item.insert, cx)
            });
        }
    }

    pub fn step(&mut self, by: isize) {
        if let Some(o) = &mut self.open {
            o.step(by);
        }
    }

    pub fn dismiss(&mut self) {
        self.muted = self.open.take().map(|o| o.range.start);
    }
}

fn detect(
    text: &str,
    cursor: usize,
    agent: Agent,
    cwd: &Path,
    muted: Option<usize>,
    client: &Client,
    cx: &mut App,
) -> Option<Open> {
    let before = text.get(..cursor)?;
    if let Some(q) = before.strip_prefix('/')
        && !q.contains(char::is_whitespace)
    {
        if muted == Some(0) {
            return None;
        }
        want(client, cwd, false, cx);
        let items = rank(&commands(agent, cwd, cx), q);
        return (!items.is_empty()).then_some(Open {
            kind: Kind::Command,
            range: 0..cursor,
            items,
            sel: 0,
        });
    }
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let q = before[start..].strip_prefix('@')?;
    if muted == Some(start) {
        return None;
    }
    want(client, cwd, true, cx);
    let known = cx.default_global::<Known>();
    let (_, files) = known.files.get(cwd)?;
    let files: Vec<Item> = files
        .iter()
        .map(|f| {
            let (dir, name) = f.rsplit_once('/').unwrap_or(("", f));
            Item {
                label: name.to_string(),
                detail: dir.to_string(),
                insert: format!("@{f} "),
            }
        })
        .collect();
    let items = rank(&files, q);
    (!items.is_empty()).then_some(Open {
        kind: Kind::File,
        range: start..cursor,
        items,
        sel: 0,
    })
}

fn commands(agent: Agent, cwd: &Path, cx: &mut App) -> Vec<Item> {
    let known = cx.default_global::<Known>();
    let mut items: Vec<Item> = known
        .skills
        .get(cwd)
        .map(|(_, s)| s.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|s| Item {
            label: s.name.clone(),
            detail: s.description.clone(),
            insert: if agent == Agent::Claude || s.body.trim().is_empty() {
                format!("/{} ", s.name)
            } else {
                format!("{}\n\n", s.body.trim())
            },
        })
        .collect();
    for name in known.commands.get(&agent).into_iter().flatten() {
        if !items.iter().any(|i| &i.label == name) {
            items.push(Item {
                label: name.clone(),
                detail: String::new(),
                insert: format!("/{name} "),
            });
        }
    }
    items
}

fn rank(all: &[Item], query: &str) -> Vec<Item> {
    let q = query.to_lowercase();
    let mut scored: Vec<(u8, usize, &Item)> = all
        .iter()
        .filter_map(|item| {
            let label = item.label.to_lowercase();
            let score = if label.starts_with(&q) {
                0
            } else if label.contains(&q) {
                1
            } else if item.detail.to_lowercase().contains(&q) {
                2
            } else {
                return None;
            };
            Some((score, label.len() + item.detail.len(), item))
        })
        .collect();
    scored.sort_by_key(|&(s, len, _)| (s, len));
    scored
        .into_iter()
        .take(SHOWN)
        .map(|(_, _, i)| i.clone())
        .collect()
}

pub fn render(
    open: &Open,
    below: bool,
    pick: impl Fn(usize, &mut gpui::Window, &mut App) + 'static,
) -> AnyElement {
    let pick = std::rc::Rc::new(pick);
    let rows = open.items.iter().enumerate().map(|(i, item)| {
        let on = i == open.sel;
        let pick = pick.clone();
        let (glyph, name) = match open.kind {
            Kind::Command => ("square-slash", format!("/{}", item.label)),
            Kind::File => ("file", item.label.clone()),
        };
        div()
            .id(("suggest", i))
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(30.))
            .px(px(10.))
            .rounded(px(7.))
            .cursor_pointer()
            .when(on, |d| d.bg(colors::accent().opacity(0.14)))
            .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.06))))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_: &ClickEvent, w, cx| pick(i, w, cx))
            .child(icon(
                glyph,
                13.,
                if on { colors::text1() } else { colors::text3() },
            ))
            .child(
                div()
                    .flex_none()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(SharedString::from(name)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(SharedString::from(item.detail.clone())),
            )
    });
    div()
        .absolute()
        .left_0()
        .right_0()
        .map(|d| {
            if below {
                d.top(relative(1.)).mt(px(6.))
            } else {
                d.bottom(relative(1.)).mb(px(6.))
            }
        })
        .flex()
        .flex_col()
        .p(px(5.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .child(
            div()
                .px(px(10.))
                .pt(px(4.))
                .pb(px(3.))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child(match open.kind {
                    Kind::Command => "Commands and skills",
                    Kind::File => "Files",
                }),
        )
        .children(rows)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, detail: &str) -> Item {
        Item {
            label: label.into(),
            detail: detail.into(),
            insert: format!("/{label} "),
        }
    }

    #[test]
    fn names_that_start_with_the_query_come_first() {
        let all = [
            item("review", ""),
            item("compact", ""),
            item("security-review", ""),
            item("init", "Write a review guide"),
        ];
        let got: Vec<String> = rank(&all, "rev").into_iter().map(|i| i.label).collect();
        assert_eq!(got, ["review", "security-review", "init"]);
    }

    #[test]
    fn steps_wrap_around() {
        let mut o = Open {
            kind: Kind::Command,
            range: 0..1,
            items: vec![item("a", ""), item("b", ""), item("c", "")],
            sel: 0,
        };
        o.step(-1);
        assert_eq!(o.sel, 2);
        o.step(1);
        assert_eq!(o.sel, 0);
    }
}
