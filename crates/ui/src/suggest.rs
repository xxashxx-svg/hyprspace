use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClickEvent, Entity, FontWeight, Global, IntoElement, KeyBinding, MouseButton,
    SharedString, actions, div, prelude::*, relative,
};
use hyprspace_proto::agents::{SkillItem, SkillKind};
use hyprspace_proto::{Agent, Client, Command, CommandInfo, FolderCommand, SkillCommand};

use crate::assets::icon;
use crate::colors;
use crate::input::TextInput;
use crate::zoom::px;

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

type Chips = Rc<HashSet<String>>;

pub const THREAD: &str = "\u{1}thread:";

#[derive(Default)]
struct Known {
    commands: HashMap<Agent, Vec<String>>,
    /// What a session in each folder said about its commands and skills.
    info: HashMap<(Agent, PathBuf), Vec<CommandInfo>>,
    /// Counts every change to the lists above, so `chips` knows when it is stale.
    version: u64,
    chips: HashMap<(Agent, PathBuf), (u64, Chips)>,
    files: HashMap<PathBuf, (Instant, Vec<String>)>,
    skills: HashMap<PathBuf, (Instant, Vec<SkillItem>)>,
    asked: HashMap<(PathBuf, bool), Instant>,
    threads: Vec<Item>,
}

impl Global for Known {}

pub fn commands_of(agent: Agent, cx: &mut App) -> Vec<String> {
    cx.default_global::<Known>()
        .commands
        .get(&agent)
        .cloned()
        .unwrap_or_default()
}

pub fn set_commands(agent: Agent, names: Vec<String>, cx: &mut App) {
    let known = cx.default_global::<Known>();
    known.commands.insert(agent, names);
    known.version += 1;
}

pub fn set_info(agent: Agent, cwd: PathBuf, info: Vec<CommandInfo>, cx: &mut App) {
    let known = cx.default_global::<Known>();
    known.info.insert((agent, cwd), info);
    known.version += 1;
}

pub fn set_threads(threads: Vec<Item>, cx: &mut App) {
    cx.default_global::<Known>().threads = threads;
}

pub fn set_files(cwd: PathBuf, files: Vec<String>, cx: &mut App) {
    cx.default_global::<Known>()
        .files
        .insert(cwd, (Instant::now(), files));
}

pub fn set_skills(cwd: PathBuf, items: Vec<SkillItem>, cx: &mut App) {
    let known = cx.default_global::<Known>();
    known.skills.insert(cwd, (Instant::now(), items));
    known.version += 1;
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
    own: Vec<Item>,
    threads: bool,
}

impl Suggest {
    pub fn with(own: Vec<Item>) -> Self {
        Self {
            own,
            ..Self::default()
        }
    }

    pub fn with_threads(mut self) -> Self {
        self.threads = true;
        self
    }

