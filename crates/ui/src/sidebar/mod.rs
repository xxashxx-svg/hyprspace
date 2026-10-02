// The sidebar: search and a new-space button, then every space as a section that folds open to
// its threads, an Archived group under a divider, and Appearance at the foot. Right-click a
// space or a thread for its menu; drag the right edge to resize. Sizes follow the Tauri app's
// rail.css; the drag handle follows zeron's shell (MIT, see THIRD_PARTY_NOTICES.md).

mod appearance;
mod row;

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton,
    MouseDownEvent, SharedString, Window, div, prelude::*, px,
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

impl Root {
    pub(crate) fn sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let now = now_ms();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(8.))
            .pt(px(2.))
            .pb(px(4.));
        let mut shown = 0;
        for space in self.state.spaces.iter().filter(|s| !s.archived) {
            let Some(threads) = visible(space, &query) else {
                continue;
            };
            shown += 1;
            let open = !space.folded || !query.is_empty();
            list = list.child(self.space_row(space, open, threads.len(), cx));
            if !open {
                continue;
            }
            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .mt(px(2.))
                .mb(px(6.))
                .ml(px(12.));
            for t in &threads {
                body = body.child(self.thread_row(t, None, now, cx));
            }
            if threads.is_empty() {
                body = body.child(
                    div()
                        .px(px(8.))
                        .pt(px(4.))
                        .pb(px(6.))
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("No threads yet. Press + to start one."),
                );
            }
            list = list.child(body);
        }
        if shown == 0 {
            list = list.child(
                div()
                    .px(px(10.))
                    .py(px(6.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child(if query.is_empty() {
                        "Add a project folder to start."
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

    /// The top row: the search field and a square button for a new space beside it.
    fn nav(&self, searching: bool, cx: &mut Context<Self>) -> AnyElement {
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
                        colors::accent().opacity(0.5)
                    } else {
                        colors::border1()
                    })
                    .bg(colors::ink(0.04))
                    .text_color(if searching {
                        colors::accent()
                    } else {
                        colors::text3()
                    })
                    .child(icon(
                        "search",
                        14.,
                        if searching {
                            colors::accent()
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
                    .id("sidebar-add")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(32.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::ink(0.04))
                    .text_color(colors::text2())
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
                    .child(icon("square-pen", 15., colors::text2()))
                    .on_click(cx.listener(|r, e: &ClickEvent, _, cx| {
                        r.menu = Some((
                            e.position(),
                            vec![
                                ("Add a project folder".into(), Action::AddProject),
                                ("New open space".into(), Action::NewOpenSpace),
                            ],
                        ));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn space_row(
        &self,
        space: &Space,
        open: bool,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = space.id;
        let active = match self.screen {
            Screen::Compose(Some(s)) => s == id,
            Screen::Thread(t) => space.threads.iter().any(|x| x.id == t),
            Screen::Compose(None) => false,
        };
        let name: AnyElement = match &self.rename {
            Some((Rename::Space(r), input, _)) if *r == id => div()
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .px(px(8.))
                .rounded(px(7.))
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
        let menu: MenuItems = vec![
            ("New thread".into(), Action::NewThread(id)),
            ("New terminal".into(), Action::NewTerminal(id)),
            ("Rename".into(), Action::Rename(Rename::Space(id))),
            ("Archive".into(), Action::ArchiveSpace(id, true)),
            ("Remove from the sidebar".into(), Action::RemoveSpace(id)),
        ];
        let group: SharedString = format!("space-{id}").into();
        div()
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
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(20.))
                    .text_color(colors::text3())
                    .child(icon(
                        if open {
                            "chevron-down"
                        } else {
                            "chevron-right"
                        },
                        13.,
                        colors::text3(),
                    )),
            )
            .child(name)
            .when(count > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .pr(px(4.))
                        .font_family(hyprspace_theme::MONO)
                        .text_size(px(10.5))
                        .text_color(colors::text3())
                        .child(count.to_string()),
                )
            })
            .child(
                widgets::icon_button(("space-new", id), "plus", 22.)
                    .when(!active, |d| {
                        d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.))
                    })
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.act(Action::NewThread(id), window, cx)
                    })),
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
        let threads: Vec<(&Space, &Thread)> = self
            .state
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .flat_map(|s| s.threads.iter().filter(|t| t.archived).map(move |t| (s, t)))
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
                            .items_center()
                            .justify_center()
                            .size(px(20.))
                            .child(icon(
                                if open {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                },
                                13.,
                                colors::text3(),
                            )),
                    )
                    .child(icon("archive", 13., colors::text3()))
                    .child(div().flex_1().ml(px(2.)).child("Archived"))
                    .child(
                        div()
                            .pr(px(4.))
                            .font_family(hyprspace_theme::MONO)
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
                    .gap_2()
                    .h(px(30.))
                    .ml(px(12.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|d| d.bg(row_hover()).text_color(colors::text1()))
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
        let mut body = div().flex().flex_col().gap(px(2.)).ml(px(12.));
        for (s, t) in threads {
            body = body.child(self.thread_row(t, Some(&s.name), now, cx));
        }
        col.child(body).into_any_element()
    }

    fn foot(&self, cx: &mut Context<Self>) -> AnyElement {
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
                    .id("appearance")
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .text_size(px(12.5))
                    .text_color(colors::text2())
                    .cursor_pointer()
                    .hover(|s| s.bg(row_hover()).text_color(colors::text1()))
                    .child(icon("palette", 14., colors::text3()))
                    .child("Appearance")
                    .on_click(cx.listener(|r, e: &ClickEvent, _, cx| {
                        r.appearance_at = Some(e.position());
                        cx.notify();
                    })),
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
