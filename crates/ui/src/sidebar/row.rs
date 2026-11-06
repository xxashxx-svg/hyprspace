// One thread in the sidebar, after T3 Code's: its project's tag and name, then one slot that says
// how old it is or its state in words, and gives way to snooze and Settle on hover; the title;
// its agent's mark beside the title; then its pull request and subagents, when it has them. Rows
// that want you keep their weight and the rest step back. What it is doing shows
// in the card beside it on hover. A working row carries a slow sheen and, off screen, fades back.
// On a shelf a thread is one quiet line. Click to open it, right-click for its menu.

use gpui::{
    AnyElement, App, ClickEvent, Context, DragMoveEvent, ExternalPaths, FontWeight, Hsla,
    IntoElement, MouseButton, ObjectFit, Pixels, SharedString, StyledImage, Transformation, canvas,
    div, img, linear_color_stop, linear_gradient, percentage, prelude::*, relative,
};
use hyprspace_proto::git::WorktreeAction;
use hyprspace_proto::{Agent, Pane, SpaceIcon, Thread, ThreadKind};

use super::{Look, Verb};

use super::card::RowCard;
use crate::workbench::PaneDrag;
use crate::zoom::px;
use hyprspace_theme::MONO;

use super::row_hover;
use crate::assets::{icon, mark};
use crate::colors;
use crate::pace::Looping;
use crate::root::{Action, MenuEntry, MenuItems, Rename, Root, Screen, View};
use crate::time::{ago, local_now, now_ms, presets, to_ms, wake_short};
use crate::transcript::Status;
use crate::widgets;

/// A running count, the way a stopwatch shows it: 45s, 3m 05s, 1h 02m.
/// A thread's pull request beside its branch: a tag with its state's icon and color and its number,
/// and while it's open a mark for its checks failing, running or passing. A click opens it.
fn pr_badge(thread: u64, pr: hyprspace_proto::git::PullRequest) -> AnyElement {
    use hyprspace_proto::git::{Checks, PrState};
    let (glyph, state, tint) = match pr.state {
        PrState::Open => ("git-pull-request", "Open", colors::tone(163.)),
        PrState::Draft => ("git-pull-request", "Draft", colors::text3()),
        PrState::Merged => ("git-merge", "Merged", colors::tone(293.)),
        PrState::Closed => ("git-pull-request-closed", "Closed", colors::tone(22.)),
    };
    let open = matches!(pr.state, PrState::Open | PrState::Draft);
    let (checks, says) = match pr.checks {
        Checks::Failing if open => (Some(colors::error()), ", checks failing"),
        Checks::Pending if open => (Some(colors::busy()), ", checks running"),
        Checks::Passing if open => (None, ", checks passed"),
        _ => (None, ""),
    };
    let url = pr.url.clone();
    div()
        .id(("pr", thread))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(3.))
        .h(px(16.))
        .px(px(4.))
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(tint)
        .hover(|s| s.bg(colors::ink(0.06)))
        .child(icon(glyph, 10., tint))
        .child(
            div()
                .font_family(MONO)
                .text_size(px(10.5))
                .child(format!("#{}", pr.number)),
        )
        .children(checks.map(|c| div().ml(px(1.)).size(px(5.)).rounded_full().bg(c)))
        .tooltip(widgets::tip_text(format!("{state}{says}: {}", pr.title)))
        .on_click(move |_: &gpui::ClickEvent, _, cx| {
            cx.stop_propagation();
            cx.open_url(&url);
        })
        .into_any_element()
}

pub(super) fn elapsed(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs / 60 % 60),
    }
}

/// A ring turning around a mark `size` across, `inset` pixels outside it.
pub(super) fn ring(size: f32, inset: f32) -> AnyElement {
    div()
        .absolute()
        .top(px(-inset))
        .left(px(-inset))
        .child(
            icon("ring", size + 2. * inset, colors::busy()).looping(900, |s, t| {
                s.with_transformation(Transformation::rotate(percentage(t)))
            }),
        )
        .into_any_element()
}

