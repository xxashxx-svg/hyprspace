// Settings, Usage, in two views (the Tauri app's UsagePanel.tsx and usage-panel.css). Limits is
// what each plan allows and how much is left, from the same readings the ring uses, so opening it
// fetches nothing (`limits.rs`). Activity is what each agent has done, read from its own files on
// this machine (`activity.rs`). This file holds the pieces both draw with.

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, IntoElement, div, prelude::*, px,
};

use super::{View, brand};
use crate::assets::{icon, provider_mark};
use crate::root::Root;
use crate::time::now_ms;
use crate::{colors, widgets};

/// 1.2k, 3.4M, 5.0B.
pub(super) fn short(n: u64) -> String {
    let f = n as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1}k", f / 1e3)
    } else {
        n.to_string()
    }
}

/// 12,345.
pub(super) fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "ash@example.com" as "as•••@example.com": the panel can be on screen while sharing it.
pub(crate) fn masked(account: &str) -> String {
    match account.split_once('@') {
        Some((user, domain)) => {
            let head: String = user.chars().take(2).collect();
            format!("{head}•••@{domain}")
        }
        None => account.chars().take(2).collect::<String>() + "•••",
    }
}

pub(super) fn dim(text: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(colors::text3())
        .child(text.into())
}

/// The card every provider gets (`.up-card`), with its head.
pub(super) fn card(id: &str, name: &str, plan: Option<String>, right: Option<String>) -> Div {
    let tint = brand(id);
    div()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .bg(colors::ink(0.035))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(16.))
                .py(px(12.))
                .border_b_1()
                .border_color(colors::border1())
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(26.))
                        .rounded(px(8.))
                        .bg(tint.opacity(0.16))
                        .children(provider_mark(id, 15., tint)),
                )
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child(name.to_string()),
                )
                .children(plan.map(|p| {
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded_full()
                        .bg(colors::ink(0.06))
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text2())
                        .child(p)
                }))
                .children(right.map(|r| {
                    div()
                        .ml_auto()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(r)
                })),
        )
}

/// Rows stacked with hairlines between them.
pub(super) fn rows(items: Vec<AnyElement>) -> Div {
    div()
        .flex()
        .flex_col()
        .children(items.into_iter().enumerate().map(|(i, r)| {
            div()
                .when(i > 0, |d| d.border_t_1().border_color(colors::border0()))
                .child(r)
        }))
}

pub(super) fn note(text: String) -> AnyElement {
    div()
        .px(px(16.))
        .py(px(10.))
        .border_t_1()
        .border_color(colors::border0())
        .text_size(px(12.))
        .text_color(colors::text3())
        .child(text)
        .into_any_element()
}

/// The big number with its unit after it: "58% left".
pub(super) fn big(value: String, unit: &str, color: Hsla) -> Div {
    div()
        .flex()
        .items_end()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(26.))
                .line_height(px(30.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(value),
        )
        .child(
            div()
                .pb(px(4.))
                .text_size(px(13.))
                .text_color(colors::text3())
                .child(unit.to_string()),
        )
}

pub(super) fn foot_text(text: &str) -> AnyElement {
    div()
        .px(px(2.))
        .pt(px(2.))
        .max_w(px(620.))
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(colors::text3())
        .child(text.to_string())
        .into_any_element()
}

impl Root {
    pub(crate) fn usage_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let now = now_ms() as i64;
        // the chart takes clicks, so it is built before `limits` is borrowed for the rest
        let chart = (self.limits.read(cx).view == View::Activity)
            .then(|| super::chart::Data::new(self.limits.read(cx)))
            .and_then(|d| super::chart::render(d, &self.limits.clone(), cx));
        let limits = self.limits.read(cx);
        let view = limits.view;
        let loading = !limits.pending.is_empty();
        let body = match view {
            View::Limits => limits.limits_view(now),
            View::Activity => limits.activity_view(chart),
        };
        let tabs = widgets::segments().children(
            [(View::Limits, "Limits"), (View::Activity, "Activity")]
                .into_iter()
                .enumerate()
                .map(|(i, (v, name))| {
                    widgets::segment(("usage-view", i), None, name, view == v, false).on_click(
                        cx.listener(move |r, _: &ClickEvent, _, cx| {
                            r.limits.update(cx, |l, cx| l.set_view(v, cx));
                        }),
                    )
                }),
        );
        let status = match view {
            View::Limits => "Refreshes every 3 minutes",
            View::Activity if loading => "Reading local files",
            View::Activity => "From each agent's own files",
        };
        let refresh = (view == View::Activity).then(|| {
            widgets::button("usage-refresh", "Refresh")
                .flex_row_reverse()
                .when(loading, |d| d.opacity(0.5))
                .child(icon("rotate-cw", 13., colors::text2()))
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    if !loading {
                        r.limits.update(cx, |l, cx| l.load_activity(cx));
                    }
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(tabs)
                    .child(dim(status).flex_1())
                    .children(refresh),
            )
            .children(body)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_short() {
        assert_eq!(short(950), "950");
        assert_eq!(short(1_250), "1.2k");
        assert_eq!(short(3_400_000), "3.4M");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(grouped(12), "12");
        assert_eq!(masked("ash@example.com"), "as•••@example.com");
    }
}
