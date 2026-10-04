// The window's layout: sidebar, then the space on screen (its panes, see `crate::panes`), the
// composer, or settings. Context menus open from here so they float over everything.

use crate::assets::icon;
use gpui::{
    AnyElement, ClickEvent, Context, DragMoveEvent, IntoElement, Render, StyleRefinement, Window,
    div, prelude::*, px, relative,
};

use super::{Action, MenuEntry, MenuItems, Root, Screen, SidebarDrag};
use crate::sidebar::{MAX_WIDTH, MIN_WIDTH};
use crate::slide::slide;
use crate::{colors, widgets};

impl Root {
    /// A right-click menu, after the native menus T3 Code shows: plain words in roomy rows, groups
    /// split by a rule, a note on the right where one helps, and an arrow on the rows that open
    /// more choices beside them while the pointer is on them.
    fn menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (at, entries) = self.menu.clone()?;
        let sub = self.menu_sub.and_then(|i| match entries.get(i) {
            Some(MenuEntry::Sub { entries, .. }) => Some((i, entries.clone())),
            _ => None,
        });
        let mut panel = menu_panel(&entries, "menu", cx);
        if let Some((i, entries)) = sub {
            // the choices line up with the row that opened them, just past the menu's edge
            // its first row level with row `i`: past the rows above, less its own edge and padding
            let top = entries_height(&entries_before(&self.menu, i)) - MENU_PAD - 1.;
            panel = panel.child(
                div()
                    .id("submenu")
                    .absolute()
                    .left(relative(1.))
                    // it hangs outside the menu's own frame, so it keeps clicks from the layer behind
                    // that shuts the menu, as the frame does
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .top(px(top))
                    .pl(px(2.))
                    .child(menu_panel(&entries, "submenu", cx)),
            );
        }
        let close = cx.listener(|r, _: &(), _, cx| {
            r.menu = None;
            r.menu_sub = None;
            cx.notify();
        });
        Some(widgets::layer(
            at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            panel,
        ))
    }
}

const MENU_PAD: f32 = 4.;
const MENU_ROW: f32 = 30.;
const MENU_RULE: f32 = 9.;

/// The lines of the open menu above line `i`.
fn entries_before(
    menu: &Option<(gpui::Point<gpui::Pixels>, MenuItems)>,
    i: usize,
) -> Vec<MenuEntry> {
    menu.as_ref()
        .map(|(_, e)| e.iter().take(i).cloned().collect())
        .unwrap_or_default()
}

/// How tall `entries` stand in a menu, padding included.
fn entries_height(entries: &[MenuEntry]) -> f32 {
    MENU_PAD
        + entries
            .iter()
            .map(|e| match e {
                MenuEntry::Divider => MENU_RULE,
                _ => MENU_ROW,
            })
            .sum::<f32>()
}

/// One menu's panel: its rows on the window's raised surface.
fn menu_panel(entries: &[MenuEntry], key: &'static str, cx: &mut Context<Root>) -> gpui::Div {
    let top = key == "menu";
    let rows = entries.iter().enumerate().map(|(i, entry)| match entry {
        MenuEntry::Divider => div()
            .h(px(1.))
            .my(px((MENU_RULE - 1.) / 2.))
            .mx(px(-MENU_PAD))
            .bg(colors::ink(0.08))
            .into_any_element(),
        MenuEntry::Item {
            label,
            hint,
            action,
        } => {
            let action = *action;
            let danger = matches!(action, Action::RemoveThread(_));
            menu_line((key, i), label.clone(), danger)
                .children(hint.clone().map(|h| {
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(h)
                }))
                // a plain row in the top menu shuts any choices open beside another
                .when(top, |d| {
                    d.on_hover(cx.listener(move |r, on: &bool, _, cx| {
                        if *on && r.menu_sub.is_some() {
                            r.menu_sub = None;
                            cx.notify();
                        }
                    }))
                })
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.menu_sub = None;
                    r.act(action, window, cx)
                }))
                .into_any_element()
        }
        MenuEntry::Sub { label, .. } => menu_line((key, i), label.clone(), false)
            .child(icon("chevron-right", 12., colors::text3()))
            .on_hover(cx.listener(move |r, on: &bool, _, cx| {
                if *on && r.menu_sub != Some(i) {
                    r.menu_sub = Some(i);
                    cx.notify();
                }
            }))
            // a click opens them too, for anyone who clicks rather than waits
            .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                r.menu_sub = Some(i);
                cx.notify();
            }))
            .into_any_element(),
    });
    div()
        .relative()
        .min_w(px(200.))
        .max_w(px(340.))
        .p(px(MENU_PAD))
        .flex()
        .flex_col()
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(13.))
        .text_color(colors::text1())
        .children(rows)
}

