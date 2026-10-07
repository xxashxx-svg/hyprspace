// The composer's box, the line under it, and the resume list. The box is framed like the
// thread's reply pill and its line firms up a little while you type: the prompt, then attach,
// permission and terminal on the left of its bottom row and the model, effort and the round
// send button on the right, the way zeron lays it out. Its pickers sit flat in the box until
// hovered. Under it, after T3 Code's: the folder and branch the thread starts on, and the keys.

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton, Window, div,
    prelude::*, px,
};
use hyprspace_proto::Permission;
use hyprspace_proto::state::Pick;
use hyprspace_theme::MONO;

use super::model_menu::Host as _;
use super::{Composer, PickFor, model_menu, pickers};
use crate::assets::{icon, mark};
use crate::{attach, colors, time, widgets};

/// The picked model's label, or "Default".
pub fn model_label(c: &Composer, pick: &Pick) -> String {
    let catalog = c
        .agents
        .iter()
        .find(|a| a.agent == pick.agent)
        .map(|a| &a.catalog);
    crate::models::name(catalog, &pick.model)
}

/// A picker in the box: flat until hovered, so the prompt leads.
fn flat(id: &'static str) -> gpui::Stateful<gpui::Div> {
    widgets::chip(id)
        .border_color(colors::ink(0.))
        .bg(colors::ink(0.))
}

/// The permission's icon: how far the agent goes on its own.
fn permission_icon(p: Permission) -> (&'static str, gpui::Hsla) {
    match p {
        Permission::Plan => ("list-checks", colors::text3()),
        Permission::Ask => ("hand", colors::text3()),
        Permission::Auto => ("zap", colors::text3()),
        // never asks: marked, so it isn't picked by accident
        Permission::Bypass => ("shield-off", colors::busy()),
    }
}

pub fn card(c: &Composer, window: &mut Window, cx: &mut Context<Composer>) -> AnyElement {
    let pick = c.pick();
    let focused = c.input.focus_handle(cx).is_focused(window);
    let empty = c.input.read(cx).text().trim().is_empty() && c.images.is_empty();
    let model_chip: AnyElement = match &pick {
        Some(p) => model_menu::anchored_chip(
            &c.anchor,
            flat("composer-model")
                .child(mark(p.agent, 13., colors::brand(p.agent).0))
                .child(div().truncate().child(model_label(c, p)))
                .child(widgets::caret())
                .on_click(cx.listener(|c, _: &ClickEvent, window, cx| c.open_models(window, cx))),
        )
        .into_any_element(),
        None if c.agents.is_empty() => div()
            .text_size(px(12.))
            .text_color(colors::text3())
            .child("Checking agents...")
            .into_any_element(),
        None => div()
            .text_size(px(12.))
            .text_color(colors::error())
            .child("No agent installed")
            .into_any_element(),
    };
    let effort_chip = c
        .model_spec()
        .filter(|s| !s.efforts.is_empty() || s.long.is_some())
        .map(|spec| {
            model_menu::anchored_chip(
                &c.effort_anchor,
                flat("composer-effort")
                    .child(model_menu::effort_chip_label(&spec))
                    .child(widgets::caret())
                    .on_click(
                        cx.listener(|c, _: &ClickEvent, window, cx| c.open_effort(window, cx)),
                    ),
            )
        });
    let (glyph, tint) = permission_icon(c.prefs.permission);
    let permission_chip = flat("composer-permission")
        .child(icon(glyph, 13., tint))
        .child(pickers::permission_label(c.prefs.permission))
        .child(widgets::caret())
        .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_permission(e, cx)));
    // on: the agent runs interactively in a terminal session, on the plan's terminal limits
    let on = c.terminal();
    let terminal_chip = (pick.is_some() && c.prefs.structured).then(|| {
        flat("composer-terminal")
            .child(icon(
                "terminal",
                13.,
                if on { colors::text1() } else { colors::text3() },
            ))
            .child("Terminal")
            .when(on, |d| {
                d.bg(colors::surface3())
                    .border_color(colors::border2())
                    .text_color(colors::text1())
            })
            .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.toggle_terminal(cx)))
    });
    let frame = if focused {
        colors::ink(0.16)
    } else {
        colors::border1()
    };
    // the send button firms up once there is something to send; empty, it starts the agent
    // with no task, as a terminal would
    let send = if empty {
        div()
            .id("composer-start")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded_full()
            .bg(colors::ink(0.1))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.18)))
            .child(icon("arrow-up", 15., colors::text2()))
            .tooltip(widgets::tip("Start without a task"))
    } else {
        widgets::send("composer-start", "arrow-up").tooltip(widgets::tip("Start"))
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(18.))
        .border_1()
        .border_color(frame)
        .bg(colors::surface2().opacity(0.85))
        .shadow(colors::shadow())
        // files held over the screen land here
        .drag_over::<gpui::ExternalPaths>(|s, _, _, _| {
            s.border_color(colors::accent())
                .bg(colors::accent().opacity(0.06))
        })
        .when(!c.images.is_empty(), |d| {
            d.child(div().px(px(14.)).pt(px(12.)).child(attach::tray(
                "composer-img",
                &c.images,
                cx.listener(|c, ix: &usize, _, cx| {
                    if *ix < c.images.len() {
                        c.images.remove(*ix);
                    }
                    cx.notify();
                }),
            )))
        })
        .child(
            div()
                .min_h(px(76.))
                .px(px(18.))
                .pt(px(16.))
                .pb(px(8.))
                .text_size(px(14.5))
                .line_height(px(23.))
                .child(c.input.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .px(px(8.))
                .pb(px(8.))
                .child(
                    widgets::icon_button("composer-attach", "paperclip", 28.)
                        .tooltip(widgets::tip("Attach images"))
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.pick_images(cx))),
                )
                .child(permission_chip)
                .children(terminal_chip)
                .child(div().flex_1())
                .child(model_chip)
                .children(effort_chip)
                .child(
                    send.ml(px(4.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.submit(cx))),
                ),
        )
        .into_any_element()
}

