// Small drawings of the app for the intro's tour, one per topic. The Tauri app's intro ran
// working copies of each part (onboarding/demos.tsx); these are still pictures in the same
// shapes, drawn from the theme's tokens so they follow every theme and side.

use gpui::{AnyElement, Div, FontWeight, Hsla, IntoElement, div, prelude::*, px, relative};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

use crate::assets::{icon, mark};
use crate::colors;

/// The frame every sketch sits in (`.dm-rail`, `.dm-composer`...).
fn panel(w: f32) -> Div {
    div()
        .w(px(w))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::surface1())
        .shadow(colors::shadow())
}

fn line(w: f32, a: f32) -> Div {
    div()
        .h(px(5.))
        .w(relative(w))
        .rounded(px(3.))
        .bg(colors::ink(a))
}

fn dot(c: Option<Hsla>) -> Div {
    div()
        .flex_none()
        .size(px(6.))
        .rounded_full()
        .bg(c.unwrap_or_else(|| colors::ink(0.2)))
}

fn thread(agent: Option<Agent>, title: &str, state: Option<Hsla>, fresh: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(30.))
        .pl(px(26.))
        .pr(px(8.))
        .rounded(px(6.))
        .when(fresh, |d| d.bg(colors::ink(0.07)))
        .text_size(px(12.5))
        .text_color(colors::text1())
        .child(match agent {
            Some(a) => mark(a, 13., colors::brand(a).0).into_any_element(),
            None => icon("terminal", 13., colors::text2()).into_any_element(),
        })
        .child(div().flex_1().min_w_0().truncate().child(title.to_string()))
        .child(dot(state))
}

fn folder(name: &str, count: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(8.))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text2())
        .child(icon("chevron-down", 12., colors::text3()))
        .child(name.to_string())
        .child(
            div()
                .ml_auto()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child(count.to_string()),
        )
}

fn search(new_lit: bool) -> Div {
    div()
        .flex()
        .gap(px(5.))
        .mb(px(6.))
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(7.))
                .h(px(28.))
                .px(px(9.))
                .rounded(px(7.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(0.04))
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(icon("search", 12., colors::text3()))
                .child("Search"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(7.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(if new_lit { 0.12 } else { 0.04 }))
                .child(icon(
                    "square-pen",
                    13.,
                    if new_lit {
                        colors::text1()
                    } else {
                        colors::text2()
                    },
                )),
        )
}

pub fn folders() -> AnyElement {
    panel(290.)
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(8.))
        .child(search(false))
        .child(folder("api-server", "2"))
        .child(thread(
            Some(Agent::Claude),
            "Fix the flaky login test",
            Some(colors::ok()),
            false,
        ))
        .child(thread(
            Some(Agent::Codex),
            "Add rate limits",
            Some(colors::busy()),
            false,
        ))
        .child(folder("website", "1"))
        .child(thread(
            Some(Agent::Codex),
            "Draft the pricing page",
            None,
            false,
        ))
        .into_any_element()
}

pub fn threads() -> AnyElement {
    panel(290.)
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(8.))
        .child(search(true))
        .child(folder("api-server", "3"))
        .child(thread(Some(Agent::Claude), "New thread", None, true))
        .child(thread(
            Some(Agent::Claude),
            "Fix the flaky login test",
            Some(colors::ok()),
            false,
        ))
        .child(thread(None, "Terminal", None, false))
        .into_any_element()
}

fn chip(content: AnyElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .h(px(22.))
        .px(px(7.))
        .rounded(px(6.))
        .border_1()
        .border_color(colors::border1())
        .text_size(px(11.))
        .text_color(colors::text2())
        .child(content)
}

