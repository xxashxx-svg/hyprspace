// The model and effort menus both prompt boxes open from their chips: the composer's, for the
// next thread, and a thread's own. After T3 Code's pickers. The model menu has a rail of agents
// on the left when more than one is installed, a search box, and the models grouped under their
// agent with the pick ticked. The highlight glides between rows, the list eases along under
// the wheel as the sidebar does, and the menu eases in. Arrows move, Enter picks, Esc closes,
// and typing searches the models. The effort menu is a slider, in effort.rs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, DispatchPhase, Div, FocusHandle, Focusable,
    FontWeight, HitboxBehavior, IntoElement, KeyDownEvent, MouseMoveEvent, Pixels, ScrollHandle,
    ScrollWheelEvent, SharedString, Stateful, Window, canvas, div, point, prelude::*, px,
};
use hyprspace_proto::Agent;

use super::effort::{self, Slider};
use super::pickers::effort_label;
use crate::assets::{icon, mark};
use crate::slide::Glide;
use crate::{colors, widgets};

/// Heights in the model list, which place the gliding highlight.
const HEADING: f32 = 26.;
const ROW: f32 = 32.;
/// Where the rail's first button sits, and the step to the next.
const RAIL_TOP: f32 = 8.;
const RAIL_STEP: f32 = 34.;
/// About how tall each menu opens, to tell whether it fits under its chip.
const MODELS_TALL: f32 = 340.;
const EFFORT_TALL: f32 = 200.;
/// The wheel's easing, the sidebar's: a line's travel, and the time constant it settles in.
const LINE: f32 = 20.;
const EASE: f32 = 0.06;

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

/// Counts openings, so each one eases in afresh.
static OPENED: AtomicUsize = AtomicUsize::new(0);

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
    /// This opening's number, so each opening eases in afresh.
    opened: usize,
    glide: Glide,
    rail_glide: Glide,
    /// Each model's place among the list's children, which the agent headings shift.
    rows: RefCell<Vec<usize>>,
    /// Wheel travel not scrolled yet, positive toward the end, and when the last step went.
    wheel: Rc<Cell<f32>>,
    stepped: Cell<Option<Instant>>,
    pub(super) slider: Slider,
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
        let m = Self {
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            hi: shown(spec, "", None)
                .iter()
                .position(|m| is_pick(spec, m))
                .unwrap_or(0),
            query: String::new(),
            effort,
            rail: None,
            opened: OPENED.fetch_add(1, Ordering::Relaxed),
            glide: Glide::default(),
            rail_glide: Glide::default(),
            rows: RefCell::default(),
            wheel: Rc::default(),
            stepped: Cell::new(None),
            slider: Slider::default(),
        };
        window.focus(&m.focus, cx);
        m
    }

    /// Whether this is the effort menu, so the host opens it from the effort chip.
    pub fn is_effort(&self) -> bool {
        self.effort
    }

    /// One frame of easing the list toward where the wheel sent it. GPUI's offset runs
    /// negative as the list scrolls down.
    fn ease(&self, window: &mut Window) {
        let left = self.wheel.get();
        if left == 0. {
            self.stepped.set(None);
            return;
        }
        let now = Instant::now();
        let dt = self
            .stepped
            .replace(Some(now))
            .map_or(1. / 120., |t| (now - t).as_secs_f32().min(0.05));
        let step = if left.abs() < 0.5 {
            left
        } else {
            left * (1. - (-dt / EASE).exp())
        };
        let at = self.scroll.offset();
        let max = self.scroll.max_offset().y;
        let y = (at.y - px(step)).clamp(-max, px(0.));
        self.scroll.set_offset(point(at.x, y));
        // an end reached drops what is left, so the next turn of the wheel answers at once
        let stuck = y == -max || y == px(0.);
        self.wheel.set(if stuck { 0. } else { left - step });
        window.request_animation_frame();
    }
}

