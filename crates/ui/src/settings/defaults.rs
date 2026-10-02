// Settings, Defaults: what new sessions start with. The permission is one setting for every
// agent, so it sits once at the top. Then a card per installed agent with its model and effort,
// which are the composer's saved picks. Agents that are not installed share one line at the foot.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, SharedString, Window, div, prelude::*, px,
};
use hyprspace_proto::agents::{AgentCatalog, AgentInfo};
use hyprspace_proto::state::Pick;
use hyprspace_proto::{Command, Permission};
use hyprspace_theme::MONO;

use super::{Picker, section};
use crate::assets::mark;
use crate::composer::effort_label;
use crate::root::Root;
use crate::{colors, widgets};

/// Each mode, most careful first: its name, icon, and what picking it means.
const MODES: [(Permission, &str, &str, &str); 4] = [
    (
        Permission::Plan,
        "Plan only",
        "list-checks",
        "Reads and plans. It changes nothing.",
    ),
    (
        Permission::Ask,
        "Ask first",
        "hand",
        "Asks before every edit and command.",
    ),
    (
        Permission::Auto,
        "Auto edit",
        "file-pen-line",
        "Edits files in the folder on its own. Asks before anything else.",
    ),
    (
        Permission::Bypass,
        "Full access",
        "shield-off",
        "Never asks. Use it only in folders you trust.",
    ),
];

fn model_label(catalog: &AgentCatalog, model: &str) -> String {
    crate::models::name(Some(catalog), model)
}

impl Root {
    pub(super) fn defaults(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.state.composer.permission;
        let modes = MODES
            .iter()
            .enumerate()
            .map(|(i, &(mode, name, glyph, _))| {
                widgets::segment(
                    ("permission", i),
                    Some(glyph),
                    name,
                    current == mode,
                    mode == Permission::Bypass,
                )
                .flex_1()
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    r.update_prefs(|p| p.permission = mode, cx)
                }))
            });
        let says = MODES.iter().find(|m| m.0 == current).map_or("", |m| m.3);
        let permission = section(
            "Permission for new sessions",
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(widgets::segments().children(modes))
                .child(
                    div()
                        .px(px(2.))
                        .text_size(px(12.))
                        .text_color(if current == Permission::Bypass {
                            colors::error()
                        } else {
                            colors::text3()
                        })
                        .child(says),
                ),
        );

        let (installed, missing): (Vec<&AgentInfo>, Vec<&AgentInfo>) =
            self.agents.iter().partition(|a| a.status.installed);
        let agents = section(
            "Agents",
            div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .when(self.agents.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child("Checking which agents are installed."),
                    )
                })
                .children(installed.into_iter().map(|a| self.agent_card(a, cx)))
                .when(!missing.is_empty(), |d| {
                    d.child(not_installed(&missing, cx))
                }),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(26.))
            .child(permission)
            .child(agents)
            .child(
                div()
                    .px(px(2.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child("The composer can still change the model and effort for one thread."),
            )
            .into_any_element()
    }

    fn agent_card(&self, info: &AgentInfo, cx: &mut Context<Self>) -> AnyElement {
        let agent = info.agent;
        let pick = self.state.composer.pick(agent);
        let (brand, _) = colors::brand(agent);
        let efforts = info.catalog.efforts_for(&pick.model);
        let status = &info.status;
        let (dot, line) = match &status.account {
            Some(_) => (colors::ok(), "Signed in".to_string()),
            None => (
                colors::busy(),
                status.detail.clone().unwrap_or_else(|| "Installed".into()),
            ),
        };
        let head = div()
            .flex()
            .items_center()
            .gap(px(11.))
            .px(px(16.))
            .py(px(13.))
            .border_b_1()
            .border_color(colors::border1())
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.))
                    .rounded(px(9.))
                    .bg(brand.opacity(0.16))
                    .child(mark(agent, 16., brand)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(agent.name()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .text_color(colors::text3())
                            .child(div().flex_none().size(px(6.)).rounded_full().bg(dot))
                            .child(div().truncate().child(line)),
                    ),
            )
            .children(status.plan.clone().map(|p| {
                div()
                    .flex_none()
                    .px(px(9.))
                    .py(px(2.))
                    .rounded_full()
                    .bg(brand.opacity(0.13))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(p)
            }))
            .children(status.version.clone().map(|v| {
                div()
                    .flex_none()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .child(format!("v{v}"))
            }));
        let effort = if pick.effort.is_empty() {
            "Default".to_string()
        } else {
            effort_label(&pick.effort)
        };
        let fields = div()
            .flex()
            .gap(px(14.))
            .p(px(16.))
            .child(self.field(
                "Model",
                Picker::Model(agent),
                widgets::select(
                    ("set-model", agent as usize),
                    model_label(&info.catalog, &pick.model),
                ),
                cx,
            ))
            .child(if efforts.is_empty() {
                div().flex_1().into_any_element()
            } else {
                self.field(
                    "Effort",
                    Picker::Effort(agent),
                    widgets::select(("set-effort", agent as usize), effort),
                    cx,
                )
            });
        div()
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .overflow_hidden()
            .child(head)
            .child(fields)
            .into_any_element()
    }

    /// A named select in an agent card that opens `picker`. Two share a row. Its bounds are
    /// kept so the menu can open right under it.
    fn field(
        &self,
        name: &str,
        picker: Picker,
        select: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fields = self.settings.fields.clone();
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text2())
                    .child(name.to_string()),
            )
            .child(
                div()
                    .on_children_prepainted(move |b, _, _| {
                        if let Some(b) = b.first() {
                            fields.borrow_mut().insert(picker, *b);
                        }
                    })
                    .child(
                        select.on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                            r.settings.menu = Some((e.position(), picker));
                            cx.notify();
                        })),
                    ),
            )
            .into_any_element()
    }

    /// The open model or effort picker, dropped under its field.
    pub(super) fn picker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (click, picker) = self.settings.menu?;
        let field = self.settings.fields.borrow().get(&picker).copied();
        let agent = match picker {
            Picker::Model(a) | Picker::Effort(a) => a,
        };
        let info = self.agents.iter().find(|a| a.agent == agent)?;
        let pick = self.state.composer.pick(agent);
        let body = match picker {
            Picker::Model(_) => div()
                .flex()
                .flex_col()
                .child(widgets::menu_heading(format!("{} model", agent.name())))
                .children(info.catalog.models.iter().enumerate().map(|(i, m)| {
                    let id = m.id.clone();
                    let catalog = info.catalog.clone();
                    widgets::menu_item(
                        ("model", i),
                        m.label.clone(),
                        m.note.clone().map(SharedString::from),
                        m.id == pick.model,
                    )
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        let old = r.state.composer.pick(agent);
                        // keep the effort when the new model takes it, like the composer does
                        let effort = if catalog.efforts_for(&id).contains(&old.effort) {
                            old.effort
                        } else {
                            String::new()
                        };
                        r.settings.menu = None;
                        let pick = Pick {
                            agent,
                            model: id.clone(),
                            effort,
                        };
                        r.update_prefs(|p| p.set_pick(pick), cx);
                    }))
                })),
            Picker::Effort(_) => {
                let levels = std::iter::once(String::new())
                    .chain(info.catalog.efforts_for(&pick.model).iter().cloned());
                div()
                    .flex()
                    .flex_col()
                    .child(widgets::menu_heading("Effort"))
                    .children(levels.enumerate().map(|(i, level)| {
                        let name = if level.is_empty() {
                            "Default".to_string()
                        } else {
                            effort_label(&level)
                        };
                        let checked = level == pick.effort;
                        widgets::menu_item(("effort", i), name, None, checked).on_click(
                            cx.listener(move |r, _: &ClickEvent, _, cx| {
                                r.settings.menu = None;
                                let mut pick = r.state.composer.pick(agent);
                                pick.effort = level.clone();
                                r.update_prefs(|p| p.set_pick(pick), cx);
                            }),
                        )
                    }))
            }
        };
        let close = cx.listener(|r, _: &(), _, cx| {
            r.settings.menu = None;
            cx.notify();
        });
        let close = move |w: &mut Window, cx: &mut gpui::App| close(&(), w, cx);
        Some(match field {
            Some(b) => widgets::dropdown(b, window, close, body),
            None => widgets::popup(click, widgets::Open::Down, window, close, body),
        })
    }
}

