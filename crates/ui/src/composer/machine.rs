// Which computer a new thread runs on: this one, or a paired one that is connected. Its chip sits
// under the box beside the folder, and only shows once a computer is paired.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, Pixels, Point, Window, div,
    prelude::*, px,
};

use super::{Composer, ComposerEvent};
use crate::assets::icon;
use crate::{colors, widgets};

#[derive(Clone, PartialEq)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub online: bool,
}

impl Composer {
    pub fn set_machines(&mut self, machines: Vec<Machine>, cx: &mut Context<Self>) {
        if self.machines != machines {
            self.machines = machines;
            cx.notify();
        }
    }

    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.error = Some(message);
        cx.notify();
    }

    fn machine_name(&self) -> String {
        match self.target.as_ref().and_then(|t| t.machine.as_deref()) {
            Some(id) => self
                .machines
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            None => "This computer".into(),
        }
    }

    fn open_machines(&mut self, e: &ClickEvent, cx: &mut Context<Self>) {
        self.menu = None;
        self.machine_menu = Some(e.position());
        cx.notify();
    }
}

pub fn chip(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.machines.is_empty() {
        return None;
    }
    Some(
        div()
            .id("composer-machine")
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(24.))
            .px(px(6.))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(colors::text2())
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
            .child(icon("monitor", 13., colors::text3()))
            .child(div().max_w(px(160.)).truncate().child(c.machine_name()))
            .child(icon("chevron-down", 12., colors::text3()))
            .on_click(cx.listener(|c, e: &ClickEvent, _, cx| c.open_machines(e, cx)))
            .into_any_element(),
    )
}

pub fn menu(
    c: &Composer,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<Composer>,
) -> AnyElement {
    let here = c.target.as_ref().and_then(|t| t.machine.clone());
    let row = |ix: usize, id: Option<String>, name: String, online: bool| {
        let on = id == here;
        div()
            .id(("machine", ix))
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(8.))
            .when(on, |d| d.bg(colors::ink(0.07)))
            .when(online, |d| {
                d.cursor_pointer().hover(|s| s.bg(colors::ink(0.05)))
            })
            .child(icon("monitor", 14., colors::text2()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if online {
                        colors::text1()
                    } else {
                        colors::text3()
                    })
                    .child(name),
            )
            .when(!online, |d| {
                d.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("Offline"),
                )
            })
            .when(online, |d| {
                d.on_click(cx.listener(move |c, _: &ClickEvent, _, cx| {
                    c.machine_menu = None;
                    cx.emit(ComposerEvent::Machine(id.clone()));
                    cx.notify();
                }))
            })
    };
    let mut rows = vec![row(0, None, "This computer".into(), true)];
    for (i, m) in c.machines.iter().enumerate() {
        rows.push(row(i + 1, Some(m.id.clone()), m.name.clone(), m.online));
    }
    let body = div()
        .w(px(240.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .children(rows)
        .into_any_element();
    let close = cx.listener(|c, _: &(), _, cx| {
        c.machine_menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        body,
    )
}
