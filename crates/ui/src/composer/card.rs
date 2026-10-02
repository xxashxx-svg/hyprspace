// The composer's box and the resume list under it, after the Tauri app's composer.css: a framed
// card whose line firms up a little while you type, the folder on top, the prompt, and a row of
// chips with attach and the round send button at the bottom.

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton, Window, div,
    prelude::*, px,
};
use hyprspace_proto::state::Pick;
use hyprspace_theme::MONO;

use super::{Composer, Menu, PickFor, pickers};
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
    let top = top_row(c, cx);
    let model_chip: AnyElement = match &pick {
        Some(p) => widgets::chip("composer-model")
            .child(mark(p.agent, 13., colors::brand(p.agent).0))
            .child(div().truncate().child(model_label(c, p)))
            .child(widgets::caret())
            .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_menu(Menu::Model, e, cx)))
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
    let effort_chip = (!pickers::efforts(c).is_empty()).then(|| {
        let label = pick
            .as_ref()
            .map(|p| p.effort.clone())
            .filter(|e| !e.is_empty())
            .map_or("Effort".to_string(), |e| pickers::effort_label(&e));
        widgets::chip("composer-effort")
            .child(label)
            .child(widgets::caret())
            .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_menu(Menu::Effort, e, cx)))
    });
    let permission_chip = widgets::chip("composer-permission")
        .child(pickers::permission_label(c.prefs.permission))
        .child(widgets::caret())
        .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_menu(Menu::Permission, e, cx)));
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
        colors::ink(0.2)
    } else {
        colors::border2()
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(16.))
        .border_1()
        .border_color(frame)
        .bg(colors::surface2().opacity(0.85))
        .shadow(colors::shadow())
        .children(top)
        .when(!c.images.is_empty(), |d| {
            d.child(div().px(px(14.)).pb(px(8.)).child(attach::tray(
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
                .min_h(px(68.))
                .px(px(14.))
                .pt(px(4.))
                .pb(px(10.))
                .text_size(px(14.5))
                .line_height(px(22.))
                .child(c.input.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px(px(10.))
                .py(px(8.))
                .border_t_1()
                .border_color(colors::border1())
                .child(model_chip)
                .children(effort_chip)
                .child(permission_chip)
                .children(terminal_chip)
                .child(div().flex_1())
                .child(
                    widgets::icon_button("composer-attach", "paperclip", 28.)
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.pick_images(cx))),
                )
                .child(
                    widgets::send("composer-start", "arrow-up")
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.submit(cx))),
                ),
        )
        .into_any_element()
}

/// The folder row: where the thread will run. A project shows its folder; an open space asks.
fn top_row(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    let target = c.target.as_ref();
    let row = div().flex().items_center().gap_2().px(px(12.)).py(px(10.));
    match target {
        None => Some(
            row.child(
                widgets::chip("composer-folder")
                    .child(icon("folder-open", 13., colors::text2()))
                    .child("Choose a folder")
                    .on_click(
                        cx.listener(|c, _: &ClickEvent, _, cx| c.pick_folder(PickFor::Project, cx)),
                    ),
            )
            .into_any_element(),
        ),
        Some(t) if t.cwd.is_none() => {
            let label = c
                .folder
                .as_ref()
                .map_or("Choose a folder".to_string(), |f| f.display().to_string());
            Some(
                row.child(
                    widgets::chip("composer-folder")
                        .child(icon("folder", 13., colors::text2()))
                        .child(div().truncate().child(label))
                        .child(widgets::caret())
                        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| {
                            c.pick_folder(PickFor::Folder, cx)
                        })),
                )
                .into_any_element(),
            )
        }
        Some(t) => {
            let path = t.cwd.as_ref()?.display().to_string();
            Some(
                row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .min_w_0()
                        .text_color(colors::text3())
                        .child(icon("folder", 12., colors::text3()))
                        .child(
                            div()
                                .truncate()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .child(path),
                        ),
                )
                .into_any_element(),
            )
        }
    }
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
