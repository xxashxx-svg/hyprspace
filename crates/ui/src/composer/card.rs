// The composer's box, the folder picker over it, and the resume list under it. The box is framed
// like the thread's reply pill and its line firms up a little while you type: the prompt, then
// attach, permission and terminal on the left of its bottom row and the model, effort and the
// round send button on the right, the way zeron lays it out.

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton, Window, div,
    prelude::*, px,
};
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

pub fn card(c: &Composer, window: &mut Window, cx: &mut Context<Composer>) -> AnyElement {
    let pick = c.pick();
    let focused = c.input.focus_handle(cx).is_focused(window);
    let model_chip: AnyElement = match &pick {
        Some(p) => model_menu::anchored_chip(
            &c.anchor,
            widgets::chip("composer-model")
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
                widgets::chip("composer-effort")
                    .child(model_menu::effort_chip_label(&spec))
                    .child(widgets::caret())
                    .on_click(
                        cx.listener(|c, _: &ClickEvent, window, cx| c.open_effort(window, cx)),
                    ),
            )
        });
    let permission_chip = widgets::chip("composer-permission")
        .child(pickers::permission_label(c.prefs.permission))
        .child(widgets::caret())
        .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_permission(e, cx)));
    // on: the agent runs interactively in a terminal session, on the plan's terminal limits
    let forced = c.agent().is_some_and(|a| !a.agent.structured());
    let on = c.terminal();
    let terminal_chip = pick.is_some().then(|| {
        widgets::chip("composer-terminal")
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
            .when(!forced, |d| {
                d.on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.toggle_terminal(cx)))
            })
            .when(forced, |d| d.cursor_default())
    });
    let frame = if focused {
        colors::ink(0.16)
    } else {
        colors::border1()
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(14.))
        .border_1()
        .border_color(frame)
        .bg(colors::surface2().opacity(0.85))
        .shadow(colors::shadow())
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
                .min_h(px(64.))
                .px(px(16.))
                .pt(px(14.))
                .pb(px(8.))
                .text_size(px(14.))
                .line_height(px(22.))
                .child(c.input.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px(px(8.))
                .pb(px(8.))
                .child(
                    widgets::icon_button("composer-attach", "paperclip", 28.)
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.pick_images(cx))),
                )
                .child(permission_chip)
                .children(terminal_chip)
                .child(div().flex_1())
                .child(model_chip)
                .children(effort_chip)
                .child(
                    widgets::send("composer-start", "arrow-up")
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.submit(cx))),
                ),
        )
        .into_any_element()
}

/// Where the thread will run, as a quiet button above the box on its right, like zeron's folder
/// picker. A project shows its folder; an open space asks for one.
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
        .h(px(26.))
        .px(px(8.))
        .rounded(px(7.))
        .text_size(px(12.5))
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
            .mt(px(16.))
            .flex()
            .flex_col()
            .rounded(px(10.))
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
