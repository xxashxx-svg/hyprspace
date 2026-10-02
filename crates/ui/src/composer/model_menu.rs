// The model picker both prompt boxes open from their model chip: the composer's, for the next
// thread, and a thread's own. After zeron's compact picker: a small menu that opens upward from
// the chip, one line per model with the agent's mark and a check on the pick, and the effort
// levels one page down. Arrows move, Enter picks, Esc closes, and typing filters a long list.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, FocusHandle, Focusable, FontWeight,
    IntoElement, KeyDownEvent, MouseMoveEvent, Pixels, ScrollHandle, Stateful, Window, div,
    prelude::*, px,
};
use hyprspace_proto::Agent;

use super::pickers::{effort_label, effort_note};
use crate::assets::{icon, mark};
use crate::{colors, widgets};

/// Past this many models, typing filters the list.
const FILTER_AT: usize = 8;

pub struct Model {
    pub agent: Agent,
    pub id: String,
    pub label: String,
    pub note: Option<String>,
}

/// What the menu offers and what is picked now.
pub struct Spec {
    pub models: Vec<Model>,
    pub agent: Agent,
    pub model: String,
    pub effort: String,
    /// The levels the picked model takes, lowest first. Empty hides the effort row.
    pub efforts: Vec<String>,
    /// What the default effort comes to for the picked model, when the catalog says.
    pub default_effort: Option<String>,
    /// A line at the bottom on what a change does.
    pub foot: Option<&'static str>,
}

pub enum Choice {
    Model(Agent, String),
    Effort(String),
}

/// The view a menu belongs to. Its focus handle is the prompt's, which gets the keyboard back
/// when the menu closes.
pub trait Host: Focusable + Sized + 'static {
    fn model_spec(&self) -> Option<Spec>;
    fn model_menu(&mut self) -> &mut Option<ModelMenu>;
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>);
}

/// Where the chip was last painted, so the menu opens from the chip.
pub type Anchor = Rc<Cell<Option<Bounds<Pixels>>>>;

/// Wraps a chip so `anchor` follows it.
pub fn anchored_chip(anchor: &Anchor, chip: impl IntoElement) -> gpui::Div {
    let anchor = anchor.clone();
    div()
        .min_w_0()
        .on_children_prepainted(move |b, _, _| anchor.set(b.first().copied()))
        .child(chip)
}

/// The open menu.
pub struct ModelMenu {
    focus: FocusHandle,
    scroll: ScrollHandle,
    /// The row the arrows or the mouse are on.
    hi: usize,
    query: String,
    /// Showing the effort levels instead of the models.
    efforts: bool,
}

#[derive(Clone, PartialEq)]
enum Item {
    Model(Agent, String),
    /// The row that opens the effort levels.
    Effort,
    Level(String),
}

impl ModelMenu {
    /// A menu on the current pick, holding the keyboard.
    pub fn open(spec: &Spec, window: &mut Window, cx: &mut App) -> Self {
        let mut m = Self {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            hi: 0,
            query: String::new(),
            efforts: false,
        };
        let current = Item::Model(spec.agent, spec.model.clone());
        m.hi = m
            .items(spec)
            .iter()
            .position(|i| *i == current)
            .unwrap_or(0);
        window.focus(&m.focus, cx);
        m
    }

    fn filtering(&self, spec: &Spec) -> bool {
        !self.efforts && spec.models.len() > FILTER_AT
    }

    fn items(&self, spec: &Spec) -> Vec<Item> {
        items(spec, &self.query, self.efforts)
    }

    fn page(&mut self, efforts: bool, spec: &Spec) {
        self.efforts = efforts;
        self.query.clear();
        let back = if efforts {
            Item::Level(spec.effort.clone())
        } else {
            Item::Effort
        };
        self.hi = self
            .items(spec)
            .iter()
            .position(|i| *i == back)
            .unwrap_or(0);
        self.scroll.scroll_to_item(0);
    }

