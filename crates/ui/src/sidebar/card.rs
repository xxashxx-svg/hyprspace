// The card that shows beside a thread's row when the pointer rests on it, after T3 Code's: the
// title, the project and its folder, and the agent with its model and effort. The row itself only
// has room for the agent's mark.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, Render, SharedString, Window, div, prelude::*, px,
};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

use crate::assets::{icon, mark};
use crate::colors;

#[derive(Clone)]
pub struct RowCard {
    pub title: SharedString,
    pub space: SharedString,
    pub path: SharedString,
    pub agent: Option<Agent>,
    /// The agent with its model and effort, "Claude Opus 5.5 · High", or "Terminal".
    pub model: SharedString,
}

/// One line of the card: a glyph in a fixed column, then its text.
fn line(glyph: AnyElement, text: SharedString) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_none()
                .justify_center()
                .w(px(14.))
                .child(glyph),
        )
        .child(div().min_w_0().truncate().child(text))
}

impl Render for RowCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let agent = match self.agent {
            Some(a) => mark(a, 12., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 12., colors::text3()).into_any_element(),
        };
        div()
            // a tooltip is drawn apart from the window's tree, so it takes the interface font itself
            .font_family(crate::settings::ui_font())
            .max_w(px(320.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .text_size(px(12.))
            .text_color(colors::text2())
            .child(
                div()
                    .truncate()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(self.title.clone()),
            )
            .child(line(
                icon("folder", 12., colors::text3()).into_any_element(),
                self.space.clone(),
            ))
            .child(
                div()
                    .pl(px(21.))
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::text3())
                    .child(self.path.clone()),
            )
            .child(line(agent, self.model.clone()))
    }
}
