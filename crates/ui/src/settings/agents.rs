// Settings, Agents: one row per installed agent with the model and effort its new threads start
// with, which are the composer's saved picks, how it's installed and its update. Agents that are
// not installed share one row at the foot, with a way to look again after installing one.

use gpui::{AnyElement, ClickEvent, ClipboardItem, Context, FontWeight, div, prelude::*};
use hyprspace_proto::agent_update::{How, Outcome};
use hyprspace_proto::agents::{AgentCatalog, AgentInfo};
use hyprspace_proto::{Agent, Command};

use super::controls::{block, row_with, section, text};
use super::index::{CLAUDE, CODEX};
use super::{Picker, Root};
use crate::assets::mark;
use crate::composer::effort_label;
use crate::zoom::px;
use crate::{colors, widgets};

pub(super) fn model_label(catalog: &AgentCatalog, model: &str) -> String {
    crate::models::name(Some(catalog), model)
}

pub(super) fn effort_name(effort: &str) -> String {
    effort_label(effort)
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
        let pick = crate::models::resolve(&info.catalog, self.state.composer.pick(agent));
        let (brand, _) = colors::brand(agent);
        let status = &info.status;
        // a plan already says it is signed in
        let mut facts = vec![match (&status.plan, &status.account) {
            (Some(plan), _) => plan.clone(),
            (None, Some(_)) => "Signed in".to_string(),
            (None, None) => status.detail.clone().unwrap_or_else(|| "Installed".into()),
        }];
        let updater = self.updater.read(cx);
        let install = updater.agents.install(agent).cloned();
        let version = install
            .as_ref()
            .and_then(|i| i.version.clone())
            .or_else(|| status.version.clone());
        facts.extend(version.map(|v| format!("v{v}")));
        facts.extend(install.as_ref().and_then(|i| match i.how {
            How::Native => Some("Native install".to_string()),
            How::Standalone => Some("Standalone install".to_string()),
            How::Npm => Some("Installed with npm".to_string()),
            How::Homebrew => Some("Installed with Homebrew".to_string()),
            How::Unknown => None,
        }));
        let running = updater.agents.running.contains(&agent);
        let outcome = updater.agents.outcome(agent).cloned();
        let update = update_line(agent, install, outcome, running, cx);
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
        let setting = match agent {
            Agent::Claude => &CLAUDE,
            Agent::Codex => &CODEX,
        };
        self.spot(
            setting.spot(),
            row_with(
                Some(lead),
                agent.name(),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(text(facts.join(" · ")).font_weight(FontWeight::NORMAL))
                    .children(update),
                self.with_reset(setting, controls, cx),
            ),
            false,
        )
    }
}

fn small(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.h(px(24.)).px(px(9.)).text_size(px(12.))
}

fn copy_button(id: (&'static str, usize), command: String) -> gpui::Stateful<gpui::Div> {
    small(widgets::button(id, "Copy command")).on_click(move |_: &ClickEvent, _, cx| {
        crate::clipboard::write(cx, ClipboardItem::new_string(command.clone()))
    })
}

/// The update under an agent's name: running, how the last one went, or a newer version.
fn update_line(
    agent: Agent,
    install: Option<hyprspace_proto::agent_update::Install>,
    outcome: Option<Outcome>,
    running: bool,
    cx: &mut Context<Root>,
) -> Option<AnyElement> {
    let line = |s: String| text(s).font_weight(FontWeight::NORMAL);
    let row = div().flex().flex_wrap().items_center().gap(px(8.));
    let column = div().flex().flex_col().items_start().gap(px(6.));
    if running {
        return Some(
            row.child(crate::spinner::ring(
                ("agent-updating", agent as usize),
                colors::text3(),
            ))
            .child(line("Updating".into()))
            .into_any_element(),
        );
    }
    let install = install?;
    let id = |name: &'static str| (name, agent as usize);
    if let Some(outcome) = outcome {
        let said = crate::update::said(agent, &outcome);
        let mut row = column.child(match &outcome {
            Outcome::Failed { .. } => line(said).text_color(colors::error()),
            _ => line(said),
        });
        match outcome {
            Outcome::Updated { from, .. }
                if agent == Agent::Claude && install.how == How::Native =>
            {
                row = row.child(
                    small(widgets::button(
                        id("agent-revert"),
                        format!("Go back to {from}"),
                    ))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        let from = from.clone();
                        r.updater
                            .update(cx, |u, cx| u.revert_agent(agent, from, cx));
                    })),
                );
            }
            Outcome::Failed { .. } => {
                row = row.child(copy_button(id("agent-copy"), install.command.clone()));
            }
            _ => {}
        }
        return Some(row.into_any_element());
    }
    let latest = install.latest.clone()?;
    if install.held {
        return Some(
            line(format!(
                "{latest} is out, but HyprSpace holds it back because it breaks threads."
            ))
            .into_any_element(),
        );
    }
    if !install.runs() {
        return Some(
            column
                .child(line(format!(
                    "{latest} is out. Run `{}` to update.",
                    install.command
                )))
                .child(copy_button(id("agent-copy"), install.command))
                .into_any_element(),
        );
    }
    Some(
        row.child(line(format!("{latest} is out")))
            .child(
                small(widgets::primary(id("agent-update"), "Update")).on_click(cx.listener(
                    move |r, _: &ClickEvent, _, cx| {
                        r.updater
                            .update(cx, |u, cx| u.update_agents(&[agent], false, cx));
                    },
                )),
            )
            .into_any_element(),
    )
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
        assert_eq!(model_label(&catalog, ""), "Opus 5.5");
        assert_eq!(model_label(&catalog, "claude-gone-1"), "Gone 1");
        assert_eq!(effort_name("high"), "High");
    }
}