    /// Keeps the highlighted model in view. The list's children are the models and, with more
    /// than one agent, a label before each agent's first, so the child index counts both.
    fn reveal(&self, spec: &Spec) {
        if self.efforts {
            self.scroll.scroll_to_item(self.hi);
            return;
        }
        let grouped = several_agents(spec);
        let mut child = 0;
        let mut last = None;
        for (i, m) in shown(spec, &self.query).into_iter().enumerate() {
            if grouped && last != Some(m.agent) {
                last = Some(m.agent);
                child += 1;
            }
            if i == self.hi {
                self.scroll.scroll_to_item(child);
                return;
            }
            child += 1;
        }
    }
}

/// The models whose name (or agent) has every word of `query`.
fn shown<'a>(spec: &'a Spec, query: &str) -> Vec<&'a Model> {
    let q = query.to_lowercase();
    spec.models
        .iter()
        .filter(|m| {
            let hay = format!("{} {}", m.label, m.agent.name()).to_lowercase();
            q.split_whitespace().all(|w| hay.contains(w))
        })
        .collect()
}

/// Everything the arrows can land on, in order.
fn items(spec: &Spec, query: &str, efforts: bool) -> Vec<Item> {
    if efforts {
        return levels(spec).into_iter().map(Item::Level).collect();
    }
    let mut items: Vec<Item> = shown(spec, query)
        .into_iter()
        .map(|m| Item::Model(m.agent, m.id.clone()))
        .collect();
    if !spec.efforts.is_empty() && query.is_empty() {
        items.push(Item::Effort);
    }
    items
}

/// The levels on the effort page: the CLI's default first.
fn levels(spec: &Spec) -> Vec<String> {
    std::iter::once(String::new())
        .chain(spec.efforts.iter().cloned())
        .collect()
}

fn several_agents(spec: &Spec) -> bool {
    spec.models.iter().any(|m| m.agent != spec.models[0].agent)
}

fn close<H: Host>(h: &mut H, window: &mut Window, cx: &mut Context<H>) {
    *h.model_menu() = None;
    window.focus(&h.focus_handle(cx), cx);
    cx.notify();
}

fn activate<H: Host>(h: &mut H, item: Item, window: &mut Window, cx: &mut Context<H>) {
    match item {
        Item::Model(agent, id) => {
            close(h, window, cx);
            h.choose(Choice::Model(agent, id), cx);
        }
        Item::Level(level) => {
            close(h, window, cx);
            h.choose(Choice::Effort(level), cx);
        }
        Item::Effort => {
            if let Some(spec) = h.model_spec()
                && let Some(m) = h.model_menu()
            {
                m.page(true, &spec);
            }
            cx.notify();
        }
    }
}