/// A band of light sweeping slowly across a working row.
fn sheen() -> AnyElement {
    let glow = colors::busy().opacity(0.035);
    let clear = colors::busy().opacity(0.);
    let half = |from: Hsla, to: Hsla| {
        div().flex_1().h_full().bg(linear_gradient(
            90.,
            linear_color_stop(from, 0.),
            linear_color_stop(to, 1.),
        ))
    };
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(0.6))
        .flex()
        .child(half(clear, glow))
        .child(half(glow, clear))
        .looping(2600, |d, t| d.left(relative(1.0 - 1.6 * t)))
        .into_any_element()
}

/// A project's initials for its tag, the way T3 Code labels projects: two words give their first
/// letters (Streamer Tycoon Lobby, ST), one word its first letter and then its first digit or else
/// its last letter (vitanova279, V2; lualink, LK).
fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let first = |w: &str| w.chars().next();
    let (a, b) = match words.as_slice() {
        [] => (None, None),
        [w] => {
            let rest = || w.chars().skip(1);
            (
                first(w),
                rest().find(char::is_ascii_digit).or_else(|| rest().last()),
            )
        }
        [a, b, ..] => (first(a), first(b)),
    };
    [a, b]
        .into_iter()
        .flatten()
        .flat_map(char::to_uppercase)
        .collect()
}

/// A project's tag: its own icon, or its initials on a small square in the project's own color.
/// A `dim` one sits on a shelf.
pub(crate) fn tag(name: &str, look: Look, dim: bool) -> AnyElement {
    let hue = look.hue;
    match look.icon {
        None => letters(name, hue, dim),
        Some(SpaceIcon::Emoji { emoji }) => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(16.))
            .text_size(px(12.5))
            .line_height(px(16.))
            .when(dim, |d| d.opacity(0.5))
            .child(emoji)
            .into_any_element(),
        Some(SpaceIcon::Image { path }) => {
            let name = name.to_string();
            img(path)
                .flex_none()
                .size(px(16.))
                .rounded(px(4.))
                .object_fit(ObjectFit::Contain)
                .when(dim, |d| d.opacity(0.5))
                .with_fallback(move || letters(&name, hue, dim))
                .into_any_element()
        }
    }
}

fn letters(name: &str, hue: Option<u16>, dim: bool) -> AnyElement {
    let (fill, ink) = colors::tag(name, hue);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(4.))
        .bg(fill)
        .text_color(ink)
        .font_family(MONO)
        .text_size(px(8.5))
        .font_weight(FontWeight::BOLD)
        .when(dim, |d| d.opacity(0.5))
        .child(initials(name))
        .into_any_element()
}

/// A thread's agent: its mark. A terminal with no agent shows the terminal glyph.
fn agent_mark(agent: Option<Agent>) -> AnyElement {
    let badge = match agent {
        Some(a) => mark(a, 13., colors::brand(a).0).into_any_element(),
        None => icon("terminal", 12., colors::text3()).into_any_element(),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .child(badge)
        .into_any_element()
}

fn jump_cap(n: usize) -> AnyElement {
    widgets::keycap(n.to_string())
        .h(px(16.))
        .flex()
        .items_center()
        .font_family(MONO)
        .into_any_element()
}

fn said(glyph: &'static str, words: &'static str, tint: Hsla) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tint)
        .child(icon(glyph, 12., tint))
        .child(words)
        .into_any_element()
}

impl Root {
    /// The box over a row's title while that thread is renamed, in the title's own size and
    /// weight so nothing jumps. It is drawn over the title's line, its text where the title's
    /// was, so the rows below stay put. Clicks in it stay in it: placing the cursor or selecting
    /// text neither opens nor drags the row.
    pub(crate) fn rename_box(
        &self,
        id: u64,
        here: bool,
        size: Pixels,
        weight: FontWeight,
    ) -> Option<AnyElement> {
        let (Rename::Thread(r), input, _) = self.rename.as_ref()?;
        (*r == id && self.rename_here == here).then(|| {
            div()
                .id(("rename", id))
                .h(px(24.))
                .my(px(-2.))
                .mx(px(-7.))
                .flex()
                .items_center()
                .px(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::accent())
                .bg(colors::surface1())
                .text_size(size)
                .font_weight(weight)
                .text_color(colors::text1())
                .cursor_text()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .child(input.clone())
                .into_any_element()
        })
    }