    pub fn refresh(
        &mut self,
        input: &Entity<TextInput>,
        agent: Agent,
        cwd: &Path,
        client: &Client,
        cx: &mut App,
    ) {
        let chips = chip_words(agent, cwd, cx);
        let (text, cursor) = input.update(cx, |i, _| {
            i.set_chips(chips);
            (i.text().to_string(), i.cursor_offset())
        });
        let found = detect(
            &text,
            cursor,
            agent,
            cwd,
            (&self.own, self.muted, self.threads),
            client,
            cx,
        );
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

    pub fn accept(
        &mut self,
        ix: Option<usize>,
        input: &Entity<TextInput>,
        cx: &mut App,
    ) -> Option<u64> {
        let open = self.open.take()?;
        let item = open.items.get(ix.unwrap_or(open.sel))?;
        let thread = item.insert.strip_prefix(THREAD);
        let text = if thread.is_some() { "" } else { &item.insert };
        input.update(cx, |i, cx| i.replace_range(open.range.clone(), text, cx));
        thread.and_then(|id| id.parse().ok())
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
    (own, muted, threads): (&[Item], Option<usize>, bool),
    client: &Client,
    cx: &mut App,
) -> Option<Open> {
    let before = text.get(..cursor)?;
    let start = before
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let word = &before[start..];
    // a command only where it starts the message; further in, or after `$`, only a skill, which
    // the agent runs wherever the message names it
    let (q, inline) = match word.strip_prefix('/') {
        Some(q) if !path(q) => (q, start > 0),
        Some(_) => return None,
        None => match word.strip_prefix('$') {
            Some(q) => (q, true),
            None => return files(word, start..cursor, cwd, (muted, threads), client, cx),
        },
    };
    if muted == Some(start) {
        return None;
    }
    want(client, cwd, false, cx);
    let items = rank(&commands(agent, cwd, own, inline, cx), q);
    (!items.is_empty()).then_some(Open {
        kind: Kind::Command,
        range: start..cursor,
        items,
        sel: 0,
    })
}

/// A typed path, `/usr/bin` or `/c/Main`, rather than a command.
fn path(q: &str) -> bool {
    q.contains(['/', '\\'])
}

fn files(
    word: &str,
    range: Range<usize>,
    cwd: &Path,
    (muted, threads): (Option<usize>, bool),
    client: &Client,
    cx: &mut App,
) -> Option<Open> {
    let q = word.strip_prefix('@')?;
    if muted == Some(range.start) {
        return None;
    }
    want(client, cwd, true, cx);
    let known = cx.default_global::<Known>();
    let mut all: Vec<Item> = known
        .files
        .get(cwd)
        .map(|(_, files)| {
            files
                .iter()
                .map(|f| {
                    let (dir, name) = f.rsplit_once('/').unwrap_or(("", f));
                    Item {
                        label: name.to_string(),
                        detail: dir.to_string(),
                        insert: format!("@{f} "),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if threads {
        all.extend(known.threads.iter().cloned());
    }
    let items = rank(&all, q);
    (!items.is_empty()).then_some(Open {
        kind: Kind::File,
        range,
        items,
        sel: 0,
    })
}

/// What the agent takes in `cwd`: the CLI's own list when a session there gave it, then the
/// skills and commands on disk it doesn't name, then any other name the CLI gave before. Only
/// skills when `inline`.
fn commands(agent: Agent, cwd: &Path, own: &[Item], inline: bool, cx: &mut App) -> Vec<Item> {
    let known = cx.default_global::<Known>();
    let mut items: Vec<Item> = if inline { Vec::new() } else { own.to_vec() };
    let add = |items: &mut Vec<Item>, item: Item| {
        if !items.iter().any(|i| i.label == item.label) {
            items.push(item);
        }
    };
    let listed = known.info.get(&(agent, cwd.to_path_buf()));
    for c in listed.into_iter().flatten().filter(|c| !inline || c.skill) {
        add(
            &mut items,
            Item {
                label: c.name.clone(),
                detail: c.description.clone(),
                insert: insert(agent, c),
            },
        );
    }
    let on_disk = known.skills.get(cwd).map(|(_, s)| s.as_slice());
    for s in on_disk.unwrap_or_default().iter().filter(|s| !s.hidden) {
        if inline && s.kind != SkillKind::Skill {
            continue;
        }
        add(
            &mut items,
            Item {
                label: s.name.clone(),
                detail: s.description.clone(),
                insert: if agent == Agent::Claude || s.body.trim().is_empty() {
                    format!("/{} ", s.name)
                } else {
                    format!("{}\n\n", s.body.trim())
                },
            },
        );
    }
    if listed.is_none() {
        // a folder no session listed yet, or one the CLI spells differently: what the agent said
        // anywhere else still describes the same plugin and user skills
        let elsewhere: HashMap<&str, &CommandInfo> = known
            .info
            .iter()
            .filter(|((a, _), _)| *a == agent)
            .flat_map(|(_, list)| list)
            .map(|c| (c.name.as_str(), c))
            .collect();
        let before_start: &[&str] = if agent == Agent::Claude {
            &["compact"]
        } else {
            &[]
        };
        let names = known
            .commands
            .get(&agent)
            .cloned()
            .unwrap_or_else(|| before_start.iter().map(|s| s.to_string()).collect());
        for name in names {
            let item = match elsewhere.get(name.as_str()) {
                Some(c) if !inline || c.skill => Item {
                    label: name,
                    detail: c.description.clone(),
                    insert: insert(agent, c),
                },
                _ if inline => continue,
                _ => Item {
                    insert: format!("/{name} "),
                    label: name,
                    detail: String::new(),
                },
            };
            add(&mut items, item);
        }
    }
    items
}

/// Each command and skill the agent takes in `cwd`, as it is typed, for the input and the sent
/// message to draw as chips.
pub fn chip_words(agent: Agent, cwd: &Path, cx: &mut App) -> Chips {
    let key = (agent, cwd.to_path_buf());
    let known = cx.default_global::<Known>();
    let version = known.version;
    if let Some((at, words)) = known.chips.get(&key)
        && *at == version
    {
        return words.clone();
    }
    let words: Chips = Rc::new(
        commands(agent, cwd, &[], false, cx)
            .into_iter()
            .filter_map(|i| {
                let word = i.insert.strip_suffix(' ')?;
                (!word.contains(char::is_whitespace)).then(|| word.to_string())
            })
            .collect(),
    );
    let known = cx.default_global::<Known>();
    known.chips.insert(key, (version, words.clone()));
    words
}

/// How a command is typed: Codex names its skills with `$`, and Claude runs an MCP server's
/// prompt, listed as `server:prompt (MCP)`, as `/mcp__server__prompt`.
fn insert(agent: Agent, c: &CommandInfo) -> String {
    if agent == Agent::Codex && c.path.is_some() {
        return format!("${} ", c.name);
    }
    match c
        .name
        .strip_suffix(" (MCP)")
        .and_then(|n| n.split_once(':'))
    {
        Some((server, prompt)) => format!("/mcp__{server}__{prompt} "),
        None => format!("/{} ", c.name),
    }
}

/// Names that start with the query first, then ones with a word that does, then any that hold
/// it, then ones that hold its letters in order, then descriptions that hold it.
fn rank(all: &[Item], query: &str) -> Vec<Item> {
    let q = query.to_lowercase();
    let mut scored: Vec<(u8, usize, &Item)> = all
        .iter()
        .filter_map(|item| {
            let label = item.label.to_lowercase();
            let mut words = label.split(['-', '_', ':', ' ', '/']).skip(1);
            let score = if label.starts_with(&q) {
                0
            } else if words.any(|w| w.starts_with(&q)) {
                1
            } else if label.contains(&q) {
                2
            } else if in_order(&label, &q) {
                3
            } else if item.detail.to_lowercase().contains(&q) {
                4
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

fn in_order(label: &str, q: &str) -> bool {
    let mut rest = label.chars();
    q.chars().all(|c| rest.any(|l| l == c))
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
            Kind::Command if item.insert.starts_with('$') => {
                ("square-slash", format!("${}", item.label))
            }
            Kind::Command => ("square-slash", format!("/{}", item.label)),
            Kind::File if item.insert.starts_with(THREAD) => ("message-square", item.label.clone()),
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
                    Kind::File if open.items.iter().any(|i| i.insert.starts_with(THREAD)) => {
                        "Files and threads"
                    }
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
    fn a_word_start_or_the_letters_in_order_find_a_name() {
        let all = [
            item("show-me", ""),
            item("marketing:seo-audit", ""),
            item("unslop", ""),
        ];
        let got = |q: &str| -> Vec<String> { rank(&all, q).into_iter().map(|i| i.label).collect() };
        assert_eq!(got("me")[0], "show-me");
        assert_eq!(got("seo"), ["marketing:seo-audit"]);
        assert_eq!(got("sm"), ["show-me"]);
        assert!(got("zz").is_empty());
    }

    #[test]
    fn paths_are_not_commands() {
        assert!(path("usr/bin"));
        assert!(path("c\\Main"));
        assert!(!path("show-me"));
    }

    #[test]
    fn commands_are_typed_the_way_their_agent_takes_them() {
        let c = |name: &str, path: Option<&str>| CommandInfo {
            name: name.into(),
            description: String::new(),
            skill: true,
            path: path.map(PathBuf::from),
        };
        assert_eq!(insert(Agent::Claude, &c("show-me", None)), "/show-me ");
        assert_eq!(
            insert(Agent::Claude, &c("lualink:inspect_game (MCP)", None)),
            "/mcp__lualink__inspect_game "
        );
        assert_eq!(
            insert(Agent::Codex, &c("review", Some("/s/SKILL.md"))),
            "$review "
        );
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