fn key<H: Host>(h: &mut H, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<H>) {
    let Some(spec) = h.model_spec() else {
        return;
    };
    let Some(m) = h.model_menu() else {
        return;
    };
    let items = m.items(&spec);
    let k = &e.keystroke;
    match k.key.as_str() {
        "up" => m.hi = m.hi.saturating_sub(1),
        "down" => m.hi = (m.hi + 1).min(items.len().saturating_sub(1)),
        "enter" => {
            if let Some(item) = items.get(m.hi).cloned() {
                activate(h, item, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        "escape" | "left" if m.efforts => m.page(false, &spec),
        "escape" => {
            close(h, window, cx);
            cx.stop_propagation();
            return;
        }
        "right" if items.get(m.hi) == Some(&Item::Effort) => m.page(true, &spec),
        "backspace" if m.filtering(&spec) => {
            m.query.pop();
            m.hi = 0;
        }
        _ => {
            let plain = !(k.modifiers.control || k.modifiers.alt || k.modifiers.platform);
            match k.key_char.as_deref() {
                Some(c) if plain && m.filtering(&spec) && !c.chars().any(char::is_control) => {
                    m.query.push_str(c);
                    m.hi = 0;
                }
                _ => return,
            }
        }
    }
    m.reveal(&spec);
    cx.stop_propagation();
    cx.notify();
}

/// Hooks a row up: the mouse moves the highlight onto it, a click picks it.
fn wire<H: Host>(row: Stateful<Div>, i: usize, item: Item, cx: &mut Context<H>) -> Stateful<Div> {
    row.on_mouse_move(cx.listener(move |h: &mut H, _: &MouseMoveEvent, _, cx| {
        if let Some(m) = h.model_menu()
            && m.hi != i
        {
            m.hi = i;
            cx.notify();
        }
    }))
    .on_click(cx.listener(move |h: &mut H, _: &ClickEvent, window, cx| {
        activate(h, item.clone(), window, cx)
    }))
}

/// The menu over its chip, or nothing before the chip has been painted once.
pub fn render<H: Host>(
    m: &ModelMenu,
    spec: &Spec,
    anchor: &Anchor,
    window: &mut Window,
    cx: &mut Context<H>,
) -> Option<AnyElement> {
    let chip = anchor.get()?;
    let mut body = div().flex().flex_col();
    let mut list = div()
        .id("model-menu-list")
        .track_scroll(&m.scroll)
        .max_h(px(320.))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    if m.efforts {
        body = body.child(
            div()
                .id("model-menu-back")
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(30.))
                .px(px(8.))
                .rounded(px(7.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.05)))
                .child(icon("arrow-left", 13., colors::text3()))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text2())
                        .child("Effort"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .justify_end()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(div().truncate().child(picked_label(spec))),
                )
                .on_click(cx.listener(|h: &mut H, _: &ClickEvent, _, cx| {
                    if let Some(spec) = h.model_spec()
                        && let Some(m) = h.model_menu()
                    {
                        m.page(false, &spec);
                    }
                    cx.notify();
                })),
        );
        for (i, level) in levels(spec).into_iter().enumerate() {
            let (label, note) = match (level.as_str(), &spec.default_effort) {
                ("", Some(d)) => (format!("Default · {}", effort_label(d)), None),
                (l, _) => (effort_label(l), Some(effort_note(l).into())),
            };
            let checked = level == spec.effort;
            let row = widgets::pick_row(
                ("effort-level", i),
                None,
                label,
                note,
                checked.then(|| icon("check", 13., colors::text2()).into_any_element()),
                m.hi == i,
            );
            list = list.child(wire(row, i, Item::Level(level), cx));
        }
        body = body.child(list);
    } else {
        if m.filtering(spec) {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(8.))
                    .mb(px(2.))
                    .border_b_1()
                    .border_color(colors::ink(0.08))
                    .child(icon("search", 13., colors::text3()))
                    .child(if m.query.is_empty() {
                        div().text_color(colors::text3()).child("Type to filter")
                    } else {
                        div().text_color(colors::text1()).child(m.query.clone())
                    }),
            );
        }
        let grouped = several_agents(spec);
        let shown = shown(spec, &m.query);
        if shown.is_empty() {
            list = list.child(
                div()
                    .px(px(8.))
                    .py(px(7.))
                    .text_color(colors::text3())
                    .child("No model matches."),
            );
        }
        let mut last = None;
        for (i, model) in shown.into_iter().enumerate() {
            if grouped && last != Some(model.agent) {
                list = list.child(
                    div()
                        .px(px(8.))
                        .pt(px(if last.is_none() { 4. } else { 10. }))
                        .pb(px(3.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text3())
                        .child(model.agent.name()),
                );
                last = Some(model.agent);
            }
            let checked = model.agent == spec.agent && model.id == spec.model;
            let item = Item::Model(model.agent, model.id.clone());
            let row = widgets::pick_row(
                ("model-row", i),
                Some(mark(model.agent, 14., colors::brand(model.agent).0).into_any_element()),
                model.label.clone(),
                model
                    .note
                    .as_deref()
                    .filter(|n| short(n))
                    .map(|n| n.to_string().into()),
                checked.then(|| icon("check", 13., colors::text2()).into_any_element()),
                m.hi == i,
            );
            list = list.child(wire(row, i, item, cx));
        }
        body = body.child(list);
        let items = m.items(spec);
        if items.last() == Some(&Item::Effort) {
            let i = items.len() - 1;
            let row = widgets::pick_row(
                "model-effort",
                Some(icon("gauge", 14., colors::text3()).into_any_element()),
                "Effort",
                Some(effort_label(&spec.effort).into()),
                Some(icon("chevron-right", 12., colors::text3()).into_any_element()),
                m.hi == i,
            );
            body = body
                .child(div().h(px(1.)).mx(px(4.)).my(px(4.)).bg(colors::ink(0.08)))
                .child(wire(row, i, Item::Effort, cx));
        }
    }
    let foot = spec.foot.map(|line| {
        div()
            .mt(px(4.))
            .px(px(8.))
            .pt(px(7.))
            .pb(px(4.))
            .border_t_1()
            .border_color(colors::ink(0.08))
            .text_size(px(11.))
            .text_color(colors::text3())
            .child(line)
    });
    let frame = div()
        .id("model-menu")
        .track_focus(&m.focus)
        .on_key_down(cx.listener(key::<H>))
        .w(px(260.))
        .p(px(4.))
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(colors::ink(0.13))
        .bg(colors::surface3())
        .shadow(colors::shadow())
        .text_size(px(12.5))
        .text_color(colors::text1())
        .child(body)
        .children(foot);
    let dismiss = cx.listener(|h: &mut H, _: &(), window, cx| close(h, window, cx));
    Some(widgets::above(
        chip,
        window,
        move |w, cx| dismiss(&(), w, cx),
        frame,
    ))
}

