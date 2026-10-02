// The effort slider, after the Tauri app's EffortSlider: a stop per level with its name under
// it, filled in the agent's colors. The leftmost stop is the CLI's default. Click a stop or its
// name to pick it.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px,
};

use super::Composer;
use super::card::model_label;
use super::pickers::{effort_label, effort_note, efforts};
use crate::assets::mark;
use crate::{colors, widgets};

/// A label under a stop has to fit, so the long names get a short form.
fn short(level: &str) -> String {
    match level {
        "minimal" => "Min".into(),
        "medium" => "Med".into(),
        "xhigh" => "X-high".into(),
        _ => effort_label(level),
    }
}

/// Where stop `i` of `n` sits along a line `width` wide.
fn stop_x(i: usize, n: usize, width: f32) -> f32 {
    if n < 2 {
        0.
    } else {
        width * i as f32 / (n - 1) as f32
    }
}

pub fn menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let Some(pick) = c.pick() else {
        return div().into_any_element();
    };
    let stops: Vec<String> = std::iter::once(String::new()).chain(efforts(c)).collect();
    let n = stops.len();
    let here = stops.iter().position(|s| *s == pick.effort).unwrap_or(0);
    let width = (n as f32 * 48. + 30.).max(300.);
    // the line is inset by the thumb's radius so the end stops sit under the thumb's centre
    let line = width - 30. - 18.;
    let (brand, brand2) = colors::brand(pick.agent);
    let fill = stop_x(here, n, line);
    let dots = stops.iter().enumerate().map(|(i, _)| {
        div()
            .absolute()
            .top(px(1.))
            .left(px(stop_x(i, n, line) - 2.))
            .size(px(4.))
            .rounded_full()
            .bg(if i < here {
                colors::bg().opacity(0.75)
            } else {
                colors::ink(0.3)
            })
            .when(i == here, |d| d.invisible())
    });
    // a hit area per stop, as wide as the gap between stops, over both the rail and the names
    let span = if n > 1 { line / (n - 1) as f32 } else { line };
    let hits: Vec<_> = stops
        .iter()
        .enumerate()
        .map(|(i, level)| {
            let mut p = pick.clone();
            p.effort = level.clone();
            div()
                .id(("effort-stop", i))
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(9. + stop_x(i, n, line) - span / 2.))
                .w(px(span))
                .cursor_pointer()
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| c.set_pick(p.clone(), cx)))
        })
        .collect();
    let labels = stops.iter().enumerate().map(|(i, level)| {
        let x = stop_x(i, n, line);
        let (left, align_end) = if i == 0 {
            (x - 9., false)
        } else if i == n - 1 {
            (x - 48. + 9., true)
        } else {
            (x - 24., false)
        };
        div()
            .absolute()
            .top_0()
            .left(px(left))
            .w(px(48.))
            .flex()
            .when(i > 0 && i < n - 1, |d| d.justify_center())
            .when(align_end, |d| d.justify_end())
            .text_size(px(10.5))
            .text_color(if i == here {
                colors::text1()
            } else {
                colors::text3()
            })
            .when(i == here, |d| d.font_weight(FontWeight::SEMIBOLD))
            .child(short(level))
    });
    let body = div()
        .w(px(width))
        .px(px(15.))
        .pt(px(11.))
        .pb(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(colors::ink(0.13))
        .bg(colors::surface3())
        .shadow(colors::shadow())
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(18.))
                        .rounded(px(6.))
                        .bg(brand.opacity(0.18))
                        .text_color(brand)
                        .child(mark(pick.agent, 11., brand)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(model_label(c, &pick)),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(brand2)
                        .child(effort_label(&pick.effort)),
                ),
        )
        .child(
            div()
                .ml(px(26.))
                .mt(px(2.))
                .mb(px(14.))
                .text_size(px(11.))
                .text_color(colors::text3())
                .child(effort_note(&pick.effort)),
        )
        .child(
            div()
                .relative()
                .child(
                    div().relative().h(px(22.)).child(
                        div()
                            .absolute()
                            .left(px(9.))
                            .top(px(8.))
                            .w(px(line))
                            .h(px(6.))
                            .rounded(px(3.))
                            .bg(colors::ink(0.1))
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .h_full()
                                    .w(px(fill))
                                    .rounded(px(3.))
                                    .bg(linear_gradient(
                                        90.,
                                        linear_color_stop(brand, 0.),
                                        linear_color_stop(brand2, 1.),
                                    )),
                            )
                            .children(dots)
                            .child(
                                div()
                                    .absolute()
                                    .top(px(-6.))
                                    .left(px(fill - 9.))
                                    .size(px(18.))
                                    .rounded_full()
                                    .bg(colors::surface2())
                                    .border_3()
                                    .border_color(brand),
                            ),
                    ),
                )
                .child(
                    div()
                        .relative()
                        .h(px(16.))
                        .mt(px(6.))
                        .mx(px(9.))
                        .children(labels),
                )
                .children(hits),
        );
    let close = cx.listener(|c, _: &(), _, cx| {
        c.menu = None;
        cx.notify();
    });
    widgets::layer(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_spread_evenly_and_names_shorten() {
        assert_eq!(stop_x(0, 5, 200.), 0.);
        assert_eq!(stop_x(4, 5, 200.), 200.);
        assert_eq!(stop_x(2, 5, 200.), 100.);
        assert_eq!(stop_x(0, 1, 200.), 0.);
        assert_eq!(short("xhigh"), "X-high");
        assert_eq!(short("high"), "High");
    }
}
