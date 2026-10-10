// The folder under the box, as a menu: the projects on the same computer to start in, and a way
// to open another folder.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, Pixels, Point, Window, div, prelude::*, px,
};

use super::{Composer, ComposerEvent};
use crate::assets::icon;
use crate::{colors, widgets};

#[derive(Clone, PartialEq)]
pub struct Place {
    pub space: u64,
    pub name: String,
}

impl Composer {
    pub fn set_places(&mut self, places: Vec<Place>, cx: &mut Context<Self>) {
        if self.places != places {
            self.places = places;
            cx.notify();
        }
    }

    pub(super) fn open_places(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.menu = None;
        self.machine_menu = None;
        self.places_menu = Some(e.position());
        cx.notify();
    }
}

pub fn menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let here = c.target.as_ref().map(|t| t.space);
    let rows: Vec<AnyElement> = c
        .places
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let space = p.space;
            div()
                .id(("place", i))
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(Some(space) == here, |d| d.bg(colors::ink(0.07)))
                .hover(|s| s.bg(colors::ink(0.05)))
                .child(crate::sidebar::tag(&p.name, false))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(colors::text1())
                        .child(p.name.clone()),
                )
                .when(Some(space) == here, |d| {
                    d.child(icon("check", 13., colors::text2()))
                })
                .on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                    c.places_menu = None;
                    cx.emit(ComposerEvent::Space(space));
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect();
    let other = div()
        .id("place-other")
        .flex()
        .items_center()
        .gap(px(9.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.05)))
        .child(icon("folder-open", 14., colors::text2()))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(colors::text1())
                .child("Open another folder..."),
        )
        .on_click(cx.listener(|c, _: &ClickEvent, _, cx| {
            c.places_menu = None;
            cx.emit(ComposerEvent::OtherFolder);
            cx.notify();
        }));
    let body = div()
        .w(px(260.))
        .flex()
        .flex_col()
        .when(!rows.is_empty(), |d| {
            d.child(
                div()
                    .id("places")
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .children(rows),
            )
            .child(widgets::menu_rule())
        })
        .child(other)
        .into_any_element();
    let close = cx.listener(|c, _: &(), _, cx| {
        c.places_menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Down,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}