/// The agents whose CLI is missing, on one line, with a way to look again after installing.
fn not_installed(missing: &[&AgentInfo], cx: &mut Context<Root>) -> AnyElement {
    let chips = missing.iter().map(|a| {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(24.))
            .px(px(9.))
            .rounded_full()
            .bg(colors::ink(0.05))
            .text_size(px(12.))
            .text_color(colors::text2())
            .child(mark(a.agent, 13., colors::text3()))
            .child(a.agent.name())
    });
    div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap(px(10.))
        .px(px(16.))
        .py(px(12.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border1())
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text2())
                .child("Not installed"),
        )
        .children(chips)
        .child(
            div()
                .flex_1()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child("Install one, then check again."),
        )
        .child(
            widgets::button("agents-recheck", "Check again").on_click(
                cx.listener(|r, _: &ClickEvent, _, _| r.client.send(Command::LoadAgents)),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::Agent;
    use hyprspace_proto::agents::ModelInfo;

    #[test]
    fn models_show_by_name_never_by_id() {
        let catalog = AgentCatalog {
            agent: Agent::Claude,
            models: vec![ModelInfo {
                id: "claude-opus-5-5".into(),
                label: "Opus 5.5".into(),
                note: None,
                efforts: Vec::new(),
                default_effort: None,
            }],
            efforts: Vec::new(),
        };
        assert_eq!(model_label(&catalog, "claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_label(&catalog, ""), "Default");
        assert_eq!(model_label(&catalog, "claude-gone-1"), "Gone 1");
    }
}
