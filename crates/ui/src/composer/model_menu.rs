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
    ScrollWheelEvent, SharedString, Stateful, Window, canvas, div, point, prelude::*, relative,
};
use hyprspace_proto::Agent;

use super::effort::{self, Slider};
use super::pickers::effort_label;
use crate::assets::{icon, mark};
use crate::slide::Glide;
use crate::zoom::px;
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
    pub fast: Option<bool>,
    /// A line at the bottom on what a change does.
    pub foot: Option<&'static str>,
}

pub enum Choice {
    Model(Agent, String),
    Effort(String),
    /// The 1M context window on or off.
    Long(bool),
    Fast(bool),
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
    let mut out = level;
    if spec.long == Some(true) {
        out.push_str(" · 1M");
    }
    if spec.fast == Some(true) {
        out.push_str(" · Fast");
    }
    out
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
    legacy: Option<Agent>,
    legacy_opened: usize,
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
            hi: shown(spec, "", None, false)
                .iter()
                .position(|m| is_pick(spec, m))
                .unwrap_or(0),
            query: String::new(),
            effort,
            rail: None,
            legacy: None,
            legacy_opened: 0,
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
        let y = (at.y - gpui::px(step)).clamp(-max, px(0.));
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

/// The models on the rail's agent whose name (or agent) has every word of `query`, each agent's
/// legacy ones last and left out while folded, unless a search is looking for them.
fn shown<'a>(spec: &'a Spec, query: &str, rail: Option<Agent>, legacy: bool) -> Vec<&'a Model> {
    let q = query.to_lowercase();
    let all = agents(spec);
    let mut out: Vec<&Model> = spec
        .models
        .iter()
        .filter(|m| rail.is_none_or(|a| a == m.agent))
        .filter(|m| legacy || !q.is_empty() || !is_legacy(spec, m))
        .filter(|m| {
            let hay = format!("{} {}", m.label, m.agent.name()).to_lowercase();
            q.split_whitespace().all(|w| hay.contains(w))
        })
        .collect();
    out.sort_by_key(|m| (all.iter().position(|&a| a == m.agent), is_legacy(spec, m)));
    out
}

