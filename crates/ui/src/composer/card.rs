// The composer's box, the question over it, the line under it, and the resume list. The box is
// the reply box's twin (bar.rs), with Structured beside the round send button. Over it, the
// question names the space, which is the button to change it. Under it: the computer and
// branch the thread starts on.

use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, Focusable, FontWeight, IntoElement, MouseButton, Window, div,
    prelude::*,
};
use hyprspace_proto::state::Pick;
use hyprspace_theme::MONO;

use super::model_menu::Host as _;
use super::{Composer, ComposerEvent, PickFor, bar, model_menu};
use crate::assets::{icon, mark};
use crate::zoom::px;
use crate::{attach, colors, time, widgets};

/// The picked model's label, the default model's when none was picked.
pub fn model_label(c: &Composer, pick: &Pick) -> String {
    let catalog = c
        .all_agents()
        .iter()
        .find(|a| a.agent == pick.agent)
        .map(|a| &a.catalog);
    crate::models::name(catalog, &pick.model)
}

pub fn card(c: &Composer, window: &mut Window, cx: &mut Context<Composer>) -> AnyElement {
    let pick = c.pick();
    let focused = c.input.focus_handle(cx).is_focused(window);
    let empty = c.input.read(cx).text().trim().is_empty() && c.images.is_empty();
    let model_chip: AnyElement = match &pick {
        Some(p) => model_menu::anchored_chip(
            &c.anchor,
            bar::model("composer-model", p.agent, model_label(c, p))
                .when(c.also_count() > 0, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .px(px(4.))
                            .rounded(px(4.))
                            .bg(colors::ink(0.08))
                            .text_size(px(10.5))
                            .text_color(colors::text2())
                            .child(format!("+{}", c.also_count())),
                    )
                })
                .on_click(cx.listener(|c, _: &ClickEvent, window, cx| c.open_models(window, cx))),
        )
        .into_any_element(),
        None if c.all_agents().is_empty() => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .child(crate::spinner::ring("checking-agents", colors::text3()))
            .child("Checking agents")
            .into_any_element(),
        None => div()
            .text_size(px(12.))
            .text_color(colors::error())
            .child("No agent installed")
            .into_any_element(),
    };
    let effort_chip = c
        .model_spec()
        .filter(|s| !s.efforts.is_empty() || s.long.is_some() || s.fast.is_some())
        .map(|spec| {
            model_menu::anchored_chip(
                &c.effort_anchor,
                bar::effort("composer-effort", &spec).on_click(
                    cx.listener(|c, _: &ClickEvent, window, cx| c.open_effort(window, cx)),
                ),
            )
        });
    let permission_chip = bar::permission("composer-permission", c.prefs.permission)
        .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_permission(e, cx)));
    let on = !c.terminal();
    let terminal_chip = (pick.is_some() && c.prefs.structured).then(|| {
        bar::chip("composer-structured")
            .gap(px(8.))
            .child(widgets::switch_turned(
                "composer-structured-switch",
                on,
                crate::slide::flipped("composer-structured", std::time::Duration::from_millis(260)),
            ))
            .child("Structured")
            .when(on, |d| d.text_color(colors::text1()))
            .tooltip(widgets::tip(
                "Run this thread as a transcript instead of a terminal",
            ))
            .on_click(cx.listener(|c, _: &ClickEvent, _, cx| {
                crate::slide::flip("composer-structured".into());
                c.toggle_structured(cx)
            }))
            .into_any_element()
    });
    // the send button firms up once there is something to send; empty, it starts the agent
    // with no task, as a terminal would
    let send = if c.planning() {
        div()
            .id("composer-start")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(32.))
            .px(px(14.))
            .rounded_full()
            .bg(colors::accent())
            .text_size(px(12.5))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors::on_accent())
            .cursor_pointer()
            .hover(|s| s.bg(colors::accent_hover()))
            .child(icon("clock", 13., colors::on_accent()))
            .child("Schedule")
    } else if empty {
        div()
            .id("composer-start")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(32.))
            .rounded_full()
            .bg(colors::ink(0.1))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.18)))
            .child(icon("arrow-up", 15., colors::text2()))
            .tooltip(widgets::tip("Start without a task"))
    } else {
        widgets::send("composer-start", "arrow-up", true).tooltip(widgets::tip("Start"))
    };
    let send = send
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.submit(false, cx)))
        .into_any_element();
    let schedule = c.can_schedule().then(|| {
        widgets::icon_button("composer-schedule", "clock", widgets::LARGE)
            .when(c.planning(), |d| d.bg(colors::ink(0.08)))
            .tooltip(widgets::tip(if c.planning() {
                "Start it now instead"
            } else {
                "Run on a schedule instead"
            }))
            .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.toggle_plan(cx)))
            .into_any_element()
    });
    widgets::composer_box(focused)
        .when(crate::wallpaper::showing(), |d| {
            d.bg(colors::surface2().opacity(0.94))
        })
        .when(attach::held(), |d| {
            d.border_color(colors::accent())
                .bg(colors::accent().opacity(0.06))
        })
        .children(attach::peek(&c.images, c.peek))
        .when(!c.images.is_empty(), |d| {
            d.child(div().px(px(14.)).pt(px(12.)).child(attach::tray(
                "composer-img",
                &c.images,
                cx.listener(|_, path: &PathBuf, _, cx| {
                    cx.emit(ComposerEvent::ShowMedia(path.clone()))
                }),
                cx.listener(|c, ix: &usize, _, cx| {
                    if *ix < c.images.len() {
                        c.images.remove(*ix);
                        attach::unmark(&c.input, *ix, cx);
                    }
                    cx.notify();
                }),
            )))
        })
        .children(super::schedule::when_line(c, cx))
        .child(bar::field(&c.input).min_h(px(76.)))
        .child(bar::bar(
            bar::attach("composer-attach")
                .on_click(cx.listener(|c, _: &ClickEvent, _, cx| c.pick_images(cx))),
            model_chip,
            effort_chip,
            permission_chip,
            terminal_chip.into_iter().chain(schedule).chain([send]),
        ))
        .into_any_element()
}

