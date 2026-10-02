// The model and effort menus both prompt boxes open from their chips: the composer's, for the
// next thread, and a thread's own. After T3 Code's pickers. The model menu has a rail of agents
// on the left when more than one is installed, a search box, and one two-line row per model,
// the pick marked by an accent bar. The effort menu lists the reasoning levels and, for a Claude
// model that has one, the 1M context window. Arrows move, Enter picks, Esc closes, and typing
// searches the models.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, FocusHandle, Focusable, FontWeight,
    IntoElement, KeyDownEvent, MouseMoveEvent, Pixels, ScrollHandle, SharedString, Stateful,
    Window, div, prelude::*, px,
};
use hyprspace_proto::Agent;

use super::pickers::effort_label;
use crate::assets::{icon, mark};
use crate::{colors, widgets};

pub struct Model {
    pub agent: Agent,
    pub id: String,
    pub label: String,
    pub note: Option<String>,
}

/// What the menus offer and what is picked now.
pub struct Spec {
    pub models: Vec<Model>,
    pub agent: Agent,
    pub model: String,
    pub effort: String,
    /// The levels the picked model takes, lowest first. Empty hides the effort chip.
    pub efforts: Vec<String>,
    /// What the default effort comes to for the picked model, when the catalog says.
    pub default_effort: Option<String>,
    /// Whether the picked model is on its 1M window, when it can be. None: it has one window.
    pub long: Option<bool>,
    /// A line at the bottom on what a change does.
    pub foot: Option<&'static str>,
}

pub enum Choice {
    Model(Agent, String),
    Effort(String),
    /// The 1M context window on or off.
    Long(bool),
}

/// The view a menu belongs to. Its focus handle is the prompt's, which gets the keyboard back
/// when the menu closes.
pub trait Host: Focusable + Sized + 'static {
    fn model_spec(&self) -> Option<Spec>;
    fn model_menu(&mut self) -> &mut Option<ModelMenu>;
    fn choose(&mut self, choice: Choice, cx: &mut Context<Self>);
}

/// Where a chip was last painted, so its menu opens from it.
pub type Anchor = Rc<Cell<Option<Bounds<Pixels>>>>;

/// Wraps a chip so `anchor` follows it.
pub fn anchored_chip(anchor: &Anchor, chip: impl IntoElement) -> gpui::Div {
    let anchor = anchor.clone();
    div()
        .min_w_0()
        .on_children_prepainted(move |b, _, _| anchor.set(b.first().copied()))
        .child(chip)
}

/// What the effort chip says: the level, and 1M when the long window is on.
pub fn effort_chip_label(spec: &Spec) -> String {
    let level = match (spec.effort.as_str(), &spec.default_effort) {
        ("", Some(d)) => effort_label(d),
        (e, _) => effort_label(e),
    };
    match spec.long {
        Some(true) => format!("{level} · 1M"),
        _ => level,
    }
}

/// The open menu.
pub struct ModelMenu {
    focus: FocusHandle,
    scroll: ScrollHandle,
    /// The row the arrows or the mouse are on.
    hi: usize,
    query: String,
    /// The effort menu rather than the model one.
    effort: bool,
    /// The agent the rail narrows the models to, or all of them.
    rail: Option<Agent>,
}

#[derive(Clone, PartialEq)]
enum Item {
    Model(Agent, String),
    Level(String),
    Long(bool),
}

impl ModelMenu {
    /// The model menu on the current pick, holding the keyboard.
    pub fn open(spec: &Spec, window: &mut Window, cx: &mut App) -> Self {
        Self::new(spec, false, window, cx)
    }

    /// The effort menu on the current level.
    pub fn open_effort(spec: &Spec, window: &mut Window, cx: &mut App) -> Self {
        Self::new(spec, true, window, cx)
    }

    fn new(spec: &Spec, effort: bool, window: &mut Window, cx: &mut App) -> Self {
        let mut m = Self {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            hi: 0,
            query: String::new(),
            effort,
            rail: None,
        };
        let items = m.items(spec);
        m.hi = items.iter().position(|i| picked(spec, i)).unwrap_or(0);
        window.focus(&m.focus, cx);
        m
    }

    /// Whether this is the effort menu, so the host opens it from the effort chip.
    pub fn is_effort(&self) -> bool {
        self.effort
    }

    fn items(&self, spec: &Spec) -> Vec<Item> {
        if self.effort {
            return effort_items(spec);
        }
        shown(spec, &self.query, self.rail)
            .into_iter()
            .map(|m| Item::Model(m.agent, m.id.clone()))
            .collect()
    }
}

/// The models on the rail's agent whose name (or agent) has every word of `query`.
fn shown<'a>(spec: &'a Spec, query: &str, rail: Option<Agent>) -> Vec<&'a Model> {
    let q = query.to_lowercase();
    spec.models
        .iter()
        .filter(|m| rail.is_none_or(|a| a == m.agent))
        .filter(|m| {
            let hay = format!("{} {}", m.label, m.agent.name()).to_lowercase();
            q.split_whitespace().all(|w| hay.contains(w))
        })
        .collect()
}