pub fn composer() -> AnyElement {
    panel(380.)
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(64.))
                .px(px(12.))
                .pt(px(10.))
                .text_size(px(13.))
                .text_color(colors::text1())
                .child("Fix the flaky login test"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .py(px(7.))
                .border_t_1()
                .border_color(colors::border1())
                .child(chip(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child(mark(Agent::Claude, 11., colors::brand(Agent::Claude).0))
                        .child("Opus 5.5")
                        .into_any_element(),
                ))
                .child(chip("High".into_any_element()))
                .child(chip("Ask first".into_any_element()))
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(24.))
                        .rounded_full()
                        .bg(colors::accent())
                        .child(icon("arrow-up", 12., colors::on_accent())),
                ),
        )
        .into_any_element()
}

fn mini_pane(agent: Agent, name: &str, widths: [f32; 3]) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::bg())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .mb(px(2.))
                .text_size(px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text2())
                .child(mark(agent, 11., colors::brand(agent).0))
                .child(name.to_string()),
        )
        .child(line(widths[0], 0.12))
        .child(line(widths[1], 0.12))
        .child(line(widths[2], 0.06))
}

/// A file's card over the thread it was opened from.
pub fn files() -> AnyElement {
    let code = |w: f32, a: f32| line(w, a).h(px(4.));
    div()
        .relative()
        .w(px(380.))
        .h(px(200.))
        .child(
            mini_pane(Agent::Claude, "Login test", [0.7, 0.45, 0.55])
                .size_full()
                .opacity(0.5),
        )
        .child(
            div()
                .absolute()
                .left(px(44.))
                .top(px(34.))
                .w(px(292.))
                .h(px(146.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(8.))
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface2())
                .shadow(colors::shadow())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .h(px(26.))
                        .px(px(8.))
                        .border_b_1()
                        .border_color(colors::border1())
                        .text_size(px(10.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text1())
                        .child(icon("file-code", 11., colors::text3()))
                        .child(div().flex_1().child("auth.rs"))
                        .child(icon("x", 11., colors::text3())),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(7.))
                        .p(px(10.))
                        .child(code(0.55, 0.14))
                        .child(code(0.8, 0.1))
                        .child(code(0.7, 0.1))
                        .child(code(0.4, 0.14))
                        .child(code(0.65, 0.1)),
                ),
        )
        .into_any_element()
}

pub fn palette() -> AnyElement {
    let row = |glyph: &str, text: &str, on: bool| {
        div()
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(32.))
            .px(px(6.))
            .rounded(px(7.))
            .when(on, |d| d.bg(colors::accent().opacity(0.14)))
            .text_size(px(12.))
            .text_color(colors::text1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .rounded(px(6.))
                    .bg(colors::ink(0.05))
                    .child(icon(glyph, 12., colors::text2())),
            )
            .child(text.to_string())
    };
    panel(380.)
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(9.))
                .h(px(40.))
                .px(px(12.))
                .border_b_1()
                .border_color(colors::border1())
                .text_size(px(13.))
                .text_color(colors::text1())
                .child(icon("search", 13., colors::accent()))
                .child("git"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .p(px(6.))
                .child(row("panel-right", "Show or hide files and git", true))
                .child(row("git-branch", "Open the git panel", false))
                .child(row("text-search", "api-server › Terminal", false)),
        )
        .into_any_element()
}

pub fn terminal() -> AnyElement {
    let text = |t: &str, c: Hsla| div().child(t.to_string()).text_color(c);
    panel(380.)
        .p(px(12.))
        .bg(colors::bg())
        .font_family(MONO)
        .text_size(px(11.5))
        .line_height(px(18.))
        .flex()
        .flex_col()
        .child(text("PS api-server> cargo test", colors::text2()))
        .child(text("running 42 tests", colors::text3()))
        .child(
            div()
                .flex()
                .gap(px(6.))
                .text_color(colors::error())
                .child("error at")
                .child(
                    div()
                        .text_color(colors::link())
                        .border_b_1()
                        .border_color(colors::link())
                        .child("src/auth.rs:48:9"),
                ),
        )
        .child(text(
            "test result: FAILED. 41 passed; 1 failed",
            colors::text3(),
        ))
        .into_any_element()
}