/// Takes the wheel over the list before the list sees it, so it can be eased.
fn wheel<H: Host>(m: &ModelMenu, cx: &mut Context<H>) -> AnyElement {
    let pending = m.wheel.clone();
    let view = cx.weak_entity();
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || !hitbox.is_hovered(window) {
                    return;
                }
                let dy = e.delta.pixel_delta(px(LINE)).y;
                pending.set(pending.get() - f32::from(dy));
                cx.stop_propagation();
                let _ = view.update(cx, |_, cx| cx.notify());
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
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

/// Whether `model` is the one picked now, whichever window it's on.
fn is_pick(spec: &Spec, model: &Model) -> bool {
    model.agent == spec.agent && model.id == crate::models::windowed(&spec.model, false)
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

/// Closes the menu, applying a level the effort slider moved to but hadn't applied yet.
pub(super) fn close<H: Host>(h: &mut H, window: &mut Window, cx: &mut Context<H>) {
    let level = h.model_menu().as_mut().and_then(|m| m.slider.take());
    *h.model_menu() = None;
    window.focus(&h.focus_handle(cx), cx);
    if let Some(level) = level {
        effort::apply(h, level, cx);
    }
    cx.notify();
}

/// From the effort card to the model list, which opens over the model chip instead.
pub(super) fn to_models<H: Host>(h: &mut H, window: &mut Window, cx: &mut Context<H>) {
    let Some(spec) = h.model_spec() else {
        return;
    };
    let level = h.model_menu().as_mut().and_then(|m| {
        m.effort = false;
        m.opened = OPENED.fetch_add(1, Ordering::Relaxed);
        m.hi = shown(&spec, "", None)
            .iter()
            .position(|x| is_pick(&spec, x))
            .unwrap_or(0);
        window.focus(&m.focus, cx);
        m.slider.take()
    });
    if let Some(level) = level {
        effort::apply(h, level, cx);
    }
    cx.notify();
}

fn pick<H: Host>(h: &mut H, agent: Agent, id: String, window: &mut Window, cx: &mut Context<H>) {
    close(h, window, cx);
    h.choose(Choice::Model(agent, id), cx);
}

fn key<H: Host>(h: &mut H, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<H>) {
    let Some(spec) = h.model_spec() else {
        return;
    };
    if h.model_menu().as_ref().is_some_and(|m| m.effort) {
        effort::key(h, &spec, e, window, cx);
        return;
    }
    let Some(m) = h.model_menu() else {
        return;
    };
    let count = shown(&spec, &m.query, m.rail).len();
    let k = &e.keystroke;
    match k.key.as_str() {
        "up" => m.hi = m.hi.saturating_sub(1),
        "down" => m.hi = (m.hi + 1).min(count.saturating_sub(1)),
        "enter" => {
            let model = shown(&spec, &m.query, m.rail)
                .get(m.hi)
                .map(|m| (m.agent, m.id.clone()));
            if let Some((agent, id)) = model {
                pick(h, agent, id, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        "escape" => {
            close(h, window, cx);
            cx.stop_propagation();
            return;
        }
        "backspace" => {
            m.query.pop();
            m.hi = 0;
        }
        _ => {
            let plain = !(k.modifiers.control || k.modifiers.alt || k.modifiers.platform);
            match k.key_char.as_deref() {
                Some(c) if plain && !c.chars().any(char::is_control) => {
                    m.query.push_str(c);
                    m.hi = 0;
                }
                _ => return,
            }
        }
    }
    let row = m.rows.borrow().get(m.hi).copied();
    if let Some(row) = row {
        m.scroll.scroll_to_item(row);
    }
    cx.stop_propagation();
    cx.notify();
}

/// A small label beside a name: a model's note, or which level is the default.
pub(super) fn badge(text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded_full()
        .bg(colors::ink(0.06))
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(text.into())
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
    m.ease(window);
    let body = if m.effort {
        effort::body(m, spec, cx)
    } else {
        models_body(m, spec, cx)
    };
    let foot = spec.foot.map(|line| {
        div()
            .px(px(12.))
            .py(px(8.))
            .border_t_1()
            .border_color(colors::border1())
            .text_size(px(11.))
            .text_color(colors::text3())
            .child(line)
    });
    let frame = div()
        .id("model-menu")
        .track_focus(&m.focus)
        .on_key_down(cx.listener(key::<H>))
        .relative()
        .w(px(if m.effort { effort::CARD } else { 340. }))
        .flex()
        .flex_col()
        // the app's menu frame, as the context menus and dialogs draw it
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .overflow_hidden()
        .text_size(px(12.5))
        .text_color(colors::text1())
        .child(body)
        .children(foot);
    // under the chip where it fits, as on the new-thread screen, so it covers neither the
    // heading nor the text; over it under a thread, where the box sits at the window's foot
    let tall = if m.effort { EFFORT_TALL } else { MODELS_TALL };
    let below = window.viewport_size().height - chip.bottom() >= px(tall + 16.);
    // slides a few pixels into place, away from its chip, as it fades in
    let from = if below { -8. } else { 8. };
    let frame = crate::slide::ease_in(frame, ("model-menu-open", m.opened), 160, move |d, t| {
        d.opacity(t).top(px(from * (1. - t)))
    });
    let dismiss = cx.listener(|h: &mut H, _: &(), window, cx| close(h, window, cx));
    Some(widgets::by_chip(
        chip,
        below,
        window,
        move |w, cx| dismiss(&(), w, cx),
        frame,
    ))
}

fn models_body<H: Host>(m: &ModelMenu, spec: &Spec, cx: &mut Context<H>) -> AnyElement {
    let all = agents(spec);
    let rail = (all.len() > 1).then(|| {
        let button = |id: &'static str, n: usize| {
            div()
                .id((id, n))
                .flex()
                .items_center()
                .justify_center()
                .size(px(30.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.04)))
        };
        let choose = |agent: Option<Agent>| {
            cx.listener(move |h: &mut H, _: &ClickEvent, _, cx| {
                if let Some(m) = h.model_menu() {
                    m.rail = agent;
                    m.hi = 0;
                    m.scroll.scroll_to_item(0);
                }
                cx.notify();
            })
        };
        let at = m
            .rail
            .and_then(|a| all.iter().position(|&x| x == a))
            .map_or(0, |i| i + 1);
        let spot = m.rail_glide.toward(RAIL_TOP + at as f32 * RAIL_STEP).apply(
            "rail-spot",
            div()
                .absolute()
                .left(px(7.))
                .size(px(30.))
                .rounded(px(8.))
                .bg(colors::ink(0.09)),
            |d, y| d.top(px(y)),
        );
        div()
            .relative()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .w(px(44.))
            .py(px(RAIL_TOP))
            .border_r_1()
            .border_color(colors::border1())
            .bg(colors::surface1())
            .child(spot)
            .child(
                button("rail-all", 0)
                    .child(icon("layout-grid", 14., colors::text2()))
                    .on_click(choose(None)),
            )
            .children(all.iter().enumerate().map(|(n, &agent)| {
                button("rail", n)
                    .child(mark(agent, 15., colors::brand(agent).0))
                    .on_click(choose(Some(agent)))
            }))
    });
    // a flat row with a line under it, as the palette's search is
    let search = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(40.))
        .mb(px(4.))
        .px(px(14.))
        .border_b_1()
        .border_color(colors::border1())
        .child(icon("search", 13., colors::text3()))
        .child(if m.query.is_empty() {
            div().text_color(colors::text3()).child("Search models")
        } else {
            div().text_color(colors::text1()).child(m.query.clone())
        });

    // agent headings and model rows, measured as they go so the highlight knows where to sit
    let shown = shown(spec, &m.query, m.rail);
    let mut children: Vec<AnyElement> = Vec::new();
    let mut rows = Vec::new();
    let mut y = 0.;
    let mut hi_y = None;
    let mut group = None;
    for (i, model) in shown.iter().enumerate() {
        if group != Some(model.agent) {
            group = Some(model.agent);
            children.push(heading(model.agent).into_any_element());
            y += HEADING;
        }
        if i == m.hi {
            hi_y = Some(y);
        }
        // the highlight goes in front of every row
        rows.push(children.len() + 1);
        let (agent, id) = (model.agent, model.id.clone());
        children.push(
            model_row(i, model, is_pick(spec, model))
                .on_mouse_move(cx.listener(move |h: &mut H, _: &MouseMoveEvent, _, cx| {
                    if let Some(m) = h.model_menu()
                        && m.hi != i
                    {
                        m.hi = i;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |h: &mut H, _: &ClickEvent, window, cx| {
                    pick(h, agent, id.clone(), window, cx)
                }))
                .into_any_element(),
        );
        y += ROW;
    }
    *m.rows.borrow_mut() = rows;
    let highlight = match hi_y {
        Some(y) => m.glide.toward(y).apply(
            "model-hi",
            div()
                .absolute()
                .left(px(4.))
                .right(px(4.))
                .h(px(ROW))
                .rounded(px(8.))
                .bg(colors::ink(0.06)),
            |d, y| d.top(px(y)),
        ),
        None => div().into_any_element(),
    };
    let empty = shown.is_empty();
    let list = div()
        .id("model-menu-list")
        .track_scroll(&m.scroll)
        .relative()
        .max_h(px(280.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .px(px(4.))
        .pb(px(4.))
        .child(highlight)
        .children(children)
        .when(empty, |d| {
            d.child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .text_color(colors::text3())
                    .child("No model matches."),
            )
        });
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
                .child(div().relative().child(list).child(wheel(m, cx))),
        )
        .into_any_element()
}

/// An agent's name over its models.
fn heading(agent: Agent) -> Div {
    div()
        .flex_none()
        .h(px(HEADING))
        .flex()
        .items_end()
        .gap(px(6.))
        .px(px(10.))
        .pb(px(5.))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(mark(agent, 11., colors::brand(agent).0))
        .child(agent.name())
}

/// One model: its name, a short note, and a tick on the pick.
fn model_row(i: usize, model: &Model, on: bool) -> Stateful<Div> {
    let note = model.note.as_deref().filter(|n| short(n));
    div()
        .id(("model-row", i))
        .flex_none()
        .h(px(ROW))
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .rounded(px(8.))
        .cursor_pointer()
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(model.label.clone()),
        )
        .children(note.map(|n| badge(n.to_string())))
        .child(div().flex_1())
        .when(on, |d| d.child(icon("check", 13., colors::accent())))
}

/// Whether a model's note fits beside its name as a badge. The catalog's own notes do
/// ("Fastest"); the sentences Codex's model cache carries would only show cut off.
fn short(note: &str) -> bool {
    note.chars().count() <= 14
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::composer) fn spec() -> Spec {
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
        assert!(is_pick(&s, &s.models[1]));
        assert!(!is_pick(&s, &s.models[0]));
        assert_eq!(effort_chip_label(&s), "High · 1M");
    }
}
