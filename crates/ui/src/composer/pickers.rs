// The composer's model and permission pickers. The model picker follows the Tauri app's
// ModelPicker: the agents as tabs across the top with their marks, then that agent's models. An
// agent that is not installed keeps its tab and says why it can't be picked.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, SharedString, Window,
    div, prelude::*, px,
};
use hyprspace_proto::state::Pick;
use hyprspace_proto::{Agent, Permission};
use hyprspace_theme::MONO;

use super::{Composer, Menu};
use crate::assets::mark;
use crate::{colors, widgets};

pub fn permission_label(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Plan only",
        Permission::Ask => "Ask first",
        Permission::Auto => "Auto edit",
        Permission::Bypass => "Full access",
    }
}

fn permission_note(p: Permission) -> &'static str {
    match p {
        Permission::Plan => "Reads and plans. Changes nothing.",
        Permission::Ask => "Asks before edits and commands.",
        Permission::Auto => "Edits files on its own, asks before the rest.",
        Permission::Bypass => "Never asks. Only for folders you trust.",
    }
}

/// An effort level's name, from src/lib/models.ts.
pub fn effort_label(level: &str) -> String {
    match level {
        "" => "Default",
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Max",
        "ultra" => "Ultra",
        other => other,
    }
    .to_string()
}

/// One line on what a level does.
pub fn effort_note(level: &str) -> &'static str {
    match level {
        "" => "The CLI picks",
        "none" => "No extra thinking",
        "minimal" => "Fastest, barely thinks",
        "low" => "Quick answers",
        "medium" => "Balanced",
        "high" => "Thinks longer",
        "xhigh" => "Thinks much longer",
        "max" => "Everything it has",
        "ultra" => "Max, plus it delegates to sub-agents",
        _ => "Thinks harder",
    }
}

fn agent_desc(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Anthropic's coding agent",
        Agent::Codex => "OpenAI's Codex CLI",
    }
}

/// The effort levels the picked model takes.
pub fn efforts(c: &Composer) -> Vec<String> {
    let Some(pick) = c.pick() else {
        return Vec::new();
    };
    c.agents
        .iter()
        .find(|a| a.agent == pick.agent)
        .map(|a| a.catalog.efforts_for(&pick.model).to_vec())
        .unwrap_or_default()
}

pub fn menu(
    c: &Composer,
    which: Menu,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let body = match which {
        Menu::Permission => permission_menu(c, cx),
        _ => model_menu(c, cx),
    };
    let close = cx.listener(|c, _: &(), _, cx| {
        c.menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

fn model_menu(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    if c.agents.is_empty() {
        return div()
            .p(px(10.))
            .text_color(colors::text3())
            .child("Checking which agents are installed...")
            .into_any_element();
    }
    let tab = c
        .tab
        .or_else(|| c.agent().map(|a| a.agent))
        .unwrap_or(c.agents[0].agent);
    let tabs = c.agents.iter().map(|info| {
        let agent = info.agent;
        let on = agent == tab;
        let (brand, _) = colors::brand(agent);
        div()
            .id(("mp-tab", agent as usize))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .h(px(34.))
            .rounded_t(px(6.))
            .border_b_2()
            .border_color(if on { brand } else { gpui::transparent_black() })
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.05)))
            .child(div().opacity(if on { 1.0 } else { 0.7 }).child(mark(
                agent,
                15.,
                if info.status.installed {
                    brand
                } else {
                    colors::text3()
                },
            )))
            .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                c.tab = Some(agent);
                cx.notify();
            }))
    });
    let Some(info) = c.agents.iter().find(|a| a.agent == tab) else {
        return div().into_any_element();
    };
    let head = div()
        .flex()
        .items_baseline()
        .gap_2()
        .px(px(12.))
        .pt(px(10.))
        .pb(px(4.))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(tab.name()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(11.))
                .text_color(colors::text3())
                .child(agent_desc(tab)),
        )
        .children(info.status.version.clone().map(|v| {
            div()
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(colors::text3())
                .child(format!("v{v}"))
        }));
    let body: AnyElement = if !info.status.installed {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px(px(12.))
            .pt(px(10.))
            .pb(px(14.))
            .text_size(px(12.))
            .text_color(colors::text2())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::busy())
                    .child(format!("{} is not installed.", tab.name())),
            )
            .child(format!(
                "Install the {} CLI and sign in to it once, then reopen HyprSpace.",
                tab.name().to_lowercase()
            ))
            .into_any_element()
    } else {
        let current = c.pick();
        div()
            .flex()
            .flex_col()
            .p(px(4.))
            .children(info.catalog.models.iter().enumerate().map(|(i, m)| {
                let checked = current
                    .as_ref()
                    .is_some_and(|p| p.agent == tab && p.model == m.id);
                let id = m.id.clone();
                let catalog = info.catalog.clone();
                widgets::menu_item(
                    ("model", i),
                    m.label.clone(),
                    m.note.clone().map(SharedString::from),
                    checked,
                )
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                    let old = c.prefs.pick(tab);
                    // keep the effort when the new model takes it
                    let effort = if catalog.efforts_for(&id).contains(&old.effort) {
                        old.effort
                    } else {
                        String::new()
                    };
                    c.menu = None;
                    c.set_pick(
                        Pick {
                            agent: tab,
                            model: id.clone(),
                            effort,
                        },
                        cx,
                    );
                }))
            }))
            .into_any_element()
    };
    div()
        .w(px(330.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .gap(px(2.))
                .px(px(2.))
                .border_b_1()
                .border_color(colors::ink(0.08))
                .children(tabs),
        )
        .child(head)
        .child(body)
        .into_any_element()
}

fn permission_menu(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let modes = [
        Permission::Plan,
        Permission::Ask,
        Permission::Auto,
        Permission::Bypass,
    ];
    div()
        .w(px(280.))
        .flex()
        .flex_col()
        .child(widgets::menu_heading("Permission"))
        .children(modes.into_iter().enumerate().map(|(i, mode)| {
            widgets::menu_item(
                ("permission", i),
                permission_label(mode),
                Some(permission_note(mode).into()),
                c.prefs.permission == mode,
            )
            .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                c.prefs.permission = mode;
                c.menu = None;
                cx.emit(super::ComposerEvent::Prefs(c.prefs.clone()));
                cx.notify();
            }))
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_words_match_the_tauri_app() {
        assert_eq!(effort_label("xhigh"), "Extra high");
        assert_eq!(effort_label(""), "Default");
        assert_eq!(effort_label("turbo"), "turbo");
        assert_eq!(effort_note("max"), "Everything it has");
    }
}
