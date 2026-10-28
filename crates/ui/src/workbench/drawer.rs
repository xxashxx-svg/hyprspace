use gpui::{
    AnyElement, AppContext, ClickEvent, Context, DragMoveEvent, Entity, Focusable, FontWeight,
    IntoElement, SharedString, StyleRefinement, Subscription, Window, div, prelude::*,
};
use hyprspace_proto::{Agent, Command, SessionId, ThreadKind};
use hyprspace_theme::MONO;

use super::ToggleTerminal;
use super::frame::Nothing;
use super::header::{head_button, short};
use crate::assets::icon;
use crate::colors;
use crate::root::{Root, View};
use crate::terminal::{TerminalEvent, TerminalView};
use crate::zoom::px;

const MIN: f32 = 120.;
const CHAT_MIN: f32 = 200.;

struct Shell {
    session: SessionId,
    view: Entity<TerminalView>,
    n: usize,
    _sub: Subscription,
}

pub(crate) struct Drawer {
    shells: Vec<Shell>,
    active: usize,
    next: usize,
    pub(crate) open: bool,
}

impl Drawer {
    fn new() -> Self {
        Self {
            shells: Vec::new(),
            active: 0,
            next: 1,
            open: true,
        }
    }

    fn shown(&self) -> Option<&Shell> {
        self.shells.get(self.active)
    }
}

pub struct DrawerDrag;

impl Root {
    pub(crate) fn toggle_terminal(
        &mut self,
        _: &ToggleTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.structured_on_screen() else {
            return;
        };
        if self.machines.remote(thread).is_some() {
            self.notify(
                "The terminal opens only for threads on this computer.".into(),
                cx,
            );
            return;
        }
        match self.work.drawers.get_mut(&thread) {
            Some(d) => d.open = !d.open,
            None => {
                self.work.drawers.insert(thread, Drawer::new());
                self.add_shell(thread, None, window, cx);
            }
        }
        self.focus_drawer(thread, window, cx);
    }

    /// A new shell under the thread that signs its agent in again.
    pub(crate) fn sign_in(&mut self, thread: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(agent) = self
            .state
            .thread(thread)
            .and_then(|(_, t)| t.agent().map(|l| l.agent))
        else {
            return;
        };
        if self.machines.remote(thread).is_some() {
            self.notify(
                format!(
                    "Sign {} in on the computer this thread runs on.",
                    agent.name()
                ),
                cx,
            );
            return;
        }
        self.work
            .drawers
            .entry(thread)
            .or_insert_with(Drawer::new)
            .open = true;
        self.add_shell(thread, Some(agent), window, cx);
        self.focus_drawer(thread, window, cx);
    }

    fn add_shell(
        &mut self,
        thread: u64,
        sign_in: Option<Agent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(cwd) = self.state.thread(thread).and_then(|(_, t)| match &t.kind {
            ThreadKind::Structured { launch } => Some(launch.cwd.clone()),
            _ => None,
        }) else {
            return;
        };
        let session = SessionId(self.next_session);
        self.next_session += 1;
        let client = self.client.clone();
        let view = cx.new(|cx| {
            let mut v = TerminalView::new(session, client, cwd, None, None, sign_in, cx);
            v.offer_chat();
            v
        });
        let sub = cx.subscribe_in(
            &view,
            window,
            move |root, _, e: &TerminalEvent, window, cx| match e {
                TerminalEvent::AddToChat(text) => {
                    if let Some(View::Structured(v)) = root.views.get(&thread) {
                        v.update(cx, |v, cx| v.add_output(text, window, cx));
                    }
                }
                TerminalEvent::OpenFile {
                    path, line: None, ..
                } if crate::attach::previews(path) => root.show_media(path.clone(), cx),
                TerminalEvent::OpenFile { path, line, col } => {
                    root.open_file(path.clone(), *line, *col, cx)
                }
            },
        );
        if let Some(d) = self.work.drawers.get_mut(&thread) {
            d.shells.push(Shell {
                session,
                view,
                n: d.next,
                _sub: sub,
            });
            d.next += 1;
            d.active = d.shells.len() - 1;
        }
    }