/// The effort menu's rows: the CLI's own default when the catalog can't say what it is, each
/// level, then the two windows when the model has both.
fn effort_items(spec: &Spec) -> Vec<Item> {
    let default = spec
        .default_effort
        .is_none()
        .then(|| Item::Level(String::new()));
    let longs = spec
        .long
        .is_some()
        .then_some([Item::Long(false), Item::Long(true)]);
    default
        .into_iter()
        .chain(spec.efforts.iter().cloned().map(Item::Level))
        .chain(longs.into_iter().flatten())
        .collect()
}

/// Whether `item` is what is picked now. A thread on its default effort shows that level picked.
fn picked(spec: &Spec, item: &Item) -> bool {
    match item {
        Item::Model(agent, id) => *agent == spec.agent && *id == base(&spec.model),
        Item::Level(level) => {
            *level == spec.effort
                || (spec.effort.is_empty() && spec.default_effort.as_ref() == Some(level))
        }
        Item::Long(long) => spec.long == Some(*long),
    }
}

/// The catalog's id for a model, without its window tag.
fn base(id: &str) -> String {
    crate::models::windowed(id, false)
}

/// The agents in the order the list has them.
fn agents(spec: &Spec) -> Vec<Agent> {
    let mut out: Vec<Agent> = Vec::new();
    for m in &spec.models {
        if !out.contains(&m.agent) {
            out.push(m.agent);
        }
    }
    out
}

fn close<H: Host>(h: &mut H, window: &mut Window, cx: &mut Context<H>) {
    *h.model_menu() = None;
    window.focus(&h.focus_handle(cx), cx);
    cx.notify();
}

fn activate<H: Host>(h: &mut H, item: Item, window: &mut Window, cx: &mut Context<H>) {
    close(h, window, cx);
    let choice = match item {
        Item::Model(agent, id) => Choice::Model(agent, id),
        Item::Level(level) => Choice::Effort(level),
        Item::Long(long) => Choice::Long(long),
    };
    h.choose(choice, cx);
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
        "escape" => {
            close(h, window, cx);
            cx.stop_propagation();
            return;
        }
        "backspace" if !m.effort => {
            m.query.pop();
            m.hi = 0;
        }
        _ => {
            let plain = !(k.modifiers.control || k.modifiers.alt || k.modifiers.platform);
            match k.key_char.as_deref() {
                Some(c) if plain && !m.effort && !c.chars().any(char::is_control) => {
                    m.query.push_str(c);
                    m.hi = 0;
                }
                _ => return,
            }
        }
    }
    m.scroll.scroll_to_item(m.hi);
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

/// A row's frame: a wash under the highlight, and the accent bar on the left for the pick.
fn row_frame(id: impl Into<gpui::ElementId>, on: bool, hi: bool) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .rounded(px(8.))
        .cursor_pointer()
        .when(hi, |d| d.bg(colors::ink(0.06)))
        .when(on, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(2.))
                    .top(px(8.))
                    .bottom(px(8.))
                    .w(px(2.))
                    .rounded_full()
                    .bg(colors::accent()),
            )
        })
}

/// A small label beside a name: a model's note, or which level is the default.
fn badge(text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(colors::ink(0.08))
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(text.into())
}

fn heading(text: &'static str) -> Div {
    div()
        .px(px(10.))
        .pt(px(6.))
        .pb(px(4.))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(text)
}