/// A model's name without its version: "Opus 5.5" is Opus, "GPT-5.6-Terra" is GPT Terra.
fn family(label: &str) -> String {
    label
        .split(|c: char| c.is_whitespace() || c == '-')
        .filter(|w| !w.is_empty() && !w.chars().any(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a newer model of the same family comes before `model`. The catalogs list newest
/// first.
fn is_legacy(spec: &Spec, model: &Model) -> bool {
    if model
        .note
        .as_deref()
        .is_some_and(|n| n.starts_with("Older") || n.starts_with("Legacy"))
    {
        return true;
    }
    let f = family(&model.label);
    spec.models
        .iter()
        .take_while(|m| !std::ptr::eq(*m, model))
        .any(|m| m.agent == model.agent && family(&m.label) == f)
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
        m.hi = shown(&spec, "", None, false)
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
    let count = shown(&spec, &m.query, m.rail, false).len();
    let k = &e.keystroke;
    match k.key.as_str() {
        "up" => {
            m.hi = m.hi.saturating_sub(1);
            m.legacy = None;
        }
        "down" => {
            m.hi = (m.hi + 1).min(count.saturating_sub(1));
            m.legacy = None;
        }
        "enter" => {
            let model = shown(&spec, &m.query, m.rail, false)
                .get(m.hi)
                .map(|m| (m.agent, m.id.clone()));
            if let Some((agent, id)) = model {
                pick(h, agent, id, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        "escape" if m.legacy.is_some() => m.legacy = None,
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
    let panel = m
        .legacy
        .filter(|_| !m.effort && m.query.is_empty())
        .map(|agent| {
            let edge = if widgets::from_left(chip, window) {
                chip.left() + px(if m.effort { effort::CARD } else { 340. })
            } else {
                chip.right()
            };
            let left = edge + px(250.) > window.viewport_size().width;
            div()
                .absolute()
                .when(below, |d| d.top_0())
                .when(!below, |d| d.bottom_0())
                .when(left, |d| d.right(relative(1.)).mr(px(6.)))
                .when(!left, |d| d.left(relative(1.)).ml(px(6.)))
                .child(legacy_panel(m, spec, agent, cx))
        });
    let frame = div().relative().child(frame).children(panel);
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
    let shown = shown(spec, &m.query, m.rail, false);
    let mut folds: Vec<(Agent, AnyElement)> = Vec::new();
    for agent in agents(spec) {
        let n = spec
            .models
            .iter()
            .filter(|x| x.agent == agent && is_legacy(spec, x))
            .count();
        let picked = spec
            .models
            .iter()
            .any(|x| x.agent == agent && is_pick(spec, x) && is_legacy(spec, x));
        if m.query.is_empty() && n > 0 {
            folds.push((
                agent,
                legacy_row(agent, n, m.legacy == Some(agent), picked, cx),
            ));
        }
    }
    let mut folded = |agent: Agent| {
        let at = folds.iter().position(|(a, _)| *a == agent)?;
        Some(folds.remove(at).1)
    };
    let mut children: Vec<AnyElement> = Vec::new();
    let mut rows = Vec::new();
    let mut y = 0.;
    let mut hi_y = None;
    let mut group = None;
    let mut fold = None;
    for (i, model) in shown.iter().enumerate() {
        if group != Some(model.agent) {
            if let Some(row) = fold.take() {
                children.push(row);
                y += ROW;
            }
            group = Some(model.agent);
            fold = folded(model.agent);
            children.push(heading(model.agent).into_any_element());
            y += HEADING;
        }
        if is_legacy(spec, model)
            && let Some(row) = fold.take()
        {
            children.push(row);
            y += ROW;
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
                        && (m.hi != i || m.legacy.is_some())
                    {
                        m.hi = i;
                        m.legacy = None;
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
    children.extend(fold);
    *m.rows.borrow_mut() = rows;
    let highlight = match hi_y.filter(|_| m.legacy.is_none()) {
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
        .px(px(10.))
        .pb(px(5.))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text3())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(mark(agent, 11., colors::brand(agent).0))
                .child(agent.name()),
        )
}

/// The row that opens an agent's older models beside the menu, with how many there are.
fn legacy_row<H: Host>(
    agent: Agent,
    n: usize,
    open: bool,
    picked: bool,
    cx: &mut Context<H>,
) -> AnyElement {
    let show = move |h: &mut H, on: bool| {
        if let Some(m) = h.model_menu()
            && (m.legacy == Some(agent)) != on
        {
            m.legacy = on.then_some(agent);
            m.legacy_opened += 1;
            return true;
        }
        false
    };
    div()
        .id(("legacy-row", agent as usize))
        .flex_none()
        .h(px(ROW))
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .rounded(px(8.))
        .cursor_pointer()
        .text_color(colors::text3())
        .when(open, |d| {
            d.bg(colors::ink(0.06)).text_color(colors::text1())
        })
        .hover(|s| s.bg(colors::ink(0.06)))
        .child("Legacy models")
        .child(badge(n.to_string()))
        .child(div().flex_1())
        .when(picked, |d| d.child(icon("check", 13., colors::accent())))
        .child(icon("chevron-right", 12., colors::text3()))
        .on_mouse_move(cx.listener(move |h: &mut H, _: &MouseMoveEvent, _, cx| {
            if show(h, true) {
                cx.notify();
            }
        }))
        .on_click(cx.listener(move |h: &mut H, _: &ClickEvent, _, cx| {
            show(h, !open);
            cx.notify();
        }))
        .into_any_element()
}

fn legacy_panel<H: Host>(
    m: &ModelMenu,
    spec: &Spec,
    agent: Agent,
    cx: &mut Context<H>,
) -> AnyElement {
    let rows = spec
        .models
        .iter()
        .filter(|x| x.agent == agent && is_legacy(spec, x))
        .enumerate()
        .map(|(i, model)| {
            let id = model.id.clone();
            model_row(1000 + i, model, is_pick(spec, model))
                .hover(|s| s.bg(colors::ink(0.06)))
                .on_click(cx.listener(move |h: &mut H, _: &ClickEvent, window, cx| {
                    pick(h, agent, id.clone(), window, cx)
                }))
        });
    let panel = div()
        .id("legacy-panel")
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(px(240.))
        .max_h(px(320.))
        .overflow_y_scroll()
        .p(px(4.))
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.5))
        .text_color(colors::text1())
        .children(rows);
    crate::slide::ease_in(
        panel,
        ("legacy-panel-open", m.opened * 1000 + m.legacy_opened),
        120,
        |d, t| d.opacity(t).ml(px(-6. * (1. - t))),
    )
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
            fast: None,
            foot: None,
        }
    }

    #[test]
    fn the_rail_and_the_search_narrow_the_models() {
        let s = spec();
        assert_eq!(shown(&s, "", None, false).len(), 4);
        assert_eq!(shown(&s, "", Some(Agent::Codex), false).len(), 1);
        assert_eq!(shown(&s, "OPUS", None, false)[0].id, "claude-opus-5-5");
        assert!(shown(&s, "opus", Some(Agent::Codex), false).is_empty());
        assert_eq!(agents(&s), [Agent::Claude, Agent::Codex]);
    }

    #[test]
    fn older_models_fold_under_legacy() {
        let mut s = spec();
        let m = |id: &str, label: &str| Model {
            agent: Agent::Claude,
            id: id.into(),
            label: label.into(),
            note: None,
        };
        s.models.insert(2, m("claude-opus-5", "Opus 5"));
        s.models.insert(3, m("claude-haiku-5-5", "Haiku 5.5"));
        let ids = |v: Vec<&Model>| v.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
        assert_eq!(
            ids(shown(&s, "", None, false)),
            ["", "claude-opus-5-5", "claude-haiku-5-5", "gpt-5.5"]
        );
        assert_eq!(
            ids(shown(&s, "", None, true)),
            [
                "",
                "claude-opus-5-5",
                "claude-haiku-5-5",
                "claude-opus-5",
                "claude-haiku-4-5",
                "gpt-5.5"
            ]
        );
        assert_eq!(ids(shown(&s, "opus", None, false)).len(), 2);
        assert_eq!(family("GPT-5.6-Terra"), "GPT Terra");
        assert_eq!(family("GPT-6-Luna"), family("GPT-5.6-Luna"));
        s.models.push(Model {
            agent: Agent::Codex,
            id: "gpt-5.6-terra".into(),
            label: "GPT-5.6-Terra".into(),
            note: Some("Older balanced model for straightforward work.".into()),
        });
        assert!(is_legacy(&s, s.models.last().unwrap()));
    }

    #[test]
    fn a_long_window_model_is_still_the_pick() {
        let s = spec();
        assert!(is_pick(&s, &s.models[1]));
        assert!(!is_pick(&s, &s.models[0]));
        assert_eq!(effort_chip_label(&s), "High · 1M");
    }
}
