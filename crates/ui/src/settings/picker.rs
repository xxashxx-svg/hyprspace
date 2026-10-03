// The open dropdown, dropped under its field: the agent, permission and opener on General, the
// terminal font on Appearance, and each agent's model and effort on Agents.

use gpui::{AnyElement, ClickEvent, Context, SharedString, Window, div, point, prelude::*, px};
use hyprspace_proto::state::Pick;

use super::agents::effort_name;
use super::appearance::{SYSTEM_UI, mono_families};
use super::general::MODES;
use super::{Picker, Root};
use crate::widgets;

const PERMISSION_MENU: f32 = 360.;

impl Root {
    pub(super) fn picker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (click, picker) = self.settings.menu?;
        let field = self.settings.fields.borrow().get(&picker).copied();
        let body = match picker {
            Picker::Agent => self.agent_menu(cx),
            Picker::Permission => self.permission_menu(cx),
            Picker::Opener => self.opener_menu(cx),
            Picker::Font => self.font_menu(window, cx),
            Picker::UiFont => self.ui_font_menu(cx),
            Picker::Model(_) | Picker::Effort(_) => self.pick_menu(picker, cx)?,
        };
        let close = cx.listener(|r, _: &(), _, cx| {
            r.settings.menu = None;
            cx.notify();
        });
        let close = move |w: &mut Window, cx: &mut gpui::App| close(&(), w, cx);
        Some(match field {
            // the modes' notes need more room than the field, so the menu grows to the left
            Some(b) if picker == Picker::Permission => widgets::popup(
                point(b.right() - px(PERMISSION_MENU), b.bottom() + px(4.)),
                widgets::Open::Down,
                window,
                close,
                body.w(px(PERMISSION_MENU)),
            ),
            Some(b) => widgets::dropdown(b, window, close, body),
            None => widgets::popup(click, widgets::Open::Down, window, close, body),
        })
    }

    fn agent_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let current = self.new_thread_agent();
        div().flex().flex_col().children(
            self.agents
                .iter()
                .filter(|a| a.status.installed)
                .enumerate()
                .map(|(i, a)| {
                    let agent = a.agent;
                    widgets::menu_item(("agent", i), agent.name(), None, Some(agent) == current)
                        .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.settings.menu = None;
                            r.update_prefs(|p| p.agent = Some(agent), cx);
                        }))
                }),
        )
    }

    fn permission_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let current = self.state.composer.permission;
        div()
            .flex()
            .flex_col()
            .children(MODES.iter().enumerate().map(|(i, &(mode, name, says))| {
                widgets::menu_item(
                    ("permission", i),
                    name,
                    Some(SharedString::from(says)),
                    mode == current,
                )
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    r.settings.menu = None;
                    r.update_prefs(|p| p.permission = mode, cx);
                }))
            }))
    }

    fn opener_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let current = self.state.open_with;
        div()
            .flex()
            .flex_col()
            .children(self.work.openers.iter().enumerate().map(|(i, &o)| {
                widgets::menu_item(("opener", i), o.name(), None, o == current).on_click(
                    cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.settings.menu = None;
                        r.pick_opener(o, cx);
                    }),
                )
            }))
    }

    /// The interface font: the bundled Geist, or whatever the system uses for its own UI.
    fn ui_font_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let current = self.state.appearance.ui_font.clone();
        let system = if cfg!(target_os = "macos") {
            "San Francisco, as macOS draws its own"
        } else {
            "Segoe UI, as Windows draws its own"
        };
        let choices = [
            (String::new(), "Geist", "Built in"),
            (SYSTEM_UI.to_string(), "System", system),
        ];
        div()
            .flex()
            .flex_col()
            .children(
                choices
                    .into_iter()
                    .enumerate()
                    .map(|(i, (family, name, note))| {
                        let checked = family == current;
                        widgets::menu_item(("ui-font", i), name, Some(note.into()), checked)
                            .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                                r.settings.menu = None;
                                r.state.appearance.ui_font = family.clone();
                                r.apply_theme(window);
                                r.save();
                                cx.notify();
                            }))
                    }),
            )
    }

    fn font_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let current = self.state.appearance.terminal_font.clone();
        let mut fonts = self.settings.fonts.borrow_mut();
        let fonts = fonts
            .get_or_insert_with(|| mono_families(window.text_system().all_font_names()))
            .clone();
        let families = std::iter::once(String::new()).chain(fonts);
        div()
            .flex()
            .flex_col()
            .children(families.enumerate().map(|(i, family)| {
                let checked = family == current;
                let (name, note) = if family.is_empty() {
                    (
                        "JetBrains Mono".to_string(),
                        Some("Built in, with Nerd Font icons".into()),
                    )
                } else {
                    (family.clone(), None)
                };
                widgets::menu_item(("font", i), name, note, checked).on_click(cx.listener(
                    move |r, _: &ClickEvent, window, cx| {
                        r.settings.menu = None;
                        r.state.appearance.terminal_font = family.clone();
                        r.apply_theme(window);
                        r.save();
                        cx.notify();
                    },
                ))
            }))
    }

    /// An agent's model or effort.
    fn pick_menu(&self, picker: Picker, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let agent = match picker {
            Picker::Model(a) | Picker::Effort(a) => a,
            _ => return None,
        };
        let info = self.agents.iter().find(|a| a.agent == agent)?;
        let pick = self.state.composer.pick(agent);
        Some(match picker {
            Picker::Model(_) => {
                div()
                    .flex()
                    .flex_col()
                    .children(info.catalog.models.iter().enumerate().map(|(i, m)| {
                        let id = m.id.clone();
                        let catalog = info.catalog.clone();
                        widgets::menu_item(
                            ("model", i),
                            m.label.clone(),
                            m.note.clone().map(SharedString::from),
                            m.id == pick.model,
                        )
                        .on_click(cx.listener(
                            move |r, _: &ClickEvent, _, cx| {
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
                            },
                        ))
                    }))
            }
            _ => {
                let levels = std::iter::once(String::new())
                    .chain(info.catalog.efforts_for(&pick.model).iter().cloned());
                div()
                    .flex()
                    .flex_col()
                    .children(levels.enumerate().map(|(i, level)| {
                        let checked = level == pick.effort;
                        widgets::menu_item(("effort", i), effort_name(&level), None, checked)
                            .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                                r.settings.menu = None;
                                let mut pick = r.state.composer.pick(agent);
                                pick.effort = level.clone();
                                r.update_prefs(|p| p.set_pick(pick), cx);
                            }))
                    }))
            }
        })
    }
}
