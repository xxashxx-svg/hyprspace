// The corner card (updater.css `.updater`): "Update x available" with Restart and update, a
// spinner and progress bar while it installs, the error with Retry, and What's new after an
// update. It shows only when there is something to act on or read.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, Div, Entity, FontWeight,
    Transformation, div, percentage, prelude::*, px, relative,
};
use hyprspace_proto::update::Step;

use super::{Phase, Updater, VERSION, step_text};
use crate::assets::icon;
use crate::{colors, widgets};

/// The cards for the window's bottom right corner, or None when there is nothing to show.
pub fn overlay(updater: &Entity<Updater>, cx: &App) -> Option<AnyElement> {
    let u = updater.read(cx);
    let news = u.whats_new.clone().map(|notes| whats_new(updater, notes));
    let card = match &u.phase {
        Phase::Available(r) if !u.dismissed => Some(available(updater, &r.version)),
        Phase::Failed(m) if !u.dismissed => Some(failed(updater, m)),
        Phase::Busy(step) => Some(busy(*step, u.progress())),
        _ => None,
    };
    if news.is_none() && card.is_none() {
        return None;
    }
    Some(
        div()
            .absolute()
            .right(px(14.))
            .bottom(px(14.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(news)
            .children(card)
            .into_any_element(),
    )
}

fn frame() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(9.))
        .py(px(8.))
        .pl(px(12.))
        .pr(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.))
        .text_color(colors::text1())
        // clicks on the card stay on the card
        .occlude()
}

fn dot(error: bool) -> Div {
    div().flex_none().size(px(7.)).rounded_full().bg(if error {
        colors::error()
    } else {
        colors::accent()
    })
}

fn close(updater: &Entity<Updater>, id: &'static str, news: bool) -> AnyElement {
    let updater = updater.clone();
    div()
        .id(id)
        .flex_none()
        .px(px(2.))
        .cursor_pointer()
        .child(icon("x", 13., colors::text3()))
        .on_click(move |_: &ClickEvent, _, cx| {
            updater.update(cx, |u, cx| {
                if news {
                    u.close_whats_new(cx)
                } else {
                    u.dismiss(cx)
                }
            })
        })
        .into_any_element()
}

fn install_button(updater: &Entity<Updater>, label: &'static str) -> AnyElement {
    let updater = updater.clone();
    widgets::primary("update-install", label)
        .h(px(24.))
        .px(px(9.))
        .font_weight(FontWeight::NORMAL)
        .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, |u, cx| u.install(cx)))
        .into_any_element()
}

fn available(updater: &Entity<Updater>, version: &str) -> AnyElement {
    frame()
        .child(dot(false))
        .child(
            div()
                .whitespace_nowrap()
                .child(format!("Update {version} available")),
        )
        .child(install_button(updater, "Restart and update"))
        .child(close(updater, "update-later", false))
        .into_any_element()
}

fn failed(updater: &Entity<Updater>, message: &str) -> AnyElement {
    frame()
        .max_w(px(460.))
        .child(dot(true))
        .child(div().min_w_0().child(message.to_string()))
        .child(install_button(updater, "Retry"))
        .child(close(updater, "update-dismiss", false))
        .into_any_element()
}

fn busy(step: Step, progress: Option<f32>) -> AnyElement {
    let spinner = icon("ring", 13., colors::accent()).with_animation(
        "update-spin",
        Animation::new(Duration::from_millis(700)).repeat(),
        |s, t| s.with_transformation(Transformation::rotate(percentage(t))),
    );
    let bar = match progress {
        Some(p) => div()
            .h_full()
            .w(relative(p))
            .rounded_full()
            .bg(colors::accent())
            .into_any_element(),
        // size unknown: a slice that slides across
        None => div()
            .absolute()
            .h_full()
            .w(relative(0.35))
            .rounded_full()
            .bg(colors::accent())
            .with_animation(
                "update-slide",
                Animation::new(Duration::from_millis(1100)).repeat(),
                |d, t| d.left(relative(-0.35 + 1.35 * t)),
            )
            .into_any_element(),
    };
    frame()
        .flex_col()
        .items_stretch()
        .gap(px(8.))
        .min_w(px(230.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(spinner)
                .child(div().whitespace_nowrap().child(step_text(step))),
        )
        .child(
            div()
                .relative()
                .h(px(4.))
                .w_full()
                .rounded_full()
                .overflow_hidden()
                .bg(colors::surface3())
                .child(bar),
        )
        .into_any_element()
}

fn whats_new(updater: &Entity<Updater>, notes: Vec<String>) -> AnyElement {
    let body: Vec<AnyElement> = if notes.is_empty() {
        vec![div().child("Thanks for updating.").into_any_element()]
    } else {
        notes
            .into_iter()
            .map(|n| {
                div()
                    .flex()
                    .gap(px(7.))
                    .child(div().flex_none().text_color(colors::text3()).child("•"))
                    .child(div().min_w_0().child(n))
                    .into_any_element()
            })
            .collect()
    };
    frame()
        .items_start()
        .w(px(360.))
        .py(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("What's new in v{VERSION}")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .text_color(colors::text2())
                        .line_height(px(17.))
                        .children(body),
                ),
        )
        .child(close(updater, "whats-new-close", true))
        .into_any_element()
}
