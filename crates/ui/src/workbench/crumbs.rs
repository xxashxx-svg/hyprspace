use std::time::Duration;

use gpui::{AnyElement, ClickEvent, Context, FontWeight, MouseButton, div, prelude::*};
use hyprspace_proto::Space;

use crate::root::{Action, Rename, Root};
use crate::sidebar::tag;
use crate::zoom::px;
use crate::{colors, widgets};

/// How long a click on the title waits for a second one before it opens the menu, so a double
/// click renames without the menu flashing open first.
const SECOND_CLICK: Duration = Duration::from_millis(250);

fn slash() -> AnyElement {
    div()
        .flex_none()
        .text_color(colors::text3().opacity(0.6))
        .child("/")
        .into_any_element()
}

impl Root {
    pub(super) fn crumbs(
        &self,
        s: &Space,
        thread: Option<u64>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let space = s.id;
        let project = div()
            .id("crumb-project")
            .occlude()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .max_w(px(200.))
            .px(px(4.))
            .py(px(2.))
            .rounded(px(6.))
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|d| d.text_color(colors::text1()))
            .child(tag(&s.name, self.look_of(s), false))
            .child(div().min_w_0().truncate().child(s.name.clone()))
            .tooltip(widgets::tip_text(format!("New thread in {}", s.name)))
            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                r.act(Action::NewThread(space), window, cx)
            }));
        let t = thread.and_then(|id| self.state.thread(id)).map(|(_, t)| t);
        let parent = t
            .and_then(|t| t.parent.filter(|_| !t.side))
            .and_then(|p| self.state.thread(p))
            .map(|(_, p)| {
                let id = p.id;
                div()
                    .id("crumb-parent")
                    .occlude()
                    .flex_shrink(1.)
                    .min_w(px(24.))
                    .max_w(px(220.))
                    .truncate()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .text_color(colors::text3())
                    .cursor_pointer()
                    .hover(|d| d.text_color(colors::text1()))
                    .child(p.title.clone())
                    .tooltip(widgets::tip_text(format!("Open {}", p.title)))
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.open_thread(id, window, cx)
                    }))
            });
        let Some(t) = t else {
            return row(project, parent, new_title());
        };
        let thread = t.id;
        let menu = self.thread_menu(t);
        let title = match self.rename_box(thread, true, px(13.), FontWeight::MEDIUM) {
            // the box reaches 7px past its text on each side, as on a sidebar row
            Some(rename) => div()
                .flex_1()
                .min_w(px(160.))
                .max_w(px(480.))
                .px(px(7.))
                .child(rename)
                .into_any_element(),
            None => {
                let right = self.context_menu(menu.clone(), cx);
                div()
                    .id("crumb-title")
                    .occlude()
                    .min_w_0()
                    .flex_shrink(1.)
                    .truncate()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .cursor_pointer()
                    .hover(|d| d.bg(colors::ink(0.06)))
                    .child(t.title.clone())
                    .on_mouse_down(MouseButton::Right, right)
                    .on_click(cx.listener(move |r, e: &ClickEvent, window, cx| {
                        if e.click_count() >= 2 {
                            r.title_click = None;
                            r.menu = None;
                            r.start_rename(Rename::Thread(thread), window, cx);
                            r.rename_here = true;
                            return;
                        }
                        let (at, items) = (e.position(), menu.clone());
                        r.title_click = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(SECOND_CLICK).await;
                            let _ = this.update(cx, |r, cx| {
                                r.title_click = None;
                                r.menu = Some((at, items));
                                r.menu_sub = None;
                                r.delete_armed = false;
                                cx.notify();
                            });
                        }));
                    }))
                    .into_any_element()
            }
        };
        row(project, parent, title)
    }
}

fn new_title() -> AnyElement {
    div()
        .min_w_0()
        .truncate()
        .px(px(4.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child("New thread")
        .into_any_element()
}

fn row(
    project: impl IntoElement,
    parent: Option<impl IntoElement>,
    title: AnyElement,
) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .text_size(px(13.))
        .child(project)
        .child(slash())
        .children(parent.map(|p| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .min_w_0()
                .child(p)
                .child(slash())
        }))
        .child(title)
        .into_any_element()
}