pub fn git() -> AnyElement {
    let file = |name: &str, ticked: bool, tag: &str| {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(26.))
            .text_size(px(12.))
            .text_color(colors::text1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(13.))
                    .rounded(px(3.))
                    .border_1()
                    .border_color(if ticked {
                        colors::accent()
                    } else {
                        colors::border2()
                    })
                    .when(ticked, |d| d.bg(colors::accent()))
                    .when(ticked, |d| d.child(icon("check", 9., colors::on_accent()))),
            )
            .child(div().flex_1().child(name.to_string()))
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(colors::busy())
                    .child(tag.to_string()),
            )
    };
    panel(300.)
        .p(px(10.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(file("src/auth.rs", true, "M"))
        .child(file("tests/login.rs", true, "M"))
        .child(file("notes.md", false, "U"))
        .child(
            div()
                .mt(px(8.))
                .h(px(28.))
                .px(px(9.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::bg())
                .text_size(px(12.))
                .text_color(colors::text1())
                .child("Fix the flaky login test"),
        )
        .child(
            div()
                .mt(px(8.))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .bg(colors::accent())
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::on_accent())
                .child("Commit 2 files"),
        )
        .into_any_element()
}

pub fn usage() -> AnyElement {
    let window = |label: &str, pct: f32, reset: &str, c: Hsla| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .child(label.to_string())
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(15.))
                            .text_color(colors::text1())
                            .child(format!("{pct}%")),
                    ),
            )
            .child(
                div()
                    .h(px(5.))
                    .rounded_full()
                    .bg(colors::ink(0.07))
                    .child(div().h_full().w(relative(pct / 100.)).rounded_full().bg(c)),
            )
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.))
                    .text_color(colors::text3())
                    .child(format!("resets in {reset}")),
            )
    };
    let brand = colors::brand(Agent::Claude).0;
    panel(272.)
        .p(px(12.))
        .flex()
        .flex_col()
        .gap(px(13.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .child(mark(Agent::Claude, 14., brand))
                .child("Claude"),
        )
        .child(window("Session", 42., "2h 16m", brand))
        .child(window("All models", 18., "3d 4h", brand))
        .into_any_element()
}

/// The welcome step's picture: the sidebar, and four threads tiled in the grid.
pub fn app() -> AnyElement {
    let rail = div()
        .flex_none()
        .w(px(150.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(10.))
        .border_r_1()
        .border_color(colors::border1())
        .child(
            div()
                .h(px(18.))
                .rounded(px(5.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::ink(0.04)),
        )
        .children(
            [("api-server", 2), ("website", 1), ("mobile-app", 0)].map(|(name, n)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text2())
                            .child(name),
                    )
                    .children((0..n).map(|i| {
                        div()
                            .ml(px(8.))
                            .h(px(7.))
                            .w(relative(if i == 0 { 0.8 } else { 0.55 }))
                            .rounded(px(3.))
                            .bg(colors::ink(0.1))
                    }))
            }),
        );
    let grid = div()
        .flex_1()
        .grid()
        .grid_cols(2)
        .grid_rows(2)
        .gap(px(1.))
        .bg(colors::border1())
        .children(
            [
                (Agent::Claude, 0.7, 0.45, 0.55),
                (Agent::Codex, 0.61, 0.53, 0.51),
                (Agent::Codex, 0.52, 0.61, 0.47),
                (Agent::Claude, 0.43, 0.69, 0.43),
            ]
            .map(|(agent, a, b, c)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .px(px(10.))
                    .py(px(8.))
                    .bg(colors::bg())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .mb(px(2.))
                            .text_size(px(10.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text2())
                            .child(mark(agent, 12., colors::brand(agent).0))
                            .child(agent.name()),
                    )
                    .child(line(a, 0.12))
                    .child(line(b, 0.12))
                    .child(line(c, 0.06))
            }),
        );
    div()
        .flex_none()
        .flex()
        .h(px(210.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::bg())
        .overflow_hidden()
        .child(rail)
        .child(grid)
        .into_any_element()
}