/// Whether a model's note fits beside its name. The catalog's own notes do ("Fastest"); the
/// sentences Codex's model cache carries would only show as a cut-off fragment.
fn short(note: &str) -> bool {
    note.chars().count() <= 14
}

/// The picked model's label as the menu lists it.
fn picked_label(spec: &Spec) -> String {
    spec.models
        .iter()
        .find(|m| m.agent == spec.agent && m.id == spec.model)
        .map(|m| m.label.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        let m = |agent, id: &str, label: &str| Model {
            agent,
            id: id.into(),
            label: label.into(),
            note: None,
        };
        Spec {
            models: vec![
                m(Agent::Claude, "", "Default"),
                m(Agent::Claude, "claude-opus-5-5", "Opus 5.5"),
                m(Agent::Claude, "claude-haiku-4-5", "Haiku 4.5"),
                m(Agent::Codex, "gpt-5.5", "GPT-5.5"),
            ],
            agent: Agent::Claude,
            model: "claude-haiku-4-5".into(),
            effort: "high".into(),
            efforts: vec!["low".into(), "high".into()],
            default_effort: None,
            foot: None,
        }
    }

    #[test]
    fn effort_sits_after_the_models_and_lists_the_default_first() {
        let s = spec();
        let all = items(&s, "", false);
        assert_eq!(all.len(), 5);
        assert!(all[4] == Item::Effort);
        let levels = items(&s, "", true);
        assert!(levels[0] == Item::Level(String::new()));
        assert!(levels[2] == Item::Level("high".into()));
        assert!(several_agents(&s));
    }

    #[test]
    fn typing_narrows_the_models_and_hides_effort() {
        let s = spec();
        assert!(items(&s, "codex", false) == vec![Item::Model(Agent::Codex, "gpt-5.5".into())]);
        assert!(
            items(&s, "OPUS", false) == vec![Item::Model(Agent::Claude, "claude-opus-5-5".into())]
        );
        assert!(items(&s, "nothing", false).is_empty());
    }
}
