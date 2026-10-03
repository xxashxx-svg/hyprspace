// The sidebar, after the Tauri app's Rail (src/components/Rail.tsx at v0.21.1): a search box and
// a New thread button on top, every space as a section that folds open to its threads, an
// Archived group under a divider, and Settings at the foot. Clicking a space opens it and its
// arrow folds it. Right-click a space or a thread for its menu; drag the right edge to resize.
// The drag handle follows zeron's shell (MIT, see THIRD_PARTY_NOTICES.md).

mod row;

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton,
    MouseDownEvent, SharedString, Transformation, Window, div, percentage, prelude::*, px,
};
use hyprspace_proto::{Pane, Space, Thread};
use hyprspace_theme::MONO;

use crate::assets::icon;
use crate::colors;
use crate::palette::TogglePalette;
use crate::root::{Action, MenuItems, Rename, Root, Screen, SidebarDrag};
use crate::time::now_ms;

pub const MIN_WIDTH: f32 = 200.;
pub const MAX_WIDTH: f32 = 480.;

/// The wash a sidebar row lifts to on hover.
pub(crate) fn row_hover() -> gpui::Hsla {
    colors::surface3().opacity(0.55)
}

/// The arrow that folds a section, turned down while it is open.
fn twist(open: bool) -> impl IntoElement {
    icon("chevron-right", 12., colors::text3()).with_transformation(Transformation::rotate(
        percentage(if open { 0.25 } else { 0. }),
    ))
}

