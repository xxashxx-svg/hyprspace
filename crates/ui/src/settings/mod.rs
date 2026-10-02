// The Settings screen. It takes the whole window: a nav column on the left, the open view on the
// right under its title, both in one centered column so their edges line up. Views edit
// `Root::state` in place and save through `Root::save`, the same path the composer's picks take.
// A new view is one more `Tab` and one row in `TABS`.

mod appearance;
mod defaults;
mod general;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FocusHandle, FontWeight, IntoElement, KeyDownEvent,
    Pixels, Point, SharedString, Window, div, prelude::*, px,
};
use hyprspace_proto::Agent;
use hyprspace_proto::state::ComposerPrefs;

use crate::assets::icon;
use crate::root::Root;
use crate::{colors, widgets};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Appearance,
    Defaults,
    Usage,
    Skills,
}

/// A picker open over the Defaults view.
#[derive(Clone, Copy)]
enum Picker {
    Model(Agent),
    Effort(Agent),
}

/// What the screen remembers while the app runs: the open view and any open picker.
pub struct Settings {
    tab: Tab,
    focus: FocusHandle,
    menu: Option<(Point<Pixels>, Picker)>,
}

impl Settings {
    pub fn new(cx: &mut App) -> Self {
        Self {
            tab: Tab::General,
            focus: cx.focus_handle(),
            menu: None,
        }
    }
}

struct Entry {
    tab: Tab,
    group: &'static str,
    label: &'static str,
    desc: &'static str,
    icon: &'static str,
}

const TABS: &[Entry] = &[
    Entry {
        tab: Tab::General,
        group: "App",
        label: "General",
        desc: "The version, how sessions start, and where your data lives",
        icon: "sliders-horizontal",
    },
    Entry {
        tab: Tab::Appearance,
        group: "App",
        label: "Appearance",
        desc: "The theme, and whether it is light or dark",
        icon: "palette",
    },
    Entry {
        tab: Tab::Defaults,
        group: "Agents",
        label: "Defaults",
        desc: "What each agent starts with: model, effort and permission",
        icon: "bot",
    },
    Entry {
        tab: Tab::Usage,
        group: "Agents",
        label: "Usage",
        desc: "What each agent has used",
        icon: "gauge",
    },
    Entry {
        tab: Tab::Skills,
        group: "Agents",
        label: "Skills",
        desc: "Reusable instructions for Claude",
        icon: "zap",
    },
];

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl Root {
    pub(crate) fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Esc closes the screen, so it holds focus whenever it is up
        if !self.settings.focus.is_focused(window) {
            window.focus(&self.settings.focus, cx);
        }
        let entry = TABS
            .iter()
            .find(|e| e.tab == self.settings.tab)
            .unwrap_or(&TABS[0]);
        let page = match self.settings.tab {
            Tab::General => self.general(cx),
            Tab::Appearance => self.appearance(cx),
            Tab::Defaults => self.defaults(cx),
            Tab::Usage => self.usage_page(cx),
            Tab::Skills => self.skills_page(window, cx),
        };
        let header = div()
            .flex_none()
            .pt(px(26.))
            .pb(px(18.))
            .border_b_1()
            .border_color(colors::border1())
            .child(column(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child(entry.label),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(colors::text3())
                            .child(entry.desc),
                    ),
            ));
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
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(
                        div()
                            .id("settings-page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .pt(px(22.))
                            .pb(px(48.))
                            .child(column(page)),
                    ),
            )
            .children(menu)
            .into_any_element()
    }

    fn settings_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div()
            .flex_none()
            .w(px(224.))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(1.))
            .p(px(10.))
            .border_r_1()
            .border_color(colors::border1())
            .bg(colors::surface1())
            .child(
                div()
                    .px(px(9.))
                    .pt(px(6.))
                    .pb(px(4.))
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child("Settings"),
            );
        let mut group = "";
        for (i, e) in TABS.iter().enumerate() {
            if e.group != group {
                group = e.group;
                col = col.child(label(group).px(px(9.)).pt(px(14.)).pb(px(5.)));
            }
            let on = e.tab == self.settings.tab;
            let tab = e.tab;
            col = col.child(
                div()
                    .id(("settings-tab", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(32.))
                    .pl(px(9.))
                    .pr(px(10.))
                    .rounded(px(7.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(on, |d| {
                        d.bg(colors::accent().opacity(0.12))
                            .text_color(colors::text1())
                    })
                    .when(!on, |d| {
                        d.text_color(colors::text2())
                            .hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
                    })
                    .child(icon(
                        e.icon,
                        16.,
                        if on {
                            colors::accent()
                        } else {
                            colors::text3()
                        },
                    ))
                    .child(e.label)
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.settings.tab = tab;
                        r.settings.menu = None;
                        cx.notify();
                    })),
            );
        }
        col.child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pt(px(8.))
                    .border_t_1()
                    .border_color(colors::border0())
                    .child(
                        div()
                            .id("settings-back")
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(30.))
                            .pl(px(9.))
                            .pr(px(8.))
                            .rounded(px(7.))
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::text2())
                            .cursor_pointer()
                            .hover(|s| s.bg(colors::ink(0.05)).text_color(colors::text1()))
                            .child(icon("arrow-left", 15., colors::text3()))
                            .child(div().flex_1().child("Back to app"))
                            .child(widgets::keycap("Esc"))
                            .on_click(cx.listener(|r, _: &ClickEvent, window, cx| {
                                r.close_settings(window, cx)
                            })),
                    )
                    .child(
                        div()
                            .px(px(9.))
                            .py(px(5.))
                            .text_size(px(11.5))
                            .text_color(colors::text3())
                            .child(format!("HyprSpace v{VERSION}")),
                    ),
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

/// The centered column the header and the page share.
fn column(content: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .justify_center()
        .px(px(32.))
        .child(div().w_full().max_w(px(760.)).child(content))
}

/// A quiet upper-case label over a group (settings.css `.set-label`).
fn label(text: &str) -> Div {
    div()
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text3())
        .child(text.to_uppercase())
}

/// A labeled section of a page.
fn section(title: &str, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(label(title).px(px(2.)))
        .child(body)
}

/// Rows in one framed group, with hairlines between them (settings.css `.set-group`).
fn group(title: &str, rows: Vec<AnyElement>) -> Div {
    let rows = rows.into_iter().enumerate().map(|(i, r)| {
        div()
            .when(i > 0, |d| d.border_t_1().border_color(colors::border1()))
            .child(r)
    });
    section(
        title,
        div()
            .px(px(16.))
            .rounded(px(10.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface2())
            .children(rows),
    )
}

/// One setting: its name and a line on what it does, with the control on the right.
fn row(key: &str, desc: impl Into<SharedString>, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(24.))
        .py(px(13.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(colors::text1())
                        .child(key.to_string()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(desc.into()),
                ),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}