/// A menu row: its words, roomy, and lit while the pointer is on it.
fn menu_line(
    id: impl Into<gpui::ElementId>,
    label: gpui::SharedString,
    danger: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(MENU_ROW))
        .px(px(12.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(move |s| {
            if danger {
                s.bg(colors::error().opacity(0.14))
                    .text_color(colors::error())
            } else {
                s.bg(colors::ink(0.08))
            }
        })
        .child(div().flex_1().min_w_0().truncate().child(label))
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main: AnyElement = if !self.loaded {
            div().into_any_element()
        } else {
            match self.screen {
                Screen::Thread(id) => self.workbench(id, window, cx),
                Screen::Compose(space) => self.compose_screen(space, window, cx),
                Screen::Settings => self.settings(window, cx),
            }
        };
        // before the title row, which slides its sidebar part in step with the column
        if self.loaded {
            self.sidebar_flips.see(!self.state.sidebar_hidden);
        }
        let titlebar = self.titlebar(window, cx);
        let menu = self.menu(window, cx);
        let card = self.hover_card(window, cx);
        let snooze = self.snooze_popup(window, cx);
        let toast = self.undo_toast(cx);
        let palette = self.palette_overlay(window, cx);
        let folders = self.folder_overlay(window, cx);
        let intro = self.intro_overlay(window, cx);
        let update = crate::update::overlay(&self.updater, cx);
        // Settings takes the whole window. A hidden sidebar comes back from the title row's
        // button; once it has slid away its column stays, zero wide
        let open = !self.state.sidebar_hidden;
        let flips = self.sidebar_flips.count();
        let width = self.state.sidebar_width.clamp(MIN_WIDTH, MAX_WIDTH);
        let sidebar = (self.screen != Screen::Settings && (open || flips > 0)).then(|| {
            // its own cached view, so a frame that only changes a terminal reuses its layout;
            // it keeps its width while the frame around it slides
            let view = self
                .sidebar_view
                .clone()
                .cached(StyleRefinement::default().w(px(width)).h_full().flex_none());
            slide("sidebar", flips, open, (0., width), false, view)
        });
        div()
            .id("root")
            .relative()
            .key_context("Root")
            .on_action(cx.listener(Self::toggle_palette))
            .on_action(cx.listener(Self::toggle_sidebar))
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::bg())
            .text_color(colors::text1())
            .font_family(crate::settings::ui_font())
            .on_drag_move(cx.listener(|r, e: &DragMoveEvent<SidebarDrag>, _, cx| {
                let x: f32 = e.event.position.x.into();
                r.state.sidebar_width = x.clamp(MIN_WIDTH, MAX_WIDTH);
                cx.notify();
            }))
            .on_drop(cx.listener(|r, _: &SidebarDrag, _, _| r.save()))
            .child(titlebar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .children(sidebar)
                    .child(div().flex_1().min_w_0().h_full().child(main)),
            )
            .children(update)
            .children(menu)
            .children(card)
            .children(snooze)
            .children(toast)
            .children(palette)
            .children(folders)
            .children(intro)
    }
}