/// The open menu over its chip, or nothing before the chip has been painted once.
pub fn render<H: Host>(
    m: &ModelMenu,
    spec: &Spec,
    anchor: &Anchor,
    window: &mut Window,
    cx: &mut Context<H>,
) -> Option<AnyElement> {
    let chip = anchor.get()?;
    let body = if m.effort {
        effort_body(m, spec, cx)
    } else {
        models_body(m, spec, cx)
    };
    let foot = spec.foot.map(|line| {
        div()
            .px(px(12.))
            .py(px(8.))
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
        .w(px(if m.effort { 240. } else { 380. }))
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(colors::ink(0.13))
        .bg(colors::surface3())
        .shadow(colors::shadow())
        .overflow_hidden()
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

fn models_body<H: Host>(m: &ModelMenu, spec: &Spec, cx: &mut Context<H>) -> AnyElement {
    let all = agents(spec);
    let rail = (all.len() > 1).then(|| {
        let button = |id: &'static str, n: usize, on: bool| {
            div()
                .id((id, n))
                .flex()
                .items_center()
                .justify_center()
                .size(px(30.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(on, |d| d.bg(colors::ink(0.09)))
                .when(!on, |d| d.hover(|s| s.bg(colors::ink(0.05))))
        };
        let pick = |agent: Option<Agent>| {
            cx.listener(move |h: &mut H, _: &ClickEvent, _, cx| {
                if let Some(m) = h.model_menu() {
                    m.rail = agent;
                    m.hi = 0;
                    m.scroll.scroll_to_item(0);
                }
                cx.notify();
            })
        };
        div()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .w(px(44.))
            .py(px(8.))
            .border_r_1()
            .border_color(colors::ink(0.08))
            .child(
                button("rail-all", 0, m.rail.is_none())
                    .child(icon("layout-grid", 14., colors::text2()))
                    .on_click(pick(None)),
            )
            .children(all.iter().enumerate().map(|(n, &agent)| {
                button("rail", n, m.rail == Some(agent))
                    .child(mark(agent, 15., colors::brand(agent).0))
                    .on_click(pick(Some(agent)))
            }))
    });
    let search = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(32.))
        .mx(px(6.))
        .mt(px(6.))
        .mb(px(4.))
        .px(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(colors::border1())
        .bg(colors::ink(0.04))
        .child(icon("search", 13., colors::text3()))
        .child(if m.query.is_empty() {
            div().text_color(colors::text3()).child("Search models")
        } else {
            div().text_color(colors::text1()).child(m.query.clone())
        });
    let shown = shown(spec, &m.query, m.rail);
    let mut list = div()
        .id("model-menu-list")
        .track_scroll(&m.scroll)
        .max_h(px(340.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(1.))
        .px(px(4.))
        .pb(px(4.));
    if shown.is_empty() {
        list = list.child(
            div()
                .px(px(10.))
                .py(px(8.))
                .text_color(colors::text3())
                .child("No model matches."),
        );
    }
    for (i, model) in shown.into_iter().enumerate() {
        let item = Item::Model(model.agent, model.id.clone());
        let note = model.note.as_deref().filter(|n| short(n));
        let row = row_frame(("model-row", i), picked(spec, &item), m.hi == i)
            .py(px(6.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(model.label.clone()),
                            )
                            .children(note.map(|n| badge(n.to_string()))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .text_size(px(11.5))
                            .text_color(colors::text3())
                            .child(mark(model.agent, 11., colors::brand(model.agent).0))
                            .child(model.agent.name()),
                    ),
            );
        list = list.child(wire(row, i, item, cx));
    }
    div()
        .flex()
        .children(rail)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(search)
                .child(list),
        )
        .into_any_element()
}

fn effort_body<H: Host>(m: &ModelMenu, spec: &Spec, cx: &mut Context<H>) -> AnyElement {
    let mut body = div()
        .flex()
        .flex_col()
        .p(px(4.))
        .child(heading("Reasoning"));
    for (i, item) in effort_items(spec).into_iter().enumerate() {
        let (label, default) = match &item {
            Item::Level(l) => (effort_label(l), spec.default_effort.as_ref() == Some(l)),
            Item::Long(false) => ("Standard".to_string(), true),
            Item::Long(true) => ("1M".to_string(), false),
            Item::Model(..) => continue,
        };
        if item == Item::Long(false) {
            body = body
                .child(div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(colors::ink(0.08)))
                .child(heading("Context window"));
        }
        let row = row_frame(("effort-row", i), picked(spec, &item), m.hi == i)
            .h(px(30.))
            .child(div().child(label))
            .when(default, |d| d.child(badge("Default")));
        body = body.child(wire(row, i, item, cx));
    }
    body.into_any_element()
}

/// Whether a model's note fits beside its name as a badge. The catalog's own notes do
/// ("Fastest"); the sentences Codex's model cache carries would only show cut off.
fn short(note: &str) -> bool {
    note.chars().count() <= 14
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
            model: "claude-opus-5-5[1m]".into(),
            effort: String::new(),
            efforts: vec!["low".into(), "high".into()],
            default_effort: Some("high".into()),
            long: Some(true),
            foot: None,
        }
    }

    #[test]
    fn the_rail_and_the_search_narrow_the_models() {
        let s = spec();
        assert_eq!(shown(&s, "", None).len(), 4);
        assert_eq!(shown(&s, "", Some(Agent::Codex)).len(), 1);
        assert_eq!(shown(&s, "OPUS", None)[0].id, "claude-opus-5-5");
        assert!(shown(&s, "opus", Some(Agent::Codex)).is_empty());
        assert_eq!(agents(&s), [Agent::Claude, Agent::Codex]);
    }

    #[test]
    fn a_long_window_model_is_still_the_pick() {
        let s = spec();
        assert!(picked(
            &s,
            &Item::Model(Agent::Claude, "claude-opus-5-5".into())
        ));
        assert!(picked(&s, &Item::Long(true)));
    }

    #[test]
    fn effort_lists_levels_then_windows_and_marks_the_default() {
        let s = spec();
        let items = effort_items(&s);
        assert!(
            items
                == [
                    Item::Level("low".into()),
                    Item::Level("high".into()),
                    Item::Long(false),
                    Item::Long(true),
                ]
        );
        // on the default effort, the level it comes to is the pick
        assert!(picked(&s, &Item::Level("high".into())));
        assert_eq!(effort_chip_label(&s), "High · 1M");
        let unknown = Spec {
            default_effort: None,
            long: None,
            ..spec()
        };
        assert!(effort_items(&unknown)[0] == Item::Level(String::new()));
        assert_eq!(effort_items(&unknown).len(), 3);
    }
}
