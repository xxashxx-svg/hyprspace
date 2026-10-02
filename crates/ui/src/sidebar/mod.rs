// The sidebar: search and a New thread button, then every space as a quiet folder heading over
// its threads, an Archived group under a divider, and Settings at the foot. Right-click a space
// or a thread for its menu; drag the right edge to resize. The compact sections follow zeron's
// sidebar; the drag handle follows zeron's shell (MIT, see THIRD_PARTY_NOTICES.md).

mod row;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, Focusable, FontWeight, IntoElement,
    MouseButton, MouseDownEvent, SharedString, Stateful, Window, div, prelude::*, px,
};
use hyprspace_proto::{Space, Thread};

use crate::assets::icon;
use crate::colors;
use crate::root::{Action, MenuItems, Rename, Root, Screen, SidebarDrag};
use crate::time::now_ms;
use crate::widgets;

pub const MIN_WIDTH: f32 = 200.;
pub const MAX_WIDTH: f32 = 480.;

/// The threads of `space` the search keeps, or None to hide the space.
fn visible<'a>(space: &'a Space, query: &str) -> Option<Vec<&'a Thread>> {
    let live = space.threads.iter().filter(|t| !t.archived);
    if query.is_empty() || space.name.to_lowercase().contains(query) {
        return Some(live.collect());
    }
    let hits: Vec<&Thread> = live
        .filter(|t| t.title.to_lowercase().contains(query))
        .collect();
    (!hits.is_empty()).then_some(hits)
}

/// The wash a sidebar row lifts to on hover.
pub(crate) fn row_hover() -> gpui::Hsla {
    colors::surface3().opacity(0.55)
}

/// A quiet 28px heading over a group of rows: a space's folder, or Archived.
fn heading(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(2.))
        .h(px(28.))
        .pl(px(8.))
        .pr(px(3.))
        .rounded(px(7.))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text2()))
}

/// A chevron that says whether a group is open.
fn chevron(open: bool) -> impl IntoElement {
    icon(
        if open {
            "chevron-down"
        } else {
            "chevron-right"
        },
        13.,
        colors::text3(),
    )
}

impl Root {
    pub(crate) fn sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let now = now_ms();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(8.))
            .pt(px(4.))
            .pb(px(8.));
        let mut shown = 0;
        for space in self.state.spaces.iter().filter(|s| !s.archived) {
            let Some(threads) = visible(space, &query) else {
                continue;
            };
            shown += 1;
            let open = !space.folded || !query.is_empty();
            let mut section = div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(self.space_row(space, open, cx));
            if open {
                for t in &threads {
                    section = section.child(self.thread_row(t, now, cx));
                }
                if threads.is_empty() {
                    section = section.child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child("No threads yet. Press + to start one."),
                    );
                }
            }
            list = list.child(section);
        }
        if shown == 0 {
            list = list.child(
                div()
                    .px(px(8.))
                    .py(px(6.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(if query.is_empty() {
                        "No threads yet. Start one with the button above."
                    } else {
                        "Nothing matches."
                    }),
            );
        }
        list = list.child(self.archived(now, cx));
        let width = self.state.sidebar_width.clamp(MIN_WIDTH, MAX_WIDTH);
        let searching = self.search.focus_handle(cx).is_focused(window);
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
            .child(self.nav(searching, cx))
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

    /// The top row: the search field and the square New thread button beside it.
    fn nav(&self, searching: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .pt(px(10.))
            .pb(px(6.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(32.))
                    .pl(px(10.))
                    .pr(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if searching {
                        colors::border2()
                    } else {
                        colors::border1()
                    })
                    .bg(colors::ink(if searching { 0.06 } else { 0.04 }))
                    .child(icon(
                        "search",
                        14.,
                        if searching {
                            colors::text2()
                        } else {
                            colors::text3()
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(colors::text1())
                            .child(self.search.clone()),
                    ),
            )
            .child(
                div()
                    .id("new-thread")
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
                    .hover(|s| s.bg(colors::ink(0.08)).border_color(colors::border2()))
                    .child(icon("square-pen", 15., colors::text2()))
                    .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                        r.menu = None;
                        r.pick_thread_folder(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn space_row(&self, space: &Space, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = space.id;
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
        let group: SharedString = format!("space-{id}").into();
        // the controls show on hover, except a folded section's chevron: it is the only sign
        // the section holds threads
        let hidden = |d: Stateful<Div>| d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.));
        heading(("space", id))
            .group(group.clone())
            .child(name)
            .child(
                widgets::icon_button(("space-new", id), "plus", 22.)
                    .map(hidden)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::NewThread(id), window, cx)
                    })),
            )
            .child(
                div()
                    .id(("space-fold", id))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .when(open, hidden)
                    .child(chevron(open)),
            )
            .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| r.toggle_fold(id, cx)))
            .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx))
            .into_any_element()
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
            .gap(px(1.))
            .pt(px(8.))
            .border_t_1()
            .border_color(colors::border0())
            .child(
                heading("archived")
                    .child(div().flex_1().child("Archived"))
                    .child(
                        div()
                            .pr(px(4.))
                            .font_family(hyprspace_theme::MONO)
                            .text_size(px(10.5))
                            .child(count.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(22.))
                            .child(chevron(open)),
                    )
                    .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                        r.archived_open = !r.archived_open;
                        cx.notify();
                    })),
            );
        if !open {
            return col.into_any_element();
        }
        for s in spaces {
            let id = s.id;
            let menu: MenuItems = vec![
                ("Restore".into(), Action::ArchiveSpace(id, false)),
                ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
            ];
            col = col.child(
                div()
                    .id(("archived-space", id))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(8.))
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|d| d.bg(row_hover()).text_color(colors::text1()))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .justify_center()
                            .w(px(14.))
                            .child(icon("folder", 12., colors::text3())),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(s.name.clone()))
                    .child(
                        div()
                            .font_family(hyprspace_theme::MONO)
                            .text_size(px(10.5))
                            .child(s.threads.len().to_string()),
                    )
                    .on_mouse_down(MouseButton::Right, self.context_menu(menu, cx)),
            );
        }
        for t in threads {
            col = col.child(self.thread_row(t, now, cx));
        }
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
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(8.))
                    .text_size(px(13.))
                    .text_color(if on { colors::text1() } else { colors::text2() })
                    .cursor_pointer()
                    .when(on, |d| d.bg(colors::surface3()))
                    .when(!on, |d| {
                        d.hover(|s| s.bg(row_hover()).text_color(colors::text1()))
                    })
                    .child(icon("settings", 14., colors::text3()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn space(name: &str, titles: &[&str]) -> Space {
        Space {
            name: name.into(),
            threads: titles
                .iter()
                .map(|t| Thread {
                    title: t.to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn search_keeps_matching_spaces_and_threads() {
        let s = space("HyprSpace", &["Fix the build", "Add search"]);
        assert_eq!(visible(&s, "").unwrap().len(), 2);
        assert_eq!(visible(&s, "hypr").unwrap().len(), 2);
        assert_eq!(visible(&s, "search").unwrap().len(), 1);
        assert!(visible(&s, "nothing").is_none());
    }
}
