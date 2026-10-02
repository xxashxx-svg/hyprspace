// The sidebar, drawn like zeron's: every space as a folder heading over its threads as cards, an
// Archived group under a divider, and Settings at the foot. Its toggle, search and New thread
// sit above it in the title row. Right-click a space or a thread, or press a space's dots, for
// its menu; drag the right edge to resize. The drag handle follows zeron's shell (MIT, see
// THIRD_PARTY_NOTICES.md).

mod row;

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, FontWeight, IntoElement, MouseButton,
    MouseDownEvent, SharedString, Stateful, Window, div, prelude::*, px, relative,
};
use hyprspace_proto::{Space, Thread};

use crate::assets::icon;
use crate::colors;
use crate::root::{Action, MenuItems, Rename, Root, Screen, SidebarDrag};
use crate::time::now_ms;
use crate::widgets;

pub const MIN_WIDTH: f32 = 200.;
pub const MAX_WIDTH: f32 = 480.;

/// The wash a sidebar row lifts to on hover.
pub(crate) fn row_hover() -> gpui::Hsla {
    colors::surface3().opacity(0.55)
}

/// A 30px heading over a group of rows: a space's folder, or Archived.
fn heading(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(2.))
        .h(px(30.))
        .pl(px(10.))
        .pr(px(3.))
        .rounded(px(7.))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text2())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text1()))
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
    pub(crate) fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let now = now_ms();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .px(px(8.))
            .pt(px(8.))
            .pb(px(8.));
        let mut shown = 0;
        for space in self.state.spaces.iter().filter(|s| !s.archived) {
            let threads: Vec<&Thread> = space.threads.iter().filter(|t| !t.archived).collect();
            shown += 1;
            let open = !space.folded;
            let mut section = div()
                .flex()
                .flex_col()
                .gap(px(2.))
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
                    .child("No threads yet. Start one with the + above."),
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
                .flex_none()
                .max_w(relative(0.6))
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
        let hidden = |d: Stateful<Div>| d.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.));
        // where the folder is, the way zeron names a space's device after an @
        let path = space.cwd.as_deref().map(|p| {
            div()
                .flex_shrink(1.)
                .min_w_0()
                .ml(px(6.))
                .truncate()
                .text_size(px(11.5))
                .font_weight(FontWeight::NORMAL)
                .text_color(colors::text3())
                .child(format!("@ {}", crate::panes::short(p)))
        });
        let dots = menu.clone();
        heading(("space", id))
            .group(group.clone())
            .h(px(36.))
            .text_size(px(14.))
            .text_color(colors::text1())
            .child(
                div()
                    .flex_none()
                    .mr(px(8.))
                    .child(icon("folder", 15., colors::text2())),
            )
            .child(name)
            .children(path)
            .child(div().flex_1())
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
                    .child(chevron(open)),
            )
            .child(
                widgets::icon_button(("space-menu", id), "ellipsis", 22.)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                        r.menu = Some((e.position(), dots.clone()));
                        cx.notify();
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
            .pt(px(4.))
            .pb(px(8.))
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