    fn focus_drawer(&mut self, thread: u64, window: &mut Window, cx: &mut Context<Self>) {
        let focus = match self.work.drawers.get(&thread) {
            Some(d) if d.open => d.shown().map(|s| s.view.focus_handle(cx)),
            _ => match self.views.get(&thread) {
                Some(View::Structured(v)) => Some(v.focus_handle(cx)),
                _ => None,
            },
        };
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn close_shell(
        &mut self,
        thread: u64,
        session: SessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.client.send(Command::Close { id: session });
        self.forget_shell(session);
        self.focus_drawer(thread, window, cx);
    }

    fn forget_shell(&mut self, session: SessionId) {
        self.work.drawers.retain(|_, d| {
            if let Some(ix) = d.shells.iter().position(|s| s.session == session) {
                d.shells.remove(ix);
                if d.active >= ix && d.active > 0 {
                    d.active -= 1;
                }
            }
            !d.shells.is_empty()
        });
    }

    pub(crate) fn show_terminals(&self, cx: &gpui::App) {
        let on = match self.screen {
            crate::root::Screen::Thread(id) => Some(id),
            _ => None,
        };
        for (thread, view) in &self.views {
            if let View::Terminal(v) = view {
                v.read(cx).show(Some(*thread) == on);
            }
        }
        for (thread, d) in &self.work.drawers {
            for (ix, s) in d.shells.iter().enumerate() {
                s.view
                    .read(cx)
                    .show(Some(*thread) == on && d.open && ix == d.active);
            }
        }
    }

    pub(crate) fn drawer_terminal(&self, id: SessionId) -> Option<&Entity<TerminalView>> {
        self.work
            .drawers
            .values()
            .flat_map(|d| &d.shells)
            .find(|s| s.session == id)
            .map(|s| &s.view)
    }

    pub(crate) fn drawer_exited(&mut self, id: SessionId, cx: &mut Context<Self>) {
        self.forget_shell(id);
        cx.notify();
    }

    pub(crate) fn close_drawer(&mut self, thread: u64) {
        if let Some(d) = self.work.drawers.remove(&thread) {
            for s in d.shells {
                self.client.send(Command::Close { id: s.session });
            }
        }
    }

    fn tabs(&self, thread: u64, d: &Drawer, cx: &mut Context<Self>) -> Vec<AnyElement> {
        d.shells
            .iter()
            .enumerate()
            .map(|(ix, s)| {
                let on = ix == d.active;
                let session = s.session;
                let group: SharedString = format!("drawer-tab-{}", session.0).into();
                div()
                    .id(("drawer-tab", session.0 as usize))
                    .group(group.clone())
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.))
                    .h(px(24.))
                    .pl(px(8.))
                    .pr(px(3.))
                    .rounded(px(6.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .text_color(if on { colors::text1() } else { colors::text3() })
                    .when(on, |d| d.bg(colors::surface3()))
                    .when(!on, |d| {
                        d.hover(|s| s.bg(colors::surface2()).text_color(colors::text1()))
                    })
                    .child(icon(
                        "square-terminal",
                        12.,
                        if on { colors::text2() } else { colors::text3() },
                    ))
                    .child(format!("Terminal {}", s.n))
                    .child(
                        div()
                            .id(("drawer-tab-x", session.0 as usize))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .rounded(px(4.))
                            .opacity(if on { 1. } else { 0. })
                            .group_hover(group, |s| s.opacity(1.))
                            .hover(|s| s.bg(colors::ink(0.1)))
                            .child(icon("x", 11., colors::text3()))
                            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                r.close_shell(thread, session, window, cx)
                            })),
                    )
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        if let Some(d) = r.work.drawers.get_mut(&thread) {
                            d.active = ix;
                        }
                        r.focus_drawer(thread, window, cx)
                    }))
                    .into_any_element()
            })
            .collect()
    }

    pub(super) fn with_drawer(
        &self,
        thread: u64,
        chat: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some((d, shown)) = self
            .work
            .drawers
            .get(&thread)
            .filter(|d| d.open)
            .and_then(|d| Some((d, d.shown()?)))
        else {
            return chat;
        };
        let cwd = match self.state.thread(thread).map(|(_, t)| &t.kind) {
            Some(ThreadKind::Structured { launch }) => short(&launch.cwd),
            _ => String::new(),
        };
        let height = self.work.drawer_height;
        let head = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .h(px(34.))
            .pl(px(6.))
            .pr(px(6.))
            .children(self.tabs(thread, d, cx))
            .child(
                head_button(("drawer-new", 0), "plus", false)
                    .ml(px(2.))
                    .tooltip(crate::widgets::tip("New terminal"))
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.add_shell(thread, None, window, cx);
                        r.focus_drawer(thread, window, cx)
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .mr(px(6.))
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(cwd),
            )
            .child(
                head_button(("drawer-close", 0), "x", false)
                    .tooltip(crate::widgets::tip("Hide the terminal"))
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                        r.toggle_terminal(&ToggleTerminal, window, cx)
                    })),
            );
        let panel = div()
            .relative()
            .flex_none()
            .h(px(height))
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(colors::border1())
            .child(
                div()
                    .id("drawer-edge")
                    .absolute()
                    .top(px(-3.))
                    .left_0()
                    .right_0()
                    .h(px(6.))
                    .cursor_row_resize()
                    .hover(|s| s.bg(colors::accent().opacity(0.45)))
                    .on_drag(DrawerDrag, |_, _, _, cx| cx.new(|_| Nothing)),
            )
            .child(head)
            .child(
                div().flex_1().min_h_0().px(px(8.)).pb(px(6.)).child(
                    shown
                        .view
                        .clone()
                        .cached(StyleRefinement::default().size_full()),
                ),
            );
        div()
            .id("with-drawer")
            .size_full()
            .flex()
            .flex_col()
            .on_drag_move(cx.listener(|r, e: &DragMoveEvent<DrawerDrag>, _, cx| {
                let y: f32 = e.event.position.y.into();
                let (top, bottom): (f32, f32) = (e.bounds.top().into(), e.bounds.bottom().into());
                let z = crate::zoom::zoom();
                let most = ((bottom - top) / z - CHAT_MIN).max(MIN);
                r.work.drawer_height = ((bottom - y) / z).clamp(MIN, most);
                cx.notify();
            }))
            .child(div().flex_1().min_h_0().child(chat))
            .child(panel)
            .into_any_element()
    }
}