    fn button_look(id: u64, key: &'static str, glyph: &'static str) -> gpui::Stateful<gpui::Div> {
        div()
            .id((key, id))
            .group("icon-button")
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .h(px(widgets::SMALL))
            .min_w(px(widgets::SMALL))
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(11.5))
            .text_color(colors::text2())
            .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
            .tooltip(widgets::tip(match key {
                "wake" => "Wake",
                "unsettle" => "Bring back",
                "pin" => "Unpin",
                "snooze" => "Snooze",
                "discard-draft" => "Discard draft",
                _ => "Settle",
            }))
            .child(
                icon(glyph, 12., colors::text2())
                    .group_hover("icon-button", |s| s.text_color(colors::text1())),
            )
    }

    /// A small icon button on a row's hover strip.
    pub(super) fn row_button(
        &self,
        id: u64,
        key: &'static str,
        glyph: &'static str,
        cx: &mut Context<Self>,
        run: fn(&mut Root, u64, &ClickEvent, &mut gpui::Window, &mut Context<Root>),
    ) -> gpui::Stateful<gpui::Div> {
        Self::button_look(id, key, glyph)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |r, e: &ClickEvent, window, cx| {
                cx.stop_propagation();
                r.menu = None;
                run(r, id, e, window, cx)
            }))
    }

    fn sweep_button(
        &self,
        id: u64,
        key: &'static str,
        glyph: &'static str,
        verb: Verb,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        Self::button_look(id, key, glyph).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |r, _, _, cx| {
                cx.stop_propagation();
                r.menu = None;
                r.start_sweep(id, verb, cx)
            }),
        )
    }

    /// The buttons that show on hover: snooze and Settle, or the one that undoes either.
    fn thread_buttons(&self, t: &Thread, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let id = t.id;
        if t.snooze.is_some() {
            vec![
                self.sweep_button(id, "wake", "sun", Verb::Wake, cx)
                    .into_any_element(),
            ]
        } else if t.settled {
            vec![
                self.sweep_button(id, "unsettle", "rotate-ccw", Verb::Unsettle, cx)
                    .into_any_element(),
            ]
        } else {
            vec![
                self.row_button(id, "snooze", "clock", cx, |r, id, e, _, cx| {
                    r.open_snooze_menu(e.position(), id, cx)
                })
                .into_any_element(),
                // the one most rows end with gets its word, as T3 Code's does
                self.sweep_button(id, "settle", "check", Verb::Settle, cx)
                    .px(px(6.))
                    .child("Settle")
                    .into_any_element(),
            ]
        }
    }

    fn running(&self, id: u64, busy: bool, cx: &App) -> Option<u64> {
        match self.views.get(&id) {
            Some(View::Structured(v)) => v.read(cx).elapsed(),
            _ => self
                .turns
                .get(&id)
                .filter(|_| busy)
                .map(|t| t.elapsed().as_secs()),
        }
    }

    /// What a thread is doing, or what it concluded until the next turn. A terminal thread hears
    /// from its hooks; a structured one reads its own transcript.
    fn doing(&self, id: u64, cx: &App) -> Option<String> {
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let background = if matches!(status, Status::Working | Status::Waiting) {
            0
        } else {
            self.subagents(id, cx).len()
        };
        match background {
            0 => match self.views.get(&id) {
                Some(View::Structured(v)) => {
                    let v = v.read(cx);
                    v.doing().or_else(|| v.background_line())
                }
                _ => self.activity.get(&id).and_then(|a| a.doing.clone()),
            }
            .filter(|_| status != Status::Idle),
            1 => Some("1 subagent running".into()),
            n => Some(format!("{n} subagents running")),
        }
    }

    /// The subagents a thread has running: what each was asked, and seconds since it started. A
    /// terminal thread hears of them from its hooks; a structured one reads its own transcript.
    pub(crate) fn subagents(&self, id: u64, cx: &App) -> Vec<(String, u64)> {
        match self.views.get(&id) {
            Some(View::Structured(v)) => v
                .read(cx)
                .subagents()
                .into_iter()
                .map(|(_, label, secs)| (label, secs))
                .collect(),
            _ => {
                let now = now_ms();
                self.activity
                    .get(&id)
                    .map(|a| {
                        a.subs
                            .iter()
                            .map(|s| (s.label.clone(), now.saturating_sub(s.started) / 1000))
                            .collect()
                    })
                    .unwrap_or_default()
            }
        }
    }

    pub(crate) fn thread_card(&self, id: u64, cx: &App) -> Option<RowCard> {
        let (space, t) = self.state.thread(id)?;
        let (model, effort) = match t.agent() {
            Some(launch) => {
                // with no model picked the label is the agent's own name, which needn't repeat
                let mut s = match launch.model {
                    Some(_) => format!("{} {}", launch.agent.name(), self.model_label(launch)),
                    None => launch.agent.name().to_string(),
                };
                if launch.model.as_deref().is_some_and(crate::models::is_long) {
                    s.push_str(" 1M");
                }
                // the level picked, or the model's own default, said as such; none for a model
                // that takes no effort
                let catalog = self
                    .agents
                    .iter()
                    .find(|a| a.agent == launch.agent)
                    .map(|a| &a.catalog);
                let id = crate::models::windowed(launch.model.as_deref().unwrap_or(""), false);
                let effort = match launch.effort.as_deref().filter(|e| !e.is_empty()) {
                    Some(e) => Some(crate::composer::effort_label(e)),
                    None => catalog
                        .and_then(|c| c.model(&id))
                        .and_then(|m| m.default_effort.clone())
                        .map(|d| format!("{} (default)", crate::composer::effort_label(&d))),
                };
                (s, effort)
            }
            None => ("Terminal".to_string(), None),
        };
        let cwd = match &t.kind {
            ThreadKind::Terminal { cwd, .. } => cwd.clone(),
            ThreadKind::Structured { launch } => launch.cwd.clone(),
        };
        let branch = match self.machines.place(id) {
            Some((place, branch)) => branch.then(|| place.clone().into()),
            None => self
                .git
                .get(&cwd)
                .map(|g| &g.branch)
                .filter(|b| b.is_repo && !b.branch.is_empty() && b.branch != "HEAD")
                .map(|b| b.branch.clone().into()),
        };
        let now = now_ms();
        let when = |ms: u64| {
            (ms > 0).then(|| match ago(ms, now).as_str() {
                "now" => "just now".into(),
                a => format!("{a} ago").into(),
            })
        };
        Some(RowCard {
            title: t.title.clone().into(),
            space: space.name.clone().into(),
            look: self.look_of(space),
            machine: space
                .machine
                .as_deref()
                .map(|m| self.machines.name(m).into()),
            path: super::card::tidy(&cwd).into(),
            branch,
            agent: t.agent().map(|l| l.agent),
            doing: self.doing(id, cx).map(Into::into),
            model: model.into(),
            effort: effort.map(Into::into),
            started: when(t.created),
            active: when(t.touched).filter(|_| t.touched > t.created),
            subagents: self
                .subagents(id, cx)
                .into_iter()
                .map(|(label, secs)| (label.into(), elapsed(secs).into()))
                .collect(),
        })
    }

    /// A thread's right-click menu, grouped as T3 Code's: where to open it; settling and
    /// snoozing; naming and finding; its project's apps and copying; and the ones that clear away.
    /// Snooze, Open in and Copy open their choices beside them.
    pub(crate) fn thread_menu(&self, t: &Thread) -> MenuItems {
        let id = t.id;
        let space = self.state.thread(id).map(|(s, _)| s);
        let mut menu = Vec::new();
        if t.snooze.is_some() {
            menu.push(MenuEntry::item("Wake now", Action::Wake(id)));
        } else if t.settled {
            menu.push(MenuEntry::item(
                "Un-settle thread",
                Action::Settle(id, false),
            ));
        } else {
            menu.push(MenuEntry::item(
                if t.pinned.is_some() {
                    "Unpin thread"
                } else {
                    "Pin thread"
                },
                Action::Pin(id, t.pinned.is_none()),
            ));
            menu.push(MenuEntry::item("Settle thread", Action::Settle(id, true)));
            let now = local_now();
            let mut times: Vec<MenuEntry> = presets(now)
                .into_iter()
                .map(|p| MenuEntry::Item {
                    label: p.label.into(),
                    hint: Some(wake_short(p.at, now).into()),
                    action: Action::SnoozeUntil(id, to_ms(p.at)),
                })
                .collect();
            if self.busy(id) {
                times.push(MenuEntry::item("Until it finishes", Action::SnoozeDone(id)));
            }
            times.push(MenuEntry::item("Custom...", Action::SnoozeCustom(id)));
            menu.push(MenuEntry::Sub {
                label: "Snooze".into(),
                icon: "clock",
                entries: times,
            });
        }
        menu.push(MenuEntry::Divider);
        menu.push(MenuEntry::item(
            "Rename thread",
            Action::Rename(Rename::Thread(id)),
        ));
        if !self.unseen.contains(&id) && !space.is_some_and(|s| s.machine.is_some()) {
            menu.push(MenuEntry::item("Mark unread", Action::MarkUnread(id)));
        }
        if self.can_rename_itself(id) {
            menu.push(MenuEntry::item(
                "Regenerate title",
                Action::RegenerateTitle(id),
            ));
        }
        if let Some(label) = self.switch_label(t) {
            menu.push(MenuEntry::Item {
                label: label.into(),
                hint: self.switch_wait(id).map(|s| format!("in {s}s").into()),
                action: Action::Switch(id),
            });
        }
        let mirrored = space.is_some_and(|s| s.machine.is_some());
        if let Some(w) = t.worktree.as_ref().filter(|_| !mirrored) {
            menu.push(MenuEntry::Sub {
                label: "Worktree".into(),
                icon: "git-branch",
                entries: vec![
                    MenuEntry::item(
                        format!("Merge into {}", w.base),
                        Action::Worktree(id, WorktreeAction::Merge),
                    ),
                    MenuEntry::item(
                        "Create pull request",
                        Action::Worktree(id, WorktreeAction::PullRequest),
                    ),
                    MenuEntry::item(
                        "Remove worktree",
                        Action::Worktree(id, WorktreeAction::Remove),
                    ),
                ],
            });
        }
        menu.push(MenuEntry::Divider);
        if let Some(s) =
            space.filter(|s| s.cwd.is_some() && !mirrored && !self.work.openers.is_empty())
        {
            menu.push(MenuEntry::Sub {
                label: format!("Open {} in", s.name).into(),
                icon: "folder-open",
                entries: self
                    .work
                    .openers
                    .iter()
                    .map(|&o| MenuEntry::item(o.name(), Action::OpenIn(o, s.id)))
                    .collect(),
            });
        }
        if let Some(s) = space.filter(|_| !mirrored) {
            menu.push(MenuEntry::item(
                "Icon and color...",
                Action::ProjectIcon(s.id),
            ));
        }
        let mut copy = vec![
            MenuEntry::item("Title", Action::CopyTitle(id)),
            MenuEntry::item("Folder path", Action::CopyPath(id)),
        ];
        if t.agent().is_some_and(|l| l.resume.is_some()) {
            copy.push(MenuEntry::item(
                "Conversation id",
                Action::CopyConversation(id),
            ));
        }
        menu.push(MenuEntry::Sub {
            label: "Copy".into(),
            icon: "copy",
            entries: copy,
        });
        if !mirrored {
            menu.push(MenuEntry::Divider);
            menu.push(MenuEntry::item("Delete thread", Action::RemoveThread(id)));
        }
        menu
    }

    /// A thread in the list, after T3 Code's: its agent's mark and its project's name, with the
    /// age or a running count on the right; the title and the agent's mark; then its pull request
    /// and subagents, when it has them.
    pub(crate) fn thread_row(
        &self,
        t: &Thread,
        now: u64,
        drops: bool,
        key: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = t.id;
        let status = self.status.get(&id).copied().unwrap_or(Status::Idle);
        let selected = self.screen == Screen::Thread(id);
        let waiting = status == Status::Waiting;
        let busy = status == Status::Working || waiting;
        // the agent can end its turn with subagents still at work in the background; the thread
        // is working until they are done
        let background = if busy {
            0
        } else {
            self.subagents(id, cx).len()
        };
        let working = status == Status::Working || background > 0;
        // background work takes less attention than a thread that needs you, after T3 Code's: a
        // working row off screen fades as a whole, and comes back up under the pointer
        let recede = working && !selected && !waiting;
        let running = self.running(id, busy, cx);
        let agent = t.agent().map(|l| l.agent);
        let (space, machine, look) = self
            .state
            .thread(id)
            .map(|(s, _)| {
                (
                    s.name.clone(),
                    s.machine.as_deref().map(|m| self.machines.name(m)),
                    self.look_of(s),
                )
            })
            .unwrap_or_default();
        let unseen = self.unseen.contains(&id);
        let quiet =
            !selected && !unseen && !waiting && status != Status::Failed && t.resume_at.is_none();
        let group: SharedString = format!("row-{id}").into();
        let title: AnyElement = match self.rename_box(id, false, px(13.5), FontWeight::MEDIUM) {
            Some(rename) => rename,
            None if let Some((shown, caret)) = self.typed(id, &t.title) => div()
                .flex()
                .items_center()
                .min_w_0()
                .h(px(20.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if self.naming(id) {
                            colors::text2()
                        } else {
                            colors::text1()
                        })
                        .child(shown),
                )
                .child(
                    div()
                        .flex_none()
                        .ml(px(1.))
                        .w(px(1.5))
                        .h(px(15.))
                        .bg(colors::text1())
                        .when(!caret, |d| d.opacity(0.)),
                )
                .into_any_element(),
            None => div()
                .min_w_0()
                .truncate()
                .text_size(px(13.5))
                .font_weight(if quiet {
                    FontWeight::NORMAL
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if quiet {
                    colors::text1().opacity(0.75)
                } else {
                    colors::text1()
                })
                .when(quiet, |d| {
                    d.group_hover(group.clone(), |s| s.text_color(colors::text1()))
                })
                .child(t.title.clone())
                .into_any_element(),
        };
        let says: AnyElement = match key {
            _ if let Some(badge) = self.swept(id) => badge,
            Some(n) => jump_cap(n),
            None if t.resume_at.is_some() => said("clock", "Limited", colors::busy()),
            None if waiting => said("circle-help", "Needs you", colors::waiting()),
            None if status == Status::Failed => said("circle-alert", "Failed", colors::error()),
            None if working => match running {
                Some(secs) => div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_color(colors::busy())
                    .child(icon("circle-dashed", 12., colors::busy()))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(elapsed(secs)),
                    )
                    .into_any_element(),
                None => said("circle-dashed", "Working", colors::busy()),
            },
            None if unseen && status == Status::Done && background == 0 => {
                said("circle-check", "Done", colors::ok())
            }
            None => div()
                .text_size(px(12.))
                .text_color(colors::text2())
                .when(t.last_touch() > 0, |d| d.child(ago(t.last_touch(), now)))
                .into_any_element(),
        };
        let slot = div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .justify_end()
            .min_w(px(32.))
            .h(px(20.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .group_hover(group.clone(), |s| s.opacity(0.))
                    .child(says),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .right(px(-5.))
                    .flex()
                    .gap(px(1.))
                    .p(px(1.))
                    .rounded(px(6.))
                    .bg(colors::surface3())
                    .opacity(0.)
                    .when(self.sweep.is_none(), |d| {
                        d.group_hover(group.clone(), |s| s.opacity(1.))
                    })
                    .children(self.thread_buttons(t, cx)),
            );
        let pr = self.pr_of(t).cloned().map(|pr| pr_badge(id, pr));
        let kids = self.kids_chip(id, cx);
        let chips = (pr.is_some() || kids.is_some()).then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .h(px(16.))
                .children(pr)
                .children(kids)
        });
        let picked = self.selection.contains(&id);
        let menu = if picked && self.selection.len() > 1 {
            self.bulk_menu()
        } else {
            self.thread_menu(t)
        };
        let row = div()
            .id(("thread", id))
            .group(group.clone())
            .relative()
            .overflow_hidden()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(2.))
            .pt(px(7.))
            .px(px(9.))
            .pb(px(8.))
            .rounded(px(super::RADIUS))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(picked && !selected, |d| d.bg(colors::ink(0.09)))
            .when(recede, |d| d.opacity(0.7))
            .when(!selected, |d| {
                d.hover(move |s| {
                    let s = s.bg(row_hover());
                    if recede { s.opacity(1.) } else { s }
                })
            })
            .when(working, |d| d.child(sheen()))
            .children(self.key_ring(id))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .h(px(20.))
                    .child(tag(&space, look.clone(), false))
                    .when(t.pinned.is_some() && t.active(), |d| {
                        d.child(
                            self.row_button(id, "pin", "pin", cx, |r, id, _, _, cx| {
                                r.pin(id, false, cx)
                            })
                            .h(px(16.))
                            .min_w(px(16.)),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(match look.hue {
                                Some(h) => colors::tag(&space, Some(h)).1,
                                None => colors::text3(),
                            })
                            .child(space.clone()),
                    )
                    .when_some(machine, |d, name| {
                        d.child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(px(4.))
                                .max_w(px(110.))
                                .text_size(px(11.))
                                .text_color(colors::text3())
                                .child(icon("monitor", 11., colors::text3()))
                                .child(div().min_w_0().truncate().child(name)),
                        )
                    })
                    .child(slot),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .child(div().flex_1().min_w_0().child(title))
                    .child(agent_mark(agent)),
            )
            .children(chips)
            .on_click(
                cx.listener(move |r, e: &ClickEvent, window, cx| r.row_click(id, e, window, cx)),
            )
            .on_mouse_move(
                cx.listener(move |r, e: &gpui::MouseMoveEvent, _, cx| r.sweep_over(id, e, cx)),
            )
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .on_hover(cx.listener(move |r, on: &bool, _, cx| r.row_hovered(id, *on, cx)))
            // where the row is drawn, for the card that shows beside it
            .child({
                let bounds = self.row_bounds.clone();
                canvas(
                    |_, _, _| (),
                    move |b, _, _, _| {
                        bounds.borrow_mut().insert(id, b);
                    },
                )
                .absolute()
                .size_full()
            })
            .on_drag(
                PaneDrag {
                    pane: Pane::Thread { id },
                    title: t.title.clone().into(),
                    agent,
                },
                |d, offset, _, cx| cx.new(|_| d.ghost(offset)),
            )
            // while a row is dragged: which half of this one the pointer is on
            .when(drops, |d| {
                d.on_drag_move(cx.listener(move |r, e: &DragMoveEvent<PaneDrag>, _, cx| {
                    let dragged = e.drag(cx).pane.thread();
                    if r.dragging != dragged {
                        r.dragging = dragged;
                        cx.notify();
                    }
                    let (p, b) = (e.event.position, e.bounds);
                    let next = if b.contains(&p) {
                        Some((id, p.y > b.center().y))
                    } else if r.drop_at.is_some_and(|(t, _)| t == id) {
                        None
                    } else {
                        return;
                    };
                    if r.drop_at != next {
                        r.drop_at = next;
                        cx.notify();
                    }
                }))
                .on_drop(cx.listener(move |r, d: &PaneDrag, _, cx| {
                    let below = r.drop_at.is_some_and(|(t, below)| t == id && below);
                    r.end_drag();
                    r.drop_on_thread(d, id, below, cx)
                }))
                .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(colors::accent().opacity(0.08)))
                .on_drop(cx.listener(
                    move |r, paths: &ExternalPaths, window, cx| {
                        cx.stop_propagation();
                        r.open_thread(id, window, cx);
                        r.drop_anywhere(paths, window, cx);
                    },
                ))
            })
            // the line where a dragged row would land
            .children(
                self.drop_at
                    .filter(|(t, _)| *t == id && cx.has_active_drag())
                    .map(|(_, below)| {
                        div()
                            .absolute()
                            .left(px(6.))
                            .right(px(6.))
                            .h(px(2.))
                            .rounded_full()
                            .bg(colors::accent())
                            .map(|l| if below { l.bottom_0() } else { l.top_0() })
                    }),
            );
        // a thread made a moment ago fades in, rising into its slot; the age check keeps rows
        // the list draws again later, like after a scroll, from fading in a second time
        if now.saturating_sub(t.created) < 1000 {
            return crate::slide::ease_in(row, ("row-in", id as usize), 260, |d, t| {
                d.opacity(t).top(px(4. * (1. - t)))
            });
        }
        row.into_any_element()
    }

    /// A thread on the Snoozed or Settled shelf: one quiet line with its project's tag, its title,
    /// and when it wakes or how old it is, which gives way to the button that brings it back.
    pub(crate) fn shelved_row(
        &self,
        t: &Thread,
        now: u64,
        key: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = t.id;
        let selected = self.screen == Screen::Thread(id);
        let group: SharedString = format!("shelved-{id}").into();
        let (space, look) = self
            .state
            .thread(id)
            .map(|(s, _)| (s.name.clone(), self.look_of(s)))
            .unwrap_or_default();
        let picked = self.selection.contains(&id);
        let menu = if picked && self.selection.len() > 1 {
            self.bulk_menu()
        } else {
            self.thread_menu(t)
        };
        let when = match t.snooze {
            Some(snooze) => Some(self.wake_text(snooze)),
            None => (t.last_touch() > 0).then(|| ago(t.last_touch(), now)),
        };
        div()
            .id(("shelved", id))
            .group(group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(30.))
            .px(px(8.))
            .rounded(px(super::RADIUS))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(picked && !selected, |d| d.bg(colors::ink(0.09)))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .children(self.key_ring(id))
            .child(tag(&space, look, true))
            .child(
                match self.rename_box(id, false, px(12.5), FontWeight::NORMAL) {
                    Some(rename) => div().flex_1().min_w_0().child(rename),
                    None => div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(colors::text3())
                        .child(t.title.clone()),
                },
            )
            .children(self.swept(id))
            .children(when.filter(|_| self.swept(id).is_none()).map(|w| {
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .group_hover(group.clone(), |s| s.opacity(0.))
                    .child(w)
            }))
            .children(key.map(jump_cap))
            .child(
                div()
                    .absolute()
                    .right(px(6.))
                    .flex()
                    .opacity(0.)
                    .when(self.sweep.is_none(), |d| {
                        d.group_hover(group, |s| s.opacity(1.))
                    })
                    .children(self.thread_buttons(t, cx)),
            )
            .on_click(
                cx.listener(move |r, e: &ClickEvent, window, cx| r.row_click(id, e, window, cx)),
            )
            .on_mouse_move(
                cx.listener(move |r, e: &gpui::MouseMoveEvent, _, cx| r.sweep_over(id, e, cx)),
            )
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .on_hover(cx.listener(move |r, on: &bool, _, cx| r.row_hovered(id, *on, cx)))
            .child({
                let bounds = self.row_bounds.clone();
                canvas(
                    |_, _, _| (),
                    move |b, _, _, _| {
                        bounds.borrow_mut().insert(id, b);
                    },
                )
                .absolute()
                .size_full()
            })
            .on_drag(
                PaneDrag {
                    pane: Pane::Thread { id },
                    title: t.title.clone().into(),
                    agent: t.agent().map(|l| l.agent),
                },
                |d, offset, _, cx| cx.new(|_| d.ghost(offset)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_are_tagged_by_their_initials() {
        assert_eq!(initials("lualink"), "LK");
        assert_eq!(initials("vitanova279"), "V2");
        assert_eq!(initials("_corprust"), "CT");
        assert_eq!(initials("Streamer Tycoon Lobby"), "ST");
        assert_eq!(initials("777-actual"), "7A");
        assert_eq!(initials("x"), "X");
        assert_eq!(initials(""), "");
    }

    #[test]
    fn counts_like_a_stopwatch() {
        assert_eq!(elapsed(45), "45s");
        assert_eq!(elapsed(185), "3m 05s");
        assert_eq!(elapsed(3720), "1h 02m");
    }
}
