// The Settings screen, laid out like zeron's: it takes the whole window, with a plain list of
// views on the left and the open view on the right in one centered column. Views edit
// `Root::state` in place and save through `Root::save`, the same path the composer's picks take.
// A new view is one more `Tab` and one row in `TABS`; the palette lists every row too.

mod about;
mod agents;
mod appearance;
mod controls;
mod general;
mod phone;
mod picker;
mod shortcuts;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, FocusHandle, FontWeight, IntoElement,
    KeyDownEvent, Pixels, Point, SharedString, Window, div, prelude::*, px,
};
use hyprspace_proto::Agent;
use hyprspace_proto::state::ComposerPrefs;

pub(crate) use appearance::{set_terminal_font, terminal_font, terminal_line_height, ui_font};

use crate::assets::icon;
use crate::root::Root;
use crate::{colors, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    General,
    Appearance,
    Agents,
    Usage,
    Phone,
    Skills,
    Shortcuts,
    About,
}

/// A dropdown open over a view.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Picker {
    Agent,
    Permission,
    Opener,
    Font,
    UiFont,
    Model(Agent),
    Effort(Agent),
}

/// What the screen remembers while the app runs: the open view and any open dropdown.
pub struct Settings {
    tab: Tab,
    focus: FocusHandle,
    menu: Option<(Point<Pixels>, Picker)>,
    /// Where each dropdown's field was last painted, so its menu drops from the field's edge
    /// like a select instead of from wherever the click landed.
    fields: Rc<RefCell<HashMap<Picker, Bounds<Pixels>>>>,
    /// The installed monospace families, read once when the Font menu first opens.
    fonts: RefCell<Option<Vec<String>>>,
}

impl Settings {
    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.menu = None;
    }

    pub(crate) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub fn new(cx: &mut App) -> Self {
        Self {
            tab: Tab::General,
            focus: cx.focus_handle(),
            menu: None,
            fields: Rc::default(),
            fonts: RefCell::default(),
        }
    }
}

pub(crate) struct Entry {
    pub(crate) tab: Tab,
    pub(crate) label: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) icon: &'static str,
}

/// Every view in nav order. The palette lists them too.
pub(crate) const TABS: &[Entry] = &[
    Entry {
        tab: Tab::General,
        label: "General",
        desc: "How new threads start and where folders open",
        icon: "sliders-horizontal",
    },
    Entry {
        tab: Tab::Appearance,
        label: "Appearance",
        desc: "The theme, light or dark, fonts and motion",
        icon: "palette",
    },
    Entry {
        tab: Tab::Agents,
        label: "Agents",
        desc: "The model and effort each agent starts with",
        icon: "bot",
    },
    Entry {
        tab: Tab::Usage,
        label: "Usage",
        desc: "What each agent has used",
        icon: "gauge",
    },
    Entry {
        tab: Tab::Phone,
        label: "Phone",
        desc: "Your threads on your phone",
        icon: "smartphone",
    },
    Entry {
        tab: Tab::Skills,
        label: "Skills",
        desc: "Reusable instructions for Claude",
        icon: "zap",
    },
    Entry {
        tab: Tab::Shortcuts,
        label: "Shortcuts",
        desc: "The keys and clicks the app answers to",
        icon: "keyboard",
    },
    Entry {
        tab: Tab::About,
        label: "About",
        desc: "The version, updates and where the code lives",
        icon: "info",
    },
];