/// The line under the box: where the thread starts (the folder, a button when it can change,
/// and its branch) on the left, and the keys on the right.
pub fn footer(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .mt(px(8.))
        .px(px(6.))
        .child(folder_picker(c, cx))
        .children(c.branch.clone().map(|b| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .min_w_0()
                .px(px(6.))
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(icon("git-branch", 12., colors::text3()))
                .child(div().max_w(px(200.)).truncate().font_family(MONO).child(b))
        }))
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .text_size(px(11.5))
                .text_color(colors::text3())
                .child("Enter to send  \u{b7}  Shift+Enter for a new line"),
        )
        .into_any_element()
}

/// Where the thread will run, as a quiet button under the box, like zeron's folder picker. A
/// project shows its folder; an open space asks for one.
pub fn folder_picker(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let (label, pick) = match c.target.as_ref() {
        None => ("Choose a folder".to_string(), Some(PickFor::Project)),
        Some(t) if t.cwd.is_none() => (
            c.folder
                .as_ref()
                .and_then(|f| f.file_name())
                .map_or("Choose a folder".to_string(), |n| {
                    n.to_string_lossy().to_string()
                }),
            Some(PickFor::Folder),
        ),
        Some(t) => (t.name.clone(), None),
    };
    div()
        .id("composer-folder")
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(24.))
        .px(px(6.))
        .rounded(px(6.))
        .text_size(px(12.))
        .text_color(colors::text2())
        .child(icon("folder", 13., colors::text3()))
        .child(div().max_w(px(260.)).truncate().child(label))
        .when_some(pick, |d, pick| {
            d.cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
                .child(icon("chevron-down", 12., colors::text3()))
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| c.pick_folder(pick, cx)))
        })
        .into_any_element()
}

/// The agent's saved conversations for this folder, to pick one up again.
pub fn resume_list(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.resumable.is_empty() {
        return None;
    }
    let agent = c.agent()?.agent;
    let now = time::now_ms();
    let rows = c.resumable.iter().enumerate().map(|(i, s)| {
        let session = s.clone();
        let group = format!("resume-{i}");
        div()
            .id(("resume", i))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(34.))
            .pl(px(10.))
            .pr(px(8.))
            .rounded(px(7.))
            .text_size(px(12.5))
            .text_color(colors::text2())
            .cursor_pointer()
            .hover(|d| {
                d.bg(colors::surface3().opacity(0.65))
                    .text_color(colors::text1())
            })
            .child(div().flex_1().min_w_0().truncate().child(s.title.clone()))
            .child(
                div()
                    .flex_none()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .group_hover(group.clone(), |d| d.invisible())
                    .child(time::ago(s.modified, now)),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(7.))
                    .py(px(2.))
                    .rounded(px(5.))
                    .bg(colors::surface3())
                    .text_size(px(11.))
                    .text_color(colors::text1())
                    .invisible()
                    .group_hover(group, |d| d.visible())
                    .child("Resume"),
            )
            .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| c.resume(session.clone(), cx)))
    });
    Some(
        div()
            .mt(px(28.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2().opacity(0.55))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(34.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(colors::border1())
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text2())
                    .child(mark(agent, 13., colors::brand(agent).0))
                    .child(format!("Continue a {} session", agent.name()))
                    .child(div().flex_1())
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(colors::text3())
                            .child(c.resumable.len().to_string()),
                    ),
            )
            .child(div().flex().flex_col().p(px(4.)).children(rows))
            .into_any_element(),
    )
}
