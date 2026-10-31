// Putting settings back: the arrow beside a changed row, and Restore defaults at the foot of
// General and Appearance with the box that lists each value it will put back.

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, MouseButton, SharedString,
    Window, div, prelude::*,
};
use hyprspace_proto::state::AppState;
use hyprspace_theme::THEMES;

use super::agents::effort_name;
use super::appearance::{
    DIFFS, MAX_SIZE, MIN_SIZE, SCHEMES, font_label, line_height, ui_font_label,
};
use super::controls::{group, row};
use super::defaults::Field;
use super::general::{MODES, settle_name};
use super::index::{self, Setting};
use super::{Root, Tab};
use crate::assets::icon;
use crate::zoom::px;
use crate::{colors, widgets};

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

fn label(s: &Setting) -> String {
    let twin = index::settings()
        .filter(|o| o.tab == s.tab && o.title == s.title)
        .count()
        > 1;
    if twin {
        format!("{} {}", s.section, s.title.to_lowercase())
    } else {
        s.title.to_string()
    }
}

fn and_list(names: Vec<String>) -> String {
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

impl Root {
    fn field_value(&self, f: Field, s: &AppState) -> String {
        let a = &s.appearance;
        match f {
            Field::Permission => MODES
                .iter()
                .find(|m| m.0 == s.composer.permission)
                .map_or("", |m| m.1)
                .into(),
            Field::Settle => settle_name(s.settle_after).into(),
            Field::Titles => on_off(s.titles).into(),
            Field::Structured => on_off(s.composer.structured).into(),
            Field::Working => on_off(s.working_shelf).into(),
            Field::Theme if a.theme == colors::WALLPAPER => "From wallpaper".into(),
            Field::Theme => THEMES
                .iter()
                .find(|t| t.id == a.theme)
                .map_or(a.theme.as_str(), |t| t.name)
                .into(),
            Field::Scheme => SCHEMES
                .iter()
                .find(|m| m.0 == a.scheme)
                .map_or("", |m| m.1)
                .into(),
            Field::Glass => on_off(a.glass).into(),
            Field::UiFont => ui_font_label(&a.ui_font).into(),
            Field::Diff => DIFFS
                .iter()
                .find(|d| d.0 == a.diff_colors)
                .map_or("", |d| d.1)
                .into(),
            Field::Animations => on_off(a.animations).into(),
            Field::TermFont => font_label(&a.terminal_font),
            Field::TermSize => format!("{:.0} px", a.terminal_font_size.clamp(MIN_SIZE, MAX_SIZE)),
            Field::LineHeight => format!("{:.2}", line_height(a.terminal_line_height)),
            Field::Pick(agent) => {
                let pick = s.composer.pick(agent);
                let catalog = self
                    .agents
                    .iter()
                    .find(|i| i.agent == agent)
                    .map(|i| &i.catalog);
                format!(
                    "{}, {}",
                    crate::models::name(catalog, &pick.model),
                    effort_name(&pick.effort)
                )
            }
        }
    }

    fn reset_fields(&mut self, fields: &[Field], window: &mut Window, cx: &mut Context<Self>) {
        for f in fields {
            f.reset(&mut self.state);
        }
        self.apply_theme(window);
        self.update_prefs(|_| {}, cx);
        self.tidy_threads(cx);
    }

    fn reset_button(&self, s: &'static Setting, cx: &mut Context<Self>) -> Option<AnyElement> {
        let changed: Vec<Field> = s
            .fields
            .iter()
            .copied()
            .filter(|f| f.changed(&self.state))
            .collect();
        if changed.is_empty() {
            return None;
        }
        let fresh = AppState::default();
        let to = changed
            .iter()
            .map(|f| self.field_value(*f, &fresh))
            .collect::<Vec<_>>()
            .join(", ");
        Some(
            div()
                .id(SharedString::from(format!(
                    "reset-{}-{}",
                    s.section, s.title
                )))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|d| d.bg(colors::ink(0.08)))
                .child(icon("rotate-ccw", 13., colors::text3()))
                .tooltip(widgets::tip_text(format!("Reset to {to}")))
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.reset_fields(s.fields, window, cx)
                }))
                .into_any_element(),
        )
    }

    pub(super) fn with_reset(
        &self,
        s: &'static Setting,
        control: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(self.reset_button(s, cx))
            .child(control)
    }

    pub(super) fn item(
        &self,
        s: &'static Setting,
        control: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = self.with_reset(s, control, cx);
        self.spot(s.spot(), row(s.title, s.desc, control), false)
    }

    fn changed_here(&self, tab: Tab) -> Vec<&'static Setting> {
        index::settings()
            .filter(|s| s.tab == tab && s.fields.iter().any(|f| f.changed(&self.state)))
            .collect()
    }

    pub(super) fn restore_group(&self, tab: Tab, cx: &mut Context<Self>) -> AnyElement {
        let changed = self.changed_here(tab);
        let desc = if changed.is_empty() {
            "Everything here is as it started.".to_string()
        } else {
            format!(
                "Changed here: {}.",
                and_list(changed.iter().map(|s| label(s)).collect())
            )
        };
        let off = changed.is_empty();
        let button = widgets::button("settings-restore", "Restore")
            .when(off, |b| b.opacity(0.5).cursor_default())
            .when(!off, |b| {
                b.on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    r.settings.restore = Some(tab);
                    cx.notify();
                }))
            });
        group("Defaults", vec![row("Restore defaults", desc, button)]).into_any_element()
    }

    pub(super) fn restore_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tab = self.settings.restore?;
        let fields: Vec<Field> = self
            .changed_here(tab)
            .into_iter()
            .flat_map(|s| s.fields.iter().copied())
            .filter(|f| f.changed(&self.state))
            .collect();
        if fields.is_empty() {
            return None;
        }
        let fresh = AppState::default();
        let lines = fields.iter().enumerate().map(|(i, &f)| {
            div()
                .mx(px(12.))
                .py(px(8.))
                .flex()
                .justify_between()
                .gap(px(12.))
                .text_size(px(12.5))
                .when(i > 0, |d| d.border_t_1().border_color(colors::border1()))
                .child(div().text_color(colors::text1()).child(f.name()))
                .child(div().text_color(colors::text3()).child(format!(
                    "{} \u{2192} {}",
                    self.field_value(f, &self.state),
                    self.field_value(f, &fresh)
                )))
        });
        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(20.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(format!("Restore {} defaults?", index::tab_label(tab))),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .line_height(px(18.))
                            .text_color(colors::text2())
                            .child("These go back to how they started. The rest of Settings stays as it is."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(9.))
                    .bg(colors::ink(0.035))
                    .children(lines),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        widgets::button("restore-cancel", "Cancel")
                            .tooltip(widgets::tip("Esc"))
                            .on_click(cx.listener(|r, _: &ClickEvent, _, cx| {
                                r.settings.restore = None;
                                cx.notify();
                            })),
                    )
                    .child(widgets::primary("restore-confirm", "Restore").on_click(
                        cx.listener(move |r, _: &ClickEvent, window, cx| {
                            r.settings.restore = None;
                            r.reset_fields(&fields, window, cx);
                        }),
                    )),
            );
        let scrim = colors::hsla(colors::theme().shadow);
        Some(
            div()
                .id("settings-restore-box")
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .bg(scrim.opacity(0.6))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|r, _, _, cx| {
                        cx.stop_propagation();
                        r.settings.restore = None;
                        cx.notify();
                    }),
                )
                .child(crate::slide::rise_in(card, "settings-restore-in"))
                .into_any_element(),
        )
    }
}