/// A 22px button in a space's header that only shows on hover, or always on the active space.
fn header_button(
    id: (&'static str, u64),
    glyph: &str,
    shown: bool,
    group: SharedString,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface3()))
        .when(!shown, |d| {
            d.opacity(0.).group_hover(group, |s| s.opacity(1.))
        })
        .child(icon(glyph, 13., colors::text3()))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl Root {
    pub(crate) fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let now = now_ms();
        let q = self.search.read(cx).text().trim().to_lowercase();
        let active = self.current_space();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(8.))
            .pt(px(2.))
            .pb(px(4.));
        let mut shown = 0;
        for space in self.state.spaces.iter().filter(|s| !s.archived) {
            let threads: Vec<&Thread> = space
                .threads
                .iter()
                .filter(|t| !t.archived)
                .filter(|t| q.is_empty() || t.title.to_lowercase().contains(&q))
                .collect();
            if !q.is_empty() && threads.is_empty() && !space.name.to_lowercase().contains(&q) {
                continue;
            }
            shown += 1;
            // a search opens every space with a hit
            let open = if q.is_empty() {
                !space.folded
            } else {
                !threads.is_empty()
            };
            list = list.child(self.space_section(
                space,
                &threads,
                open,
                active == Some(space.id),
                now,
                cx,
            ));
        }
        if shown == 0 {
            list = list.child(
                div()
                    .px(px(10.))
                    .py(px(6.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(if q.is_empty() {
                        "No threads yet."
                    } else {
                        "Nothing matches."
                    }),
            );
        }
        list = list.child(self.archived(now, cx));
        let width = self.state.sidebar_width.clamp(MIN_WIDTH, MAX_WIDTH);
        div()
            .relative()
            .flex_none()
            .w(px(width))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors::bg())
            .border_r_1()
            .border_color(colors::border0())
            .child(self.nav_row(&q, cx))
            .child(
                div()
                    .id("sidebar-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(self.foot(cx))
            .child(
                div()
                    .id("sidebar-resize")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(-3.))
                    .w(px(6.))
                    .cursor_col_resize()
                    .hover(|s| s.bg(colors::accent().opacity(0.45)))
                    .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| DragGhost)),
            )
            .into_any_element()
    }

    /// The top row: the search box, with the palette's shortcut as a keycap until something is
    /// typed, and a square New thread button that asks for a folder.
    fn nav_row(&self, q: &str, cx: &mut Context<Self>) -> AnyElement {
        let end = if q.is_empty() {
            div()
                .id("search-key")
                .flex_none()
                .h(px(20.))
                .px(px(5.))
                .flex()
                .items_center()
                .rounded(px(5.))
                .border_1()
                .border_color(colors::border1())
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .cursor_pointer()
                .hover(|s| {
                    s.text_color(colors::text1())
                        .border_color(colors::border2())
                })
                .child(if cfg!(target_os = "macos") {
                    "Cmd K"
                } else {
                    "Ctrl K"
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                    r.toggle_palette(&TogglePalette, window, cx)
                }))
        } else {
            div()
                .id("search-clear")
                .flex_none()
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.08)))
                .child(icon("x", 12., colors::text3()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                    r.search.update(cx, |i, cx| i.set_text("", cx));
                    cx.notify();
                }))
        };
        let focused = self.search.focus_handle(cx);
        let glass = if q.is_empty() {
            colors::text3()
        } else {
            colors::accent()
        };
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(10.))
            .mb(px(4.))
            .border_b_1()
            .border_color(colors::border0())
            .child(
                div()
                    .id("sidebar-search")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(32.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .hover(|s| s.bg(colors::ink(0.06)))
                    .cursor_text()
                    .child(icon("search", 14., glass))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(colors::text1())
                            .child(self.search.clone()),
                    )
                    .child(end)
                    .on_click(move |_, window, cx| window.focus(&focused, cx)),
            )
            .child(
                div()
                    .id("sidebar-new")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(32.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)))
                    .child(icon("square-pen", 15., colors::text2()))
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                        r.menu = None;
                        r.pick_thread_folder(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// A space: its header, and while open its git summary and threads, indented under it.
    fn space_section(
        &self,
        space: &Space,
        threads: &[&Thread],
        open: bool,
        active: bool,
        now: u64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = space.id;
        let group: SharedString = format!("space-{id}").into();
        let name: AnyElement = match &self.rename {
            Some((Rename::Space(r), input, _)) if *r == id => div()
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(6.))
                .rounded(px(6.))
                .border_1()
                .border_color(colors::accent())
                .bg(colors::bg())
                .text_color(colors::text1())
                .child(input.clone())
                .into_any_element(),
            _ => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(space.name.clone())
                .into_any_element(),
        };
        let mut menu: MenuItems = vec![
            ("New thread".into(), Action::NewThread(id)),
            ("New terminal".into(), Action::NewTerminal(id)),
        ];
        // the same apps, in the same order, as the Open button's menu
        if space.cwd.is_some() {
            menu.extend(self.work.openers.iter().map(|&o| {
                (
                    format!("Open in {}", o.name()).into(),
                    Action::OpenIn(o, id),
                )
            }));
        }
        menu.extend([
            ("Rename".into(), Action::Rename(Rename::Space(id))),
            ("Archive".into(), Action::ArchiveSpace(id, true)),
            ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
        ]);
        let header = div()
            .id(("space", id))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(30.))
            .pl(px(2.))
            .pr(px(4.))
            .rounded(px(7.))
            .text_size(px(13.))
            .font_weight(if active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(if active {
                colors::text1()
            } else {
                colors::text2()
            })
            .cursor_pointer()
            .hover(|s| s.bg(row_hover()).text_color(colors::text1()))
            .child(
                div()
                    .id(("space-fold", id))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(20.))
                    .rounded(px(5.))
                    .hover(|s| s.bg(colors::surface3()))
                    .child(twist(open))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.toggle_fold(id, cx))),
            )
            .child(name)
            .child(
                header_button(("space-new", id), "plus", active, group.clone()).on_click(
                    cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::NewThread(id), window, cx)
                    }),
                ),
            )
            .child(
                header_button(("space-archive", id), "archive", active, group).on_click(
                    cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::ArchiveSpace(id, true), window, cx)
                    }),
                ),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.menu = None;
                r.open_space(id, window, cx)
            }))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx));
        let mut section = div().flex().flex_col().child(header);
        if open {
            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .mt(px(2.))
                .mb(px(6.))
                .ml(px(12.))
                .children(self.summary(space));
            for t in threads {
                body = body.child(self.thread_row(t, now, cx));
            }
            if threads.is_empty() {
                body = body.child(
                    div()
                        .px(px(8.))
                        .pt(px(4.))
                        .pb(px(6.))
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("No threads yet"),
                );
            }
            section = section.child(body);
        }
        section.into_any_element()
    }

    /// The working tree under an open space: file count and line deltas. A clean tree says
    /// nothing; the absence of the line is the message.
    fn summary(&self, space: &Space) -> Option<AnyElement> {
        let status = self.git.get(space.cwd.as_ref()?)?;
        if status.changes.is_empty() {
            return None;
        }
        let files = status.changes.len();
        let added: u32 = status.changes.iter().map(|c| c.added).sum();
        let removed: u32 = status.changes.iter().map(|c| c.removed).sum();
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .pt(px(2.))
                .pb(px(4.))
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(colors::text3())
                .child(format!(
                    "{files} {}",
                    if files == 1 { "file" } else { "files" }
                ))
                .child(
                    div()
                        .text_color(colors::diff_add())
                        .child(format!("+{added}")),
                )
                .child(
                    div()
                        .text_color(colors::diff_del())
                        .child(format!("\u{2212}{removed}")),
                )
                .into_any_element(),
        )
    }

    /// Opens a space: the thread its grid shows first, or its composer when the grid is empty.
    pub(crate) fn open_space(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(id) {
            s.folded = false;
        }
        let first = self.live_panes(id).into_iter().find_map(|p| match p {
            Pane::Thread { id } => Some(id),
            _ => None,
        });
        match first {
            Some(thread) => self.open_thread(thread, window, cx),
            None => self.compose(Some(id), window, cx),
        }
        self.save();
        self.git_poll(cx);
    }

    /// Opens `items` as a menu where the right-click was.
    pub(crate) fn context_menu(
        &self,
        items: MenuItems,
        cx: &mut Context<Self>,
    ) -> impl Fn(&MouseDownEvent, &mut Window, &mut gpui::App) + 'static {
        cx.listener(move |r, e: &MouseDownEvent, _, cx| {
            r.menu = Some((e.position, items.clone()));
            cx.stop_propagation();
            cx.notify();
        })
    }

    /// Archived spaces, and archived threads of live ones, parked under a divider.
    fn archived(&self, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let spaces: Vec<&Space> = self.state.spaces.iter().filter(|s| s.archived).collect();
        let threads: Vec<&Thread> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .flat_map(|s| s.threads.iter().filter(|t| t.archived))
            .collect();
        let count = spaces.len() + threads.len();
        if count == 0 {
            return div().into_any_element();
        }
        let open = self.archived_open;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .mt(px(6.))
            .pt(px(6.))
            .border_t_1()
            .border_color(colors::border1())
            .child(
                div()
                    .id("archived")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .h(px(30.))
                    .pl(px(2.))
                    .pr(px(4.))
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|s| s.bg(row_hover()).text_color(colors::text1()))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(20.))
                            .child(twist(open)),
                    )
                    .child(
                        div()
                            .mr(px(2.))
                            .child(icon("archive", 13., colors::text3())),
                    )
                    .child(div().flex_1().child("Archived"))
                    .child(
                        div()
                            .pr(px(4.))
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                        r.archived_open = !r.archived_open;
                        cx.notify();
                    })),
            );
        if !open {
            return col.into_any_element();
        }
        let mut body = div().flex().flex_col().gap(px(2.)).mt(px(2.)).ml(px(12.));
        for s in spaces {
            let id = s.id;
            let group: SharedString = format!("arch-{id}").into();
            let menu: MenuItems = vec![
                ("Restore".into(), Action::ArchiveSpace(id, false)),
                ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
            ];
            let n = s.threads.len();
            body = body.child(
                div()
                    .id(("archived-space", id))
                    .group(group.clone())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|d| d.bg(row_hover()).text_color(colors::text1()))
                    .child(div().flex_1().min_w_0().truncate().child(s.name.clone()))
                    // the count gives way to a Restore pill on hover
                    .when(n > 0, |d| {
                        d.child(
                            div()
                                .flex_none()
                                .font_family(MONO)
                                .text_size(px(10.5))
                                .group_hover(group.clone(), |s| s.opacity(0.))
                                .child(format!(
                                    "{n} {}",
                                    if n == 1 { "thread" } else { "threads" }
                                )),
                        )
                    })
                    .child(
                        div()
                            .id(("restore", id))
                            .absolute()
                            .right(px(6.))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .h(px(20.))
                            .px(px(7.))
                            .rounded(px(5.))
                            .bg(colors::surface3())
                            .text_size(px(11.))
                            .text_color(colors::text1())
                            .opacity(0.)
                            .group_hover(group, |s| s.opacity(1.))
                            .child(icon("archive-restore", 11., colors::text1()))
                            .child("Restore")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                                r.act(Action::ArchiveSpace(id, false), window, cx)
                            })),
                    )
                    .relative()
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.open_space(id, window, cx)
                    }))
                    .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx)),
            );
        }
        for t in threads {
            body = body.child(self.thread_row(t, now, cx));
        }
        col = col.child(body);
        col.into_any_element()
    }

    fn foot(&self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.screen == Screen::Settings;
        div()
            .flex_none()
            .flex()
            .items_center()
            .px(px(8.))
            .pt(px(7.))
            .pb(px(8.))
            .border_t_1()
            .border_color(colors::border0())
            .child(
                div()
                    .id("settings")
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .text_color(if on { colors::text1() } else { colors::text2() })
                    .cursor_pointer()
                    .when(on, |d| d.bg(colors::surface3()))
                    .when(!on, |d| {
                        d.hover(|s| s.bg(row_hover()).text_color(colors::text1()))
                    })
                    .child(icon("settings", 16., colors::text3()))
                    .child("Settings")
                    .on_click(
                        cx.listener(|r, _: &ClickEvent, window, cx| r.open_settings(window, cx)),
                    ),
            )
            .into_any_element()
    }
}

/// What follows the pointer while the sidebar edge is dragged: nothing visible.
struct DragGhost;

impl gpui::Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
