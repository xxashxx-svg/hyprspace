// One thread in the sidebar, after T3 Code's: its project's tag and name, with the age or a
// running count; the title; then what it is doing, or its branch or folder, a dot when it waits on
// you or a tick when it finished, and its agent's mark, a ring turning around it while it works.
// Its subagents show in the card beside it on hover. A working row carries a slow sheen and,
// off screen, fades back; a waiting one a tint. Hovering shows snooze and Settle. On a shelf a thread is one quiet line.
// Click to open it, right-click for its menu.

use gpui::{
    AnyElement, App, ClickEvent, Context, DragMoveEvent, FontWeight, Hsla, IntoElement,
    MouseButton, Pixels, SharedString, Transformation, canvas, div, linear_color_stop,
    linear_gradient, percentage, prelude::*, px, relative,
};
use hyprspace_proto::{Agent, Pane, Thread, ThreadKind};

use super::card::RowCard;
use crate::workbench::PaneDrag;
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
fn elapsed(secs: u64) -> String {
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

/// A project's tag: its initials on a small square in the project's own color. A `dim` one sits
/// on a shelf.
pub(crate) fn tag(name: &str, dim: bool) -> AnyElement {
    let (fill, ink) = colors::tag(name);
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

/// A thread's agent: its mark, with a ring turning around it while it works. A terminal with no
/// agent shows the terminal glyph.
fn agent_mark(agent: Option<Agent>, working: bool) -> AnyElement {
    let badge = match agent {
        Some(a) => mark(a, 13., colors::brand(a).0).into_any_element(),
        None => icon("terminal", 12., colors::text3()).into_any_element(),
    };
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .child(badge)
        .when(working, |d| d.child(ring(16., 3.)))
        .into_any_element()
}

impl Root {
    /// The box over a row's title while that thread is renamed, in the title's own size and
    /// weight so nothing jumps. It is drawn over the title's line, its text where the title's
    /// was, so the rows below stay put. Clicks in it stay in it: placing the cursor or selecting
    /// text neither opens nor drags the row.
    fn rename_box(&self, id: u64, size: Pixels, weight: FontWeight) -> Option<AnyElement> {
        let (Rename::Thread(r), input, _) = self.rename.as_ref()?;
        (*r == id).then(|| {
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
                .bg(colors::bg())
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

    /// A small icon button on a row's hover strip.
    fn row_button(
        &self,
        id: u64,
        key: &'static str,
        glyph: &'static str,
        cx: &mut Context<Self>,
        run: fn(&mut Root, u64, &ClickEvent, &mut gpui::Window, &mut Context<Root>),
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id((key, id))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .h(px(22.))
            .min_w(px(22.))
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(11.5))
            .text_color(colors::text2())
            .hover(|s| s.bg(colors::ink(0.1)).text_color(colors::text1()))
            .child(icon(glyph, 13., colors::text2()))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |r, e: &ClickEvent, window, cx| {
                cx.stop_propagation();
                r.menu = None;
                run(r, id, e, window, cx)
            }))
    }

    /// The buttons that show on hover: snooze and Settle, or the one that undoes either.
    fn thread_buttons(&self, t: &Thread, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let id = t.id;
        if t.snooze.is_some() {
            vec![
                self.row_button(id, "wake", "sun", cx, |r, id, _, window, cx| {
                    r.snooze(id, None, window, cx)
                })
                .into_any_element(),
            ]
        } else if t.settled {
            vec![
                self.row_button(id, "unsettle", "rotate-ccw", cx, |r, id, _, window, cx| {
                    r.settle(id, false, window, cx)
                })
                .into_any_element(),
            ]
        } else {
            let pinned = t.pinned.is_some();
            vec![
                self.row_button(
                    id,
                    "pin",
                    if pinned { "pin-off" } else { "pin" },
                    cx,
                    |r, id, _, _, cx| r.toggle_pin(id, cx),
                )
                .into_any_element(),
                self.row_button(id, "snooze", "clock", cx, |r, id, e, _, cx| {
                    r.open_snooze_menu(e.position(), id, cx)
                })
                .into_any_element(),
                // the one most rows end with gets its word, as T3 Code's does
                self.row_button(id, "settle", "check", cx, |r, id, _, window, cx| {
                    r.settle(id, true, window, cx)
                })
                .px(px(6.))
                .child("Settle")
                .into_any_element(),
            ]
        }
    }

    /// What a thread's hover card says: its title, project and folder, and the agent with its
    /// model and effort in words.
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
        Some(RowCard {
            title: t.title.clone().into(),
            space: space.name.clone().into(),
            machine: space
                .machine
                .as_deref()
                .map(|m| self.machines.name(m).into()),
            path: super::card::tidy(&cwd).into(),
            branch,
            agent: t.agent().map(|l| l.agent),
            model: model.into(),
            effort: effort.map(Into::into),
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
    fn thread_menu(&self, t: &Thread) -> MenuItems {
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
            menu.push(MenuEntry::Sub {
                label: "Snooze".into(),
                entries: times,
            });
        }
        menu.push(MenuEntry::Divider);
        menu.push(MenuEntry::item(
            "Rename thread",
            Action::Rename(Rename::Thread(id)),
        ));
        menu.push(MenuEntry::Divider);
        let mirrored = space.is_some_and(|s| s.machine.is_some());
        if let Some(s) =
            space.filter(|s| s.cwd.is_some() && !mirrored && !self.work.openers.is_empty())
        {
            menu.push(MenuEntry::Sub {
                label: format!("Open {} in", s.name).into(),
                entries: self
                    .work
                    .openers
                    .iter()
                    .map(|&o| MenuEntry::item(o.name(), Action::OpenIn(o, s.id)))
                    .collect(),
            });
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
            entries: copy,
        });
        if !mirrored {
            menu.push(MenuEntry::Divider);
            menu.push(MenuEntry::item("Delete thread", Action::RemoveThread(id)));
        }
        menu
    }

    /// A thread in the list, after T3 Code's: its agent's mark and its project's name, with the
    /// age or a running count on the right; the title; then what it is doing, or its branch or
    /// folder, and its model.
    pub(crate) fn thread_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
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
        // a terminal thread hears from its hooks; a structured one reads its own transcript
        let (running, doing) = match self.views.get(&id) {
            Some(View::Structured(v)) => {
                let v = v.read(cx);
                (v.elapsed(), v.doing())
            }
            _ => (
                self.turns
                    .get(&id)
                    .filter(|_| busy)
                    .map(|t| t.elapsed().as_secs()),
                self.activity.get(&id).and_then(|a| a.doing.clone()),
            ),
        };
        // the line says what it does, or what it concluded until the next turn
        let doing = match background {
            0 => doing.filter(|_| status != Status::Idle),
            1 => Some("1 subagent running".into()),
            n => Some(format!("{n} subagents running")),
        };
        let agent = t.agent().map(|l| l.agent);
        let (space, machine) = self
            .state
            .thread(id)
            .map(|(s, _)| {
                (
                    s.name.clone(),
                    s.machine.as_deref().map(|m| self.machines.name(m)),
                )
            })
            .unwrap_or_default();
        let cwd = match &t.kind {
            ThreadKind::Terminal { cwd, .. } => cwd.clone(),
            ThreadKind::Structured { launch } => launch.cwd.clone(),
        };
        // a branch when the folder is a repo, otherwise the folder; the icon says which
        let host = self.machines.place(id).map(|(place, branch)| {
            let icon = if *branch { "git-branch" } else { "folder" };
            (icon, place.clone())
        });
        let place = match self.git.get(&cwd).map(|g| &g.branch) {
            _ if let Some(host) = host => host,
            Some(b) if b.is_repo && !b.branch.is_empty() => ("git-branch", b.branch.clone()),
            _ => (
                "folder",
                cwd.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            ),
        };
        let title: AnyElement = match self.rename_box(id, px(13.5), FontWeight::MEDIUM) {
            Some(rename) => rename,
            None => div()
                .min_w_0()
                .truncate()
                .text_size(px(13.5))
                .font_weight(if recede {
                    FontWeight::NORMAL
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(colors::text1())
                .child(t.title.clone())
                .into_any_element(),
        };
        let group: SharedString = format!("row-{id}").into();
        let right = match running {
            Some(secs) => div()
                .font_family(MONO)
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::busy())
                .child(elapsed(secs)),
            None => div()
                .text_size(px(11.))
                .text_color(colors::text3())
                .when(t.last_touch() > 0, |d| d.child(ago(t.last_touch(), now))),
        }
        // the buttons take its place on hover
        .flex_none()
        .group_hover(group.clone(), |s| s.opacity(0.));
        let state = match status {
            _ if t.resume_at.is_some() => {
                Some(icon("clock", 12., colors::busy()).into_any_element())
            }
            Status::Waiting => Some(
                widgets::status_dot(status)
                    .looping(1000, |d, t| d.opacity(0.35 + 0.65 * (t * 2.0 - 1.0).abs()))
                    .into_any_element(),
            ),
            Status::Done if background == 0 => Some(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(12.))
                    .rounded_full()
                    .bg(colors::ok())
                    .child(icon("check", 9., colors::on_accent()))
                    .into_any_element(),
            ),
            Status::Failed => Some(widgets::status_dot(status).into_any_element()),
            Status::Done | Status::Working | Status::Idle => None,
        };
        let foot = match doing {
            Some(doing) => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if waiting {
                    colors::waiting()
                } else {
                    colors::text2()
                })
                .child(doing),
            None => div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(icon(place.0, 11., colors::text3()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(MONO)
                        .text_size(px(10.5))
                        .child(place.1),
                ),
        };
        let menu = self.thread_menu(t);
        let buttons = self.thread_buttons(t, cx);
        let row = div()
            .id(("thread", id))
            .group(group.clone())
            .relative()
            .overflow_hidden()
            .flex()
            .flex_none()
            .flex_col()
            .gap(px(3.))
            .pt(px(8.))
            .px(px(8.))
            .pb(px(9.))
            .rounded(px(8.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected && waiting, |d| {
                d.bg(colors::waiting().opacity(0.07))
            })
            .when(recede, |d| d.opacity(0.7))
            .when(!selected, |d| {
                d.hover(move |s| {
                    let s = s.bg(row_hover());
                    if recede { s.opacity(1.) } else { s }
                })
            })
            .when(working, |d| d.child(sheen()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .min_w_0()
                    .child(tag(&space, false))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(colors::text3())
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
                    .when(t.pinned.is_some() && t.active(), |d| {
                        d.child(icon("pin", 11., colors::text3()))
                    })
                    .child(right),
            )
            .child(title)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(foot)
                    .children(state)
                    .child(agent_mark(agent, working)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(5.))
                    .right(px(6.))
                    .flex()
                    .gap(px(2.))
                    .p(px(1.))
                    .rounded(px(7.))
                    .bg(colors::surface3())
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .children(buttons),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_thread(id, window, cx)
            }))
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
            .on_drag_move(cx.listener(move |r, e: &DragMoveEvent<PaneDrag>, _, cx| {
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
                r.drop_at = None;
                r.drop_on_thread(d, id, below, cx)
            }))
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
    pub(crate) fn shelved_row(&self, t: &Thread, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let id = t.id;
        let selected = self.screen == Screen::Thread(id);
        let group: SharedString = format!("shelved-{id}").into();
        let space = self
            .state
            .thread(id)
            .map(|(s, _)| s.name.clone())
            .unwrap_or_default();
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
            .rounded(px(7.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colors::surface3()))
            .when(!selected, |d| d.hover(|s| s.bg(row_hover())))
            .child(tag(&space, true))
            .child(match self.rename_box(id, px(12.5), FontWeight::NORMAL) {
                Some(rename) => div().flex_1().min_w_0().child(rename),
                None => div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.5))
                    .text_color(colors::text3())
                    .child(t.title.clone()),
            })
            .children(when.map(|w| {
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .group_hover(group.clone(), |s| s.opacity(0.))
                    .child(w)
            }))
            .child(
                div()
                    .absolute()
                    .right(px(6.))
                    .flex()
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .children(self.thread_buttons(t, cx)),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_thread(id, window, cx)
            }))
            .on_mouse_down(
                MouseButton::Right,
                self.context_menu(self.thread_menu(t), cx),
            )
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