impl Root {
    pub(crate) fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let i = TABS
            .iter()
            .position(|e| e.tab == self.settings.tab)
            .unwrap_or(0);
        let entry = &TABS[i];
        let body = match self.settings.tab {
            Tab::General => self.general(cx),
            Tab::Appearance => self.appearance(cx),
            Tab::Agents => self.agents_page(cx),
            Tab::Usage => self.usage_page(cx),
            Tab::Phone => self.phone_page(cx),
            Tab::Skills => self.skills_page(window, cx),
            Tab::Shortcuts => shortcuts::page(),
            Tab::About => self.about(cx),
        };
        let menu = self.picker(window, cx);
        div()
            .id("settings")
            .track_focus(&self.settings.focus)
            .on_key_down(cx.listener(|r, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    if r.settings.menu.take().is_none() {
                        r.close_settings(window, cx);
                    }
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .child(self.settings_nav(cx))
            .child(
                div()
                    .id("settings-page")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    // each page settles a few pixels down into place as it fades in
                    .child(crate::slide::ease_in(
                        div()
                            .relative()
                            .child(controls::page(entry.label, entry.desc, body)),
                        ("settings-tab", i),
                        220,
                        |d, t| d.opacity(t).top(px(-6. * (1. - t))),
                    )),
            )
            .children(menu)
            .into_any_element()
    }

    fn settings_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = TABS.iter().enumerate().map(|(i, e)| {
            let on = e.tab == self.settings.tab;
            let tab = e.tab;
            div()
                .id(("settings-tab", i))
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(32.))
                .px(px(10.))
                .rounded(px(7.))
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |d| {
                    d.bg(colors::ink(0.07))
                        .text_color(colors::text1())
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(!on, |d| {
                    d.text_color(colors::text2())
                        .hover(|s| s.bg(colors::ink(0.04)).text_color(colors::text1()))
                })
                .child(icon(
                    e.icon,
                    15.,
                    if on { colors::text1() } else { colors::text3() },
                ))
                .child(e.label)
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    r.settings.set_tab(tab);
                    cx.notify();
                }))
        });
        let back = div()
            .id("settings-back")
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(10.))
            .rounded(px(7.))
            .text_size(px(13.))
            .text_color(colors::text2())
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.04)).text_color(colors::text1()))
            .child(icon("arrow-left", 15., colors::text3()))
            .child(div().flex_1().child("Back"))
            .child(widgets::keycap("Esc"))
            .on_click(cx.listener(|r, _: &ClickEvent, window, cx| r.close_settings(window, cx)));
        div()
            .flex_none()
            .w(px(220.))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .border_r_1()
            .border_color(colors::border1())
            .bg(colors::surface1())
            .child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(6.))
                    .text_size(px(12.))
                    .text_color(colors::text3())
                    .child("Settings"),
            )
            .children(items)
            .child(div().flex_1())
            .child(back)
            .into_any_element()
    }

    /// A dropdown's face that opens `picker`. Its bounds are kept so the menu can open right
    /// under it.
    fn dropdown(
        &self,
        picker: Picker,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fields = self.settings.fields.clone();
        let id = SharedString::from(format!("set-pick-{}", picker_key(picker)));
        div()
            .on_children_prepainted(move |b, _, _| {
                if let Some(b) = b.first() {
                    fields.borrow_mut().insert(picker, *b);
                }
            })
            .child(
                widgets::select(id, label)
                    .min_w(px(168.))
                    .max_w(px(240.))
                    .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                        r.settings.menu = Some((e.position(), picker));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// Changes the composer's saved picks, saves them, and hands them to the composer so the next
    /// thread starts with them.
    fn update_prefs(&mut self, f: impl FnOnce(&mut ComposerPrefs), cx: &mut Context<Self>) {
        f(&mut self.state.composer);
        self.save();
        let prefs = self.state.composer.clone();
        self.composer.update(cx, |c, cx| c.set_prefs(prefs, cx));
        cx.notify();
    }
}

fn picker_key(p: Picker) -> String {
    match p {
        Picker::Agent => "agent".into(),
        Picker::Permission => "permission".into(),
        Picker::Opener => "opener".into(),
        Picker::Font => "font".into(),
        Picker::UiFont => "ui-font".into(),
        Picker::Model(a) => format!("model-{}", a.cli()),
        Picker::Effort(a) => format!("effort-{}", a.cli()),
    }
}
