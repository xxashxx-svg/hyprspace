// Settings, Agents: one row per installed agent with the model and effort its new threads start
// with, which are the composer's saved picks. Agents that are not installed share one row at the
// foot, with a way to look again after installing one.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, div, prelude::*, px};
use hyprspace_proto::Command;
use hyprspace_proto::agents::{AgentCatalog, AgentInfo};

use super::controls::{block, row_with, section, text};
use super::{Picker, Root};
use crate::assets::mark;
use crate::composer::effort_label;
use crate::{colors, widgets};

pub(super) fn model_label(catalog: &AgentCatalog, model: &str) -> String {
    crate::models::name(Some(catalog), model)
}

pub(super) fn effort_name(effort: &str) -> String {
    if effort.is_empty() {
        "Default effort".into()
    } else {
        effort_label(effort)
    }
}

impl Root {
    pub(super) fn agents_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let (installed, missing): (Vec<&AgentInfo>, Vec<&AgentInfo>) =
            self.agents.iter().partition(|a| a.status.installed);
        let mut rows: Vec<AnyElement> = installed.iter().map(|a| self.agent_row(a, cx)).collect();
        if self.agents.is_empty() {
            rows.push(
                div()
                    .py(px(18.))
                    .child(text("Checking which agents are installed."))
                    .into_any_element(),
            );
        }
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(section("Installed", block(rows)));
        if !missing.is_empty() {
            page = page.child(section(
                "Not installed",
                block(vec![not_installed(&missing, cx)]),
            ));
        }
        page.into_any_element()
    }

    fn agent_row(&self, info: &AgentInfo, cx: &mut Context<Self>) -> AnyElement {
        let agent = info.agent;
        let pick = self.state.composer.pick(agent);
        let (brand, _) = colors::brand(agent);
        let status = &info.status;
        // a plan already says it is signed in
        let mut facts = vec![match (&status.plan, &status.account) {
            (Some(plan), _) => plan.clone(),
            (None, Some(_)) => "Signed in".to_string(),
            (None, None) => status.detail.clone().unwrap_or_else(|| "Installed".into()),
        }];
        facts.extend(status.version.as_ref().map(|v| format!("v{v}")));
        let lead = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(30.))
            .rounded(px(8.))
            .bg(brand.opacity(0.14))
            .child(mark(agent, 15., brand))
            .into_any_element();
        let efforts = info.catalog.efforts_for(&pick.model);
        let controls = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.dropdown(
                Picker::Model(agent),
                model_label(&info.catalog, &pick.model),
                cx,
            ))
            .when(!efforts.is_empty(), |d| {
                d.child(self.dropdown(Picker::Effort(agent), effort_name(&pick.effort), cx))
            });
        row_with(
            Some(lead),
            agent.name(),
            text(facts.join(" · ")).font_weight(FontWeight::NORMAL),
            controls,
        )
    }
}

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
    row_with(
        None,
        "Install one, then check again",
        div()
            .pt(px(6.))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .children(chips),
        widgets::button("agents-recheck", "Check again")
            .on_click(cx.listener(|r, _: &ClickEvent, _, _| r.client.send(Command::LoadAgents))),
    )
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
        assert_eq!(effort_name(""), "Default effort");
    }
}