/// The line under the box: the computer and branch the thread starts on.
pub fn footer(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .mt(px(8.))
        .px(px(6.))
        .children(super::machine::chip(c, cx))
        .children(super::worktree::branch_chip(c, cx))
        .children(super::worktree::chip(c, cx))
        .into_any_element()
}

/// The question over the box. The space in it is the button that changes it: a project opens
/// the list of projects, an open space asks for a folder.
pub fn ask(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let scratch = c.target.as_ref().is_some_and(|t| {
        t.machine.is_none()
            && t.cwd
                .as_deref()
                .is_some_and(|cwd| crate::root::same_folder(cwd, &crate::root::scratch_root()))
    });
    let (named, label, pick) = match c.target.as_ref() {
        None => (false, "Pick a folder".to_string(), None),
        Some(t) if t.cwd.is_none() => match c.folder.as_ref().and_then(|f| f.file_name()) {
            Some(n) => (true, n.to_string_lossy().to_string(), Some(PickFor::Folder)),
            None => (false, "Pick a folder".to_string(), Some(PickFor::Folder)),
        },
        Some(t) => (!scratch, t.name.clone(), None),
    };
    let button = div()
        .id("composer-folder")
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.06)))
        .when(!named, |d| {
            d.mt(px(6.))
                .py(px(2.))
                .text_size(px(13.))
                .font_weight(FontWeight::NORMAL)
                .text_color(colors::text2())
        })
        .child(div().min_w_0().truncate().child(label))
        .when(!named, |d| {
            d.child(icon("chevron-down", 13., colors::text3()))
        })
        .on_click(cx.listener(move |c, e: &ClickEvent, _, cx| match pick {
            Some(pick) => c.pick_folder(pick, cx),
            None => c.open_places(e, cx),
        }));
    let lead = |text: &'static str| div().flex_none().child(text);
    if !named {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .child(lead("What should we build?"))
            .child(button)
            .into_any_element();
    }
    div()
        .flex()
        .w_full()
        .min_w_0()
        .items_center()
        .justify_center()
        .child(lead("What should we build in"))
        .child(button)
        .child(lead("?").ml(px(-5.)))
        .into_any_element()
}

const RESUME_SHOWN: usize = 5;

/// The agent's saved conversations for this folder, to pick one up again.
pub fn resume_list(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.resumable.is_empty() {
        return None;
    }
    let agent = c.agent()?.agent;
    let now = time::now_ms();
    let total = c.resumable.len();
    let shown = if c.resume_all {
        total
    } else {
        total.min(RESUME_SHOWN)
    };
    let rows = c.resumable.iter().take(shown).enumerate().map(|(i, s)| {
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
            .rounded(px(6.))
            .text_size(px(12.5))
            .text_color(colors::text2())
            .cursor_pointer()
            .hover(|d| d.bg(colors::ink(0.05)).text_color(colors::text1()))
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
                    .rounded(px(6.))
                    .bg(colors::ink(0.1))
                    .text_size(px(11.))
                    .text_color(colors::text1())
                    .invisible()
                    .group_hover(group, |d| d.visible())
                    .child("Resume"),
            )
            .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| c.resume(session.clone(), cx)))
    });
    let more = (total > RESUME_SHOWN).then(|| {
        let open = c.resume_all;
        div()
            .id("resume-more")
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(32.))
            .pl(px(10.))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .cursor_pointer()
            .hover(|d| d.bg(colors::ink(0.05)).text_color(colors::text1()))
            .child(icon(
                if open { "chevron-up" } else { "chevron-down" },
                12.,
                colors::text3(),
            ))
            .child(if open {
                "Show fewer".to_string()
            } else {
                format!("Show {} more", total - RESUME_SHOWN)
            })
            .on_click(cx.listener(|c, _: &ClickEvent, _, cx| {
                c.resume_all = !c.resume_all;
                cx.notify();
            }))
    });
    Some(
        div()
            .mt(px(28.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .bg(colors::ink(0.035))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(34.))
                    .px(px(12.))
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(4.))
                    .children(rows)
                    .children(more),
            )
            .into_any_element(),
    )
}
