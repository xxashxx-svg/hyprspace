// Draws a transcript after zeron's: one centered column, the user's prompts as bubbles on the
// right, replies as markdown, thinking and each run of tool calls folded into one muted line
// until clicked, approval prompts with their buttons, and the pill-shaped box to reply or steer.

use chrono::TimeZone;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, Div, ElementId, ExternalPaths,
    Focusable, FontWeight, HighlightStyle, InteractiveText, IntoElement, MouseButton,
    ScrollWheelEvent, SharedString, StyledText, Transformation, Window, div, list, percentage,
    prelude::*, relative,
};
use hyprspace_proto::{RunStatus, TaskKind, Tool};

use super::model::{self, Item};
use super::rows::{self, Row};
use super::{TranscriptEvent, TranscriptView, doing, tool};
use crate::assets::{icon, mark};
use crate::composer::model_menu::{self, Host as _, ModelMenu};
use crate::pace::Looping;
use crate::slide::{self, Glide, ease_out};
use crate::zoom::px;
use crate::{attach, colors, markdown, spinner, widgets};

/// The transcript and the composer share one column, so their edges line up. Text runs 768px
/// wide, as T3 Code's chat does.
const COLUMN: f32 = 816.;
const GUTTER: f32 = 24.;
const ENTER: Duration = Duration::from_millis(280);
const UNFOLD: Duration = Duration::from_millis(200);
const WORKING_IN: Duration = Duration::from_millis(240);

pub fn view(
    v: &mut TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    if let Some(effort) = v.want_menu.take()
        && let Some(spec) = v.model_spec()
    {
        v.menu = Some(if effort {
            ModelMenu::open_effort(&spec, window, cx)
        } else {
            ModelMenu::open(&spec, window, cx)
        });
    }
    if v._focus.is_empty() {
        let input = v.input.focus_handle(cx);
        v._focus = vec![
            cx.on_focus(&input, window, |_, _, cx| cx.notify()),
            cx.on_blur(&input, window, |_, _, cx| cx.notify()),
        ];
    }
    v.follow.ease(&v.list, window);
    if !v.finding() {
        v.follow.step(&v.list, window, cx);
    }
    // rows measure taller than estimated once laid out, which only shows after this frame
    cx.on_next_frame(window, |v, window, cx| {
        if v.follow.behind(&v.list) {
            crate::pace::next_for(window, cx.entity_id());
        }
    });
    let revealed = v
        .reveal
        .as_mut()
        .and_then(|r| match v.model.items.get(r.ix) {
            Some(Item::Text { source, .. }) => r.step(source, window, cx).map(|end| (r.ix, end)),
            _ => None,
        });
    // a reply still coming in goes through the streaming parser even once the reveal caught up,
    // so its unfinished markup never shows
    let live = revealed.or_else(|| match v.model.items.last() {
        Some(Item::Text { source, .. }) if v.model.elapsed().is_some() => {
            Some((v.model.items.len() - 1, source.len()))
        }
        _ => None,
    });
    // between beats the text is what it was, and so is its parse
    if live.is_none() || live != v.partial_at {
        v.partial = live.and_then(|(ix, end)| match v.model.items.get(ix) {
            Some(Item::Text { source, .. }) => {
                let text = &source[..end];
                if v.model.after_command(ix) {
                    return Some((ix, model::reply(text, true)));
                }
                if v.stream.0 != ix {
                    v.stream = (ix, Default::default());
                }
                Some((ix, v.stream.1.parse(text)))
            }
            _ => None,
        });
    }
    v.partial_at = live;
    if slide::animations()
        && let Some((ix, b)) = &v.partial
    {
        v.fade.grow(*ix, b);
    }
    if v.fade.tick() {
        crate::pace::beat(window, cx);
    }
    v.model.parse(v.partial.as_ref().map(|(ix, _)| *ix));
    let now = Instant::now();
    for ix in v.seen.max(v.fresh)..v.model.items.len() {
        v.born.insert(ix, now);
    }
    v.seen = v.model.items.len();
    v.born.retain(|_, at| at.elapsed() < ENTER);
    if !v.born.is_empty() {
        crate::pace::next(window, cx);
    }
    let live = v.model.elapsed().is_some();
    v.working = live.then(|| v.working.unwrap_or(now));
    if !live {
        v.doing = Default::default();
    } else if v.doing.step(doing::label(&v.model, v.stopping), v.stopping) {
        crate::pace::next(window, cx);
    }
    if v.working.is_some_and(|at| at.elapsed() < WORKING_IN) {
        crate::pace::next(window, cx);
    }
    let rows = rows::split(rows::plan(&v.model.items, v.loading, live), |ix| {
        reply_blocks(v, ix).map_or(1, |b| rows::rows_for(b))
    });
    v.sync_rows(rows);
    let prompts: Vec<usize> = v
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Row::Item { user: true, .. }))
        .map(|(i, _)| i)
        .collect();
    let surface: Arc<str> = format!("t{}", v.id.0).into();
    edge_scroll(v, &surface, window);
    v.aim_find(window);
    let hunt = v.find.as_ref().map(|f| f.hunt(cx));
    markdown::find::set(
        &surface,
        hunt.as_ref().map(|(q, current)| (q.as_str(), *current)),
    );
    let jump = v.follow.away(&v.list).then(|| jump(cx));
    let ticks = ticks(v, &prompts, window, cx);
    let rows = list(
        v.list.clone(),
        cx.processor(|v, ix: usize, _, cx| list_row(v, ix, cx)),
    )
    .size_full();
    let bar = v.find_bar(cx);
    div()
        .id("transcript")
        .key_context("Thread")
        .on_action(cx.listener(|v, _: &super::FindInThread, window, cx| v.open_find(window, cx)))
        .on_action(cx.listener(|v, _: &super::PrevPrompt, _, cx| {
            jump_prompt(v, false);
            cx.notify();
        }))
        .on_action(cx.listener(|v, _: &super::NextPrompt, _, cx| {
            jump_prompt(v, true);
            cx.notify();
        }))
        .size_full()
        .flex()
        .flex_col()
        .bg(colors::bg())
        .text_color(colors::text1())
        .on_drop(
            cx.listener(|v, paths: &ExternalPaths, window, cx| v.drop_paths(paths, window, cx)),
        )
        .child(
            div()
                .id("transcript-scroll")
                .relative()
                .flex_1()
                .min_h_0()
                .on_scroll_wheel(cx.listener(|v, e: &ScrollWheelEvent, _, cx| {
                    let dy = e.delta.pixel_delta(px(20.)).y;
                    v.follow.wheel(dy > px(0.));
                    stir_rail(v, dy, cx);
                    cx.notify();
                }))
                .child(wheel(cx))
                // before any text, so selection knows this transcript's elements
                .child(markdown::select::reset(surface.clone()))
                .child(rows)
                .child(markdown::select::tail(surface))
                .children(ticks)
                .children(jump)
                .children(bar),
        )
        .child(composer(v, window, cx))
        .children(model_menu(v, window, cx))
        .children(permission_menu(v, window, cx))
        .into_any_element()
}

/// Takes a mouse wheel's notches before the list sees them, so they glide instead of jumping a
/// notch a frame. A touchpad's pixels and sideways scrolls go on to the list as they were.
fn wheel(cx: &mut Context<TranscriptView>) -> AnyElement {
    let view = cx.weak_entity();
    gpui::canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
                let gpui::ScrollDelta::Lines(lines) = e.delta else {
                    return;
                };
                if phase != gpui::DispatchPhase::Capture
                    || lines.x != 0.
                    || lines.y == 0.
                    || !hitbox.is_hovered(window)
                {
                    return;
                }
                let dy = px(20.) * lines.y;
                cx.stop_propagation();
                let _ = view.update(cx, |v, cx| {
                    v.follow.spin(-dy);
                    stir_rail(v, dy, cx);
                    cx.notify();
                });
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

fn edge_scroll(v: &mut TranscriptView, surface: &Arc<str>, window: &mut Window) {
    let Some(at) = markdown::select::dragging(surface) else {
        v.drag_step = None;
        return;
    };
    let b = v.list.viewport_bounds();
    let edge = px(32.);
    let over = if at.y < b.top() + edge {
        at.y - (b.top() + edge)
    } else if at.y > b.bottom() - edge {
        at.y - (b.bottom() - edge)
    } else {
        px(0.)
    };
    let now = Instant::now();
    let dt = v
        .drag_step
        .replace(now)
        .map_or(0., |t| (now - t).as_secs_f32().min(0.05));
    if over != px(0.) {
        v.list.scroll_by(over.clamp(px(-80.), px(80.)) * (dt * 12.));
        if over < px(0.) {
            v.follow.wheel(true);
        }
        window.request_animation_frame();
    }
}

fn list_row(v: &mut TranscriptView, ix: usize, cx: &mut Context<TranscriptView>) -> AnyElement {
    let Some(&row) = v.rows.get(ix) else {
        return div().into_any_element();
    };
    let body = match row {
        Row::Loading => div()
            .text_size(px(12.))
            .text_color(colors::text3())
            .child("Loading the conversation...")
            .into_any_element(),
        Row::Empty => div()
            .pt(px(80.))
            .flex()
            .justify_center()
            .text_size(px(13.))
            .text_color(colors::text3())
            .child(format!(
                "Send a message to start {} in {}.",
                v.launch.agent.name(),
                folder_name(v)
            ))
            .into_any_element(),
        Row::Item { ix, .. } => {
            let el = item(v, ix, &v.model.items[ix], cx);
            enter(v, ix, el)
        }
        Row::Part { ix, part } => {
            let el = reply(v, ix, part);
            enter(v, ix, el)
        }
        Row::Quiet(start) => quiet(v, start, cx),
        Row::Working => {
            let t = v.working.filter(|_| slide::animations()).map_or(1., |at| {
                ease_out((at.elapsed().as_secs_f32() / WORKING_IN.as_secs_f32()).min(1.))
            });
            div()
                .relative()
                .top(px(4. * (1. - t)))
                .opacity(t)
                .child(working(v))
                .into_any_element()
        }
    };
    let joined = matches!(v.rows.get(ix + 1), Some(Row::Part { .. }));
    centered(column().child(body))
        .when(ix == 0, |d| d.pt(px(24.)))
        .when(!joined, |d| d.pb(px(20.)))
        .into_any_element()
}

/// The blocks a reply draws: its streaming parse while it comes in.
fn reply_blocks(v: &TranscriptView, ix: usize) -> Option<&Vec<markdown::Block>> {
    let Some(Item::Text { blocks, .. }) = v.model.items.get(ix) else {
        return None;
    };
    v.partial
        .as_ref()
        .filter(|(p, _)| *p == ix)
        .map(|(_, b)| b)
        .or(blocks.as_ref())
}

/// Piece `part` of a reply, spaced from the next as the reply drawn whole would space them.
fn reply(v: &TranscriptView, ix: usize, part: usize) -> AnyElement {
    let (Some(Item::Text { source, .. }), Some(b)) = (v.model.items.get(ix), reply_blocks(v, ix))
    else {
        return div().into_any_element();
    };
    let units = markdown::units(b);
    let stretches = rows::stretches(&units);
    let Some(range) = stretches.get(part).cloned() else {
        return div().into_any_element();
    };
    let key = format!("t{ix}");
    let fade = v.fade.at(ix);
    let last = part + 1 == stretches.len();
    let in_list = matches!(
        (units.get(range.end.wrapping_sub(1)), units.get(range.end)),
        (Some(markdown::Unit::Entry(a, _)), Some(markdown::Unit::Entry(b, _))) if a == b
    );
    let body = if stretches.len() == 1 {
        markdown::render(b, &key, &fade)
    } else {
        markdown::part(b, &key, &fade, &units, range)
    };
    div()
        .group(SharedString::from(format!("msg-{ix}")))
        .flex()
        .flex_col()
        .gap(px(2.))
        .text_size(px(14.))
        .line_height(relative(1.65))
        .text_color(colors::text1())
        .child(body)
        .when(last && !streaming(v, ix), |d| {
            d.child(hover_copy(ix, source))
        })
        .when(!last && in_list, |d| d.pb_1())
        .when(!last && !in_list, |d| d.pb(px(10.)))
        .into_any_element()
}

fn centered(child: Div) -> Div {
    div().w_full().flex().justify_center().child(child)
}

fn column() -> Div {
    div()
        .w_full()
        .max_w(px(COLUMN))
        .px(px(GUTTER))
        .flex()
        .flex_col()
}

fn folder_name(v: &TranscriptView) -> String {
    v.launch
        .cwd
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "this folder".into())
}

fn retry(v: &TranscriptView, ix: usize, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    (v.retry_at() == Some(ix)).then(|| {
        div()
            .flex()
            .child(
                widgets::button(("retry", ix), "Try again")
                    .h(px(24.))
                    .px(px(9.))
                    .text_size(px(12.))
                    .text_color(colors::text1())
                    .child(icon("rotate-cw", 12., colors::text2()))
                    .flex_row_reverse()
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.retry(cx))),
            )
            .into_any_element()
    })
}

fn took(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m {}s", secs / 60, secs % 60),
        _ => format!("{}h {}m", secs / 3600, secs / 60 % 60),
    }
}

fn working(v: &TranscriptView) -> AnyElement {
    let took = took(v.model.elapsed().unwrap_or(0));
    let line = |text: &str| {
        div()
            .flex()
            .items_center()
            .h(px(22.))
            .flex_none()
            .min_w_0()
            .child(div().min_w_0().truncate().child(text.to_string()))
    };
    let label = div()
        .flex_1()
        .min_w_0()
        .h(px(22.))
        .overflow_hidden()
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(
            div()
                .relative()
                .when(v.doing.prev.is_some(), |d| {
                    d.top(px(-22. * ease_out(v.doing.roll)))
                })
                .flex()
                .flex_col()
                .children(v.doing.prev.as_deref().map(line))
                .child(line(&v.doing.shown)),
        )
        .looping(1800, |d, t| {
            d.opacity(0.55 + 0.45 * (0.5 + 0.5 * (t * std::f32::consts::TAU).cos()))
        });
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(22.))
        .text_size(px(12.5))
        .text_color(colors::text3())
        .child(if v.model.compacting && !v.stopping {
            spinner::compass(colors::text2())
        } else {
            spinner::eclipse("working", colors::text2())
        })
        .child(label)
        .child(div().flex_none().child(took))
        .when(!v.stopping, |d| {
            d.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(5.))
                    .child(widgets::keycap("Esc"))
                    .child("to stop"),
            )
        })
        .into_any_element()
}

fn jump(cx: &mut Context<TranscriptView>) -> AnyElement {
    let pill = div()
        .absolute()
        .bottom(px(12.))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .id("scroll-to-bottom")
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .px(px(12.))
                .rounded_full()
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface2())
                .text_size(px(12.))
                .text_color(colors::text1())
                .cursor_pointer()
                .hover(|s| s.bg(colors::surface3()))
                .child(icon("arrow-down", 12., colors::text2()))
                .child("Scroll to bottom")
                .on_click(cx.listener(|v, _: &ClickEvent, _, cx| {
                    v.follow.down(&v.list);
                    cx.notify();
                })),
        );
    slide::ease_in(pill, "jump-in", 180, |d, t| {
        d.opacity(t).bottom(px(4. + 8. * t))
    })
}

fn enter(v: &TranscriptView, ix: usize, row: AnyElement) -> AnyElement {
    let Some(at) = v.born.get(&ix) else {
        return row;
    };
    let t = ease_out((at.elapsed().as_secs_f32() / ENTER.as_secs_f32()).min(1.));
    div()
        .relative()
        .top(px(10. * (1. - t)))
        .opacity(t)
        .child(row)
        .into_any_element()
}

fn quiet(v: &TranscriptView, start: usize, cx: &mut Context<TranscriptView>) -> AnyElement {
    let all = &v.model.items;
    let end = rows::quiet_end(all, start);
    let mut out = Vec::new();
    let mut ix = start;
    while ix < end {
        let calls = all[ix..end]
            .iter()
            .take_while(|i| matches!(i, Item::Tool { .. }))
            .count();
        if calls > 1 {
            let row = tool_run(v, ix, ix + calls, cx);
            out.push(enter(v, ix, row));
            ix += calls;
        } else {
            let row = item(v, ix, &all[ix], cx);
            out.push(enter(v, ix, row));
            ix += 1;
        }
    }
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(out)
        .into_any_element()
}

fn jump_prompt(v: &mut TranscriptView, down: bool) {
    let prompts: Vec<usize> = v
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r, Row::Item { user: true, .. }))
        .map(|(i, _)| i)
        .collect();
    let (first, top) = (
        v.list.logical_scroll_top().item_ix,
        v.list.viewport_bounds().top(),
    );
    let above = |row: usize| {
        row < first
            || v.list
                .bounds_for_item(row)
                .is_some_and(|b| b.top() < top - px(2.))
    };
    let below = |row: usize| {
        row >= first
            && v.list
                .bounds_for_item(row)
                .is_none_or(|b| b.top() > top + px(2.))
    };
    let target = if down {
        prompts.iter().copied().find(|&r| below(r))
    } else {
        prompts.iter().rev().copied().find(|&r| above(r))
    };
    match target {
        Some(row) => v.follow.seek(&v.list, row),
        None if down => v.follow.down(&v.list),
        None => {}
    }
}

fn lit(v: &TranscriptView, prompts: &[usize]) -> usize {
    let (list, last) = (&v.list, prompts.len().saturating_sub(1));
    if v.follow.stick {
        return last;
    }
    let max = list.max_offset_for_scrollbar().y;
    let y = -list.scroll_px_offset_for_scrollbar().y;
    if max <= px(4.) || y >= max - px(4.) {
        return last;
    }
    if y <= px(4.) {
        return 0;
    }
    let view = list.viewport_bounds();
    let line = view.top() + view.size.height / 3.;
    let top = list.logical_scroll_top().item_ix;
    prompts
        .iter()
        .rposition(|&row| row < top || list.bounds_for_item(row).is_some_and(|b| b.top() <= line))
        .unwrap_or(0)
}

/// One tick per prompt on the left edge: the one for the part being read lit, the ones near the
/// pointer longer the closer they are, and a card with the prompt and the start of its reply
/// beside the hovered one. A click scrolls to its prompt. `prompts` are the prompts' rows.
fn ticks(
    v: &TranscriptView,
    prompts: &[usize],
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> Option<AnyElement> {
    // two prompts at least, and room beside the column so the ticks don't sit on the text
    if prompts.len() < 2 || v.list.viewport_bounds().size.width < px(COLUMN + 80.) {
        return None;
    }
    // positions come from the last layout, and a scroll set during this one (to the bottom, say)
    // only shows in the next, so check again then and redraw if the lit tick moved
    let lit = lit(v, prompts);
    let rows = prompts.to_vec();
    cx.on_next_frame(window, move |v, _, cx| {
        if self::lit(v, &rows) != lit {
            cx.notify();
        }
    });
    let height = f32::from(v.list.viewport_bounds().size.height) / crate::zoom::zoom();
    let (shown, pitch) = fit_ticks(prompts.len(), height - 48.);
    let last = prompts.len() - 1;
    let prompt_of = |n: usize| (n * last + (shown - 1) / 2) / (shown - 1);
    let lit = (lit * (shown - 1) + last / 2) / last;
    let hover = v.hover_tick.get().filter(|&h| h < shown);
    let previews = previews(&v.model.items);
    let mut glides = v.rail.borrow_mut();
    glides.resize_with(shown, Glide::default);
    let marks: Vec<AnyElement> = (0..shown)
        .map(|n| {
            let row = prompts[prompt_of(n)];
            let near = hover.map(|h| h.abs_diff(n));
            let width = match near {
                Some(0) => 18.,
                Some(1) => 13.,
                Some(2) => 10.,
                _ if n == lit => 12.,
                _ => 6.,
            };
            let bar = glides[n].toward(width).apply(
                "tick-bar",
                div().h(px(1.5)).rounded_full(),
                |d, w| {
                    let k = ((w - 6.) / 12.).clamp(0., 1.);
                    d.w(px(w)).bg(colors::text1().opacity(0.3 + 0.7 * k))
                },
            );
            div()
                .id(("tick", n))
                .flex()
                .items_center()
                .w(px(24.))
                .h(px(pitch))
                .cursor_pointer()
                .on_hover(cx.listener(move |v, on: &bool, _, cx| {
                    if *on {
                        v.hover_tick.set(Some(n));
                    } else if v.hover_tick.get() == Some(n) {
                        v.hover_tick.set(None);
                    }
                    cx.notify();
                }))
                .child(bar)
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                    v.follow.seek(&v.list, row);
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect();
    let card = hover.map(|n| {
        let (title, reply) = previews.get(prompt_of(n)).cloned().unwrap_or_default();
        let column = shown as f32 * pitch;
        let start = (height - column) / 2.;
        let top = (start + n as f32 * pitch - 18.).min(height - 120.).max(8.) - start;
        div()
            .absolute()
            .left(px(30.))
            .top(px(top))
            .w(px(320.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(14.))
            .py(px(11.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .child(
                div()
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(title),
            )
            .when(!reply.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.5))
                        .line_height(relative(1.5))
                        .text_color(colors::text3())
                        .line_clamp(3)
                        .child(reply),
                )
            })
            .with_animation(
                "tick-card",
                Animation::new(Duration::from_millis(120)).with_easing(gpui::ease_in_out),
                |d, t| d.opacity(t),
            )
    });
    let seen = pitch >= 9.
        || v.rail_near.get()
        || hover.is_some()
        || v.rail_until.is_some_and(|u| Instant::now() < u);
    let shown = fade_rail(v, seen, window, cx);
    let column = div()
        .relative()
        .flex()
        .flex_col()
        .children(marks)
        .children(card);
    Some(
        div()
            .id("rail")
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(56.))
            .pl(px(10.))
            .flex()
            .items_center()
            .on_hover(cx.listener(|v, on: &bool, _, cx| {
                v.rail_near.set(*on);
                cx.notify();
            }))
            .child(column.opacity(shown * shown * (3. - 2. * shown)))
            .into_any_element(),
    )
}

fn stir_rail(v: &mut TranscriptView, dy: gpui::Pixels, cx: &mut Context<TranscriptView>) {
    let now = Instant::now();
    let (sum, last) = v.rail_stir;
    let sum = match last {
        Some(l) if now - l < Duration::from_millis(400) => sum + dy.abs(),
        _ => dy.abs(),
    };
    v.rail_stir = (sum, Some(now));
    if sum < v.list.viewport_bounds().size.height / 2. {
        return;
    }
    let linger = Duration::from_millis(900);
    v.rail_until = Some(now + linger);
    v.rail_timer = Some(cx.spawn(async move |this, cx| {
        cx.background_executor().timer(linger).await;
        this.update(cx, |_, cx| cx.notify()).ok();
    }));
}

fn fade_rail(
    v: &TranscriptView,
    seen: bool,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> f32 {
    let goal = if seen { 1. } else { 0. };
    let (was, last) = v.rail_fade.get();
    let now = Instant::now();
    if !slide::animations() || was == goal {
        v.rail_fade.set((goal, None));
        return goal;
    }
    let dt = last.map_or(0.016, |l| (now - l).as_secs_f32().min(0.05));
    let step = dt / if seen { 0.18 } else { 0.3 };
    let shown = if seen {
        (was + step).min(goal)
    } else {
        (was - step).max(goal)
    };
    v.rail_fade.set((shown, Some(now)));
    crate::pace::next(window, cx);
    shown
}

fn fit_ticks(prompts: usize, room: f32) -> (usize, f32) {
    let shown = prompts.min((room / 4.).floor().max(2.) as usize);
    (shown, (room / shown as f32).clamp(4., 9.))
}

fn previews(items: &[Item]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for item in items {
        match item {
            Item::User { text, .. } => {
                let first = text
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("An image");
                out.push((first.to_string(), String::new()));
            }
            Item::Text { source, .. } => {
                if let Some((_, reply)) = out.last_mut()
                    && reply.is_empty()
                {
                    *reply = plain(source);
                }
            }
            _ => {}
        }
    }
    out
}

fn plain(source: &str) -> String {
    let text: String = source
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', '>', ' ']))
        .filter(|l| !l.is_empty() && !l.starts_with("```"))
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['*', '`'], "");
    text.chars().take(240).collect()
}

/// A reply still coming in: being revealed, or the last thing a live run has said.
fn streaming(v: &TranscriptView, ix: usize) -> bool {
    v.partial.as_ref().is_some_and(|(p, _)| *p == ix)
        || (v.model.elapsed().is_some() && ix + 1 == v.model.items.len())
}

fn hover_copy(ix: usize, text: &str) -> gpui::Div {
    div()
        .flex()
        .opacity(0.)
        .group_hover(SharedString::from(format!("msg-{ix}")), |s| s.opacity(1.))
        .child(widgets::CopyButton::new(
            format!("copy-msg-{ix}"),
            text.to_string(),
        ))
}

fn item(
    v: &TranscriptView,
    ix: usize,
    item: &Item,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    match item {
        Item::User {
            text,
            images,
            steer,
        } => div()
            .flex()
            .flex_col()
            .items_end()
            .gap_1()
            .when(*steer, |d| {
                d.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child("Steer"),
                )
            })
            .group(SharedString::from(format!("msg-{ix}")))
            .child(
                div()
                    .id(("bubble", ix))
                    .on_hover(cx.listener(move |v, over: &bool, _, cx| {
                        if !over {
                            v.light(ix, None, cx);
                        }
                    }))
                    .max_w(relative(0.8))
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(18.))
                    .bg(colors::ink(0.06))
                    .text_size(px(14.))
                    .line_height(relative(1.6))
                    .text_color(colors::text1())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(!text.is_empty(), |d| {
                        let long = text.lines().count() > 10 || text.len() > 900;
                        let open = v.expanded.contains(&ix);
                        let body = SharedString::from(text.clone());
                        let chip = HighlightStyle {
                            background_color: Some(colors::ink(0.1)),
                            ..Default::default()
                        };
                        let marks = attach::markers(text);
                        let cwd = v.model.cwd.clone().unwrap_or_else(|| v.launch.cwd.clone());
                        let words = crate::suggest::chip_words(v.launch.agent, &cwd, cx);
                        let mut lit: Vec<_> = marks.iter().map(|(r, _)| r.clone()).collect();
                        lit.extend(crate::input::chip_spans(text, &words, true));
                        lit.sort_by_key(|r| r.start);
                        let styled = StyledText::new(body.clone())
                            .with_highlights(lit.into_iter().map(|r| (r, chip)));
                        let layout = styled.layout().clone();
                        let shown = if marks.is_empty() {
                            styled.into_any_element()
                        } else {
                            let view = cx.entity().downgrade();
                            InteractiveText::new(("marks", ix), styled)
                                .on_hover(move |at, _, _, cx| {
                                    let n = at.and_then(|at| {
                                        marks.iter().find(|(r, _)| r.contains(&at)).map(|m| m.1)
                                    });
                                    let _ = view.update(cx, |v, cx| v.light(ix, n, cx));
                                })
                                .into_any_element()
                        };
                        let shown = div()
                            .when(long && !open, |d| d.max_h(px(220.)).overflow_hidden())
                            .child(markdown::select::wrap(
                                &format!("u{ix}"),
                                body,
                                layout,
                                shown,
                            ));
                        d.child(shown).when(long, |d| {
                            d.child(
                                div()
                                    .id(("show-full", ix))
                                    .text_size(px(12.))
                                    .text_color(colors::text3())
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(colors::text1()))
                                    .child(if open {
                                        "Show less"
                                    } else {
                                        "Show full message"
                                    })
                                    .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                                        if !v.expanded.remove(&ix) {
                                            v.expanded.insert(ix);
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                    })
                    .when(!images.is_empty(), |d| {
                        let lit = v.lit.filter(|(row, _)| *row == ix).map(|(_, n)| n);
                        d.child(div().flex().flex_wrap().gap_2().children(
                            images.iter().enumerate().map(|(k, p)| {
                                let path = p.clone();
                                div()
                                    .id(SharedString::from(format!("sent-img-{ix}-{k}")))
                                    .when(lit.is_some_and(|n| n != k + 1), |d| d.opacity(0.3))
                                    .on_hover(cx.listener(move |v, over: &bool, _, cx| {
                                        if *over {
                                            v.light(ix, None, cx);
                                        }
                                    }))
                                    .child(attach::thumb(p, 120.))
                                    .when(attach::previews(p) && p.exists(), |d| {
                                        d.cursor_pointer().on_click(cx.listener(
                                            move |_, _: &ClickEvent, _, cx| {
                                                cx.emit(TranscriptEvent::ShowMedia(path.clone()))
                                            },
                                        ))
                                    })
                            }),
                        ))
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .opacity(0.)
                    .group_hover(SharedString::from(format!("msg-{ix}")), |s| s.opacity(1.))
                    .when(!text.is_empty(), |d| {
                        d.child(widgets::CopyButton::new(
                            format!("copy-msg-{ix}"),
                            text.to_string(),
                        ))
                    })
                    .when(!*steer && v.can_rewind(), |d| {
                        d.child(
                            widgets::icon_button(("fork", ix), "git-branch", 24.)
                                .tooltip(widgets::tip("Fork from here into a new thread"))
                                .on_click(
                                    cx.listener(move |v, _: &ClickEvent, _, cx| v.fork(ix, cx)),
                                ),
                        )
                        .child(
                            widgets::icon_button(("rewind", ix), "pencil", 24.)
                                .tooltip(widgets::tip("Edit from here"))
                                .on_click(
                                    cx.listener(move |v, _: &ClickEvent, _, cx| v.rewind(ix, cx)),
                                ),
                        )
                    }),
            )
            .into_any_element(),
        Item::Text { .. } => reply(v, ix, 0),
        Item::Thinking { text, open } => {
            let head = fold_head(
                ("thinking", ix),
                *open,
                "Thinking".into(),
                None,
                cx,
                move |v| {
                    if let Some(Item::Thinking { open, .. }) = v.model.items.get_mut(ix) {
                        *open = !*open;
                    }
                },
            );
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(head)
                .when(*open || folding(cx, ("thinking", ix)), |d| {
                    d.child(unfold(
                        cx,
                        ("thinking", ix),
                        *open,
                        div()
                            .pl(px(26.))
                            .text_size(px(12.5))
                            .line_height(relative(1.55))
                            .italic()
                            .text_color(colors::text3())
                            .child(text.trim().to_string()),
                    ))
                })
                .into_any_element()
        }
        Item::Tool {
            tool,
            done,
            open,
            images,
            live,
            ..
        } if super::ask::special(&v.model.items[ix]) => match super::ask::named(tool) {
            Some((super::ask::PLAN, input)) => {
                super::ask::plan_card(v, None, input, None, false, ix, cx)
            }
            Some((super::ask::TODO, input)) => super::ask::todo_card(input),
            _ => main_tool(ix, tool, done.as_ref(), *open, images, live, cx),
        },
        Item::Tool {
            tool,
            done,
            open,
            images,
            live,
            ..
        } => main_tool(ix, tool, done.as_ref(), *open, images, live, cx),
        Item::Agent(a) => super::subagent::card(ix, a, cx),
        Item::Approval {
            request,
            tool,
            reason,
            always,
            answer,
            expired,
        } => super::approval::card(
            v,
            request,
            tool,
            reason.as_deref(),
            *always,
            *answer,
            *expired,
            cx,
        ),
        Item::Error(message) => {
            // a model the account can't use is the error people hit most; say how to get past it
            let hint = message.to_lowercase().contains("model").then(|| {
                div()
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .child("Pick another model with the model button below, then send again.")
            });
            div()
                .flex()
                .gap(px(10.))
                .px(px(14.))
                .py(px(11.))
                .rounded(px(12.))
                .bg(colors::error().opacity(0.08))
                .text_size(px(13.))
                .child(
                    div()
                        .pt(px(2.))
                        .child(icon("circle-alert", 14., colors::error())),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_color(colors::text1())
                        .child(message.clone())
                        .children(hint)
                        .children(retry(v, ix, cx)),
                )
                .into_any_element()
        }
        Item::Finished { status, ms, error } => {
            let secs = *ms as f32 / 1000.0;
            let line = match status {
                RunStatus::Done => format!("Worked for {}", took(ms / 1000)),
                RunStatus::Interrupted => format!("Stopped after {secs:.1}s"),
                RunStatus::Failed => match error {
                    Some(e) => format!("Failed: {e}"),
                    None => format!("Failed after {secs:.1}s"),
                },
            };
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(11.5))
                .text_color(if *status == RunStatus::Failed {
                    colors::error()
                } else {
                    colors::text3()
                })
                .child(line)
                .children(retry(v, ix, cx))
                .into_any_element()
        }
        Item::Note(text) => div()
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(px(11.5))
            .text_color(colors::text3())
            .child(text.clone())
            .when(v.cut == Some(ix) && !v.model.running(), |d| {
                d.child(
                    widgets::button("resume-cut", "Resume")
                        .h(px(24.))
                        .px(px(9.))
                        .text_size(px(12.))
                        .child(icon("rotate-cw", 12., colors::text2()))
                        .flex_row_reverse()
                        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.resume_cut(cx))),
                )
            })
            .into_any_element(),
        Item::Changes {
            files, all, undo, ..
        } => super::changes::card(v, ix, files, *all, undo, cx),
    }
}

/// A muted clickable line led by a fold chevron. `toggle` flips
/// what it opens.
pub(super) fn fold_head(
    id: impl Into<gpui::ElementId>,
    open: bool,
    label: SharedString,
    right: Option<AnyElement>,
    cx: &mut Context<TranscriptView>,
    toggle: impl Fn(&mut TranscriptView) + 'static,
) -> AnyElement {
    let id: ElementId = id.into();
    let key = fold_key(cx, &id);
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .text_size(px(13.))
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text2()))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(18.))
                .child(twist(&key, open)),
        )
        .child(div().min_w_0().truncate().child(label))
        .children(right)
        .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
            slide::flip(key.clone());
            toggle(v);
            settle_fold(cx);
            cx.notify();
        }))
        .into_any_element()
}

pub(super) fn fold_key(cx: &Context<TranscriptView>, id: &ElementId) -> String {
    format!("{}:{id:?}", cx.entity_id())
}

pub(super) fn twist(key: &str, open: bool) -> AnyElement {
    let turn = move |t: f32| percentage(if open { 0.25 * t } else { 0.25 * (1. - t) });
    let chevron = icon("chevron-right", 12., colors::text3());
    match slide::flipped(key, UNFOLD) {
        Some(n) => chevron
            .with_animation(
                ("twist", n),
                Animation::new(UNFOLD).with_easing(ease_out),
                move |c, t| c.with_transformation(Transformation::rotate(turn(t))),
            )
            .into_any_element(),
        None => chevron
            .with_transformation(Transformation::rotate(turn(1.)))
            .into_any_element(),
    }
}

thread_local! {
    /// Each fold body's natural height as last drawn, for growing it open and shrinking it shut.
    static FOLD_HEIGHTS: std::cell::RefCell<std::collections::HashMap<String, f32>> =
        std::cell::RefCell::default();
}

/// `body` under its fold header, growing open or shrinking shut while it fades after a toggle.
/// The parent's 6px gap opens and closes with it, so nothing snaps at either end.
pub(super) fn unfold(
    cx: &Context<TranscriptView>,
    id: impl Into<ElementId>,
    open: bool,
    body: impl IntoElement,
) -> AnyElement {
    let key = fold_key(cx, &id.into());
    let mark = key.clone();
    let measure = gpui::canvas(
        move |b, _, _| {
            let h = f32::from(b.size.height);
            FOLD_HEIGHTS.with(|m| m.borrow_mut().insert(mark, h));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    let body = div()
        .relative()
        .flex()
        .flex_col()
        .child(div().relative().flex_none().child(body).child(measure));
    let Some(n) = slide::flipped(&key, UNFOLD) else {
        return body.into_any_element();
    };
    // the height is read each frame: on a first opening it is known from the second frame on
    let shown = move |t: f32| if open { t } else { 1. - t };
    body.overflow_hidden()
        .with_animation(
            ("fold", n),
            Animation::new(UNFOLD).with_easing(ease_out),
            move |d, t| {
                let s = shown(t);
                let h = FOLD_HEIGHTS
                    .with(|m| m.borrow().get(&key).copied())
                    .unwrap_or(0.);
                d.opacity(s).mt(px(-6. * (1. - s))).h(gpui::px(h * s))
            },
        )
        .into_any_element()
}

pub(super) fn folding(cx: &Context<TranscriptView>, id: impl Into<ElementId>) -> bool {
    slide::flipped(&fold_key(cx, &id.into()), UNFOLD).is_some()
}

// an animation that ends asks for no frame after it, so the folded body would stay drawn
pub(super) fn settle_fold(cx: &mut Context<TranscriptView>) {
    cx.spawn(async move |this, cx| {
        cx.background_executor()
            .timer(UNFOLD + Duration::from_millis(30))
            .await;
        let _ = this.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

/// A run of tool calls as one line counting them, opening to the single calls.
fn tool_run(
    v: &TranscriptView,
    start: usize,
    end: usize,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let calls = &v.model.items[start..end];
    let tools = calls.iter().filter_map(|i| match i {
        Item::Tool { tool, .. } => Some(tool),
        _ => None,
    });
    let failed = calls
        .iter()
        .filter(|i| {
            matches!(
                i,
                Item::Tool {
                    done: Some((false, _)),
                    ..
                }
            )
        })
        .count();
    let live = calls
        .iter()
        .any(|i| matches!(i, Item::Tool { done: None, .. }));
    let open = v.open_runs.contains(&start);
    let right = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .flex_none()
        .when(failed > 0, |d| {
            d.child(
                div()
                    .text_color(colors::error())
                    .child(format!("{failed} failed")),
            )
        })
        .when(live && !open, |d| {
            d.child(spinner::ring(("run-live", start), colors::text3()))
        });
    let head = fold_head(
        ("tool-run", start),
        open,
        tool::summary(tools).into(),
        Some(right.into_any_element()),
        cx,
        move |v| {
            if !v.open_runs.remove(&start) {
                v.open_runs.insert(start);
            }
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(head)
        .when(open || folding(cx, ("tool-run", start)), |d| {
            let calls: Vec<AnyElement> = (start..end)
                .filter_map(|ix| match &v.model.items[ix] {
                    Item::Tool {
                        tool,
                        done,
                        open,
                        images,
                        live,
                        ..
                    } => Some(main_tool(ix, tool, done.as_ref(), *open, images, live, cx)),
                    _ => None,
                })
                .collect();
            d.child(unfold(
                cx,
                ("tool-run", start),
                open,
                div()
                    .pl(px(26.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(calls),
            ))
        })
        .into_any_element()
}

fn main_tool(
    ix: usize,
    t: &Tool,
    done: Option<&(bool, String)>,
    open: bool,
    images: &[PathBuf],
    live: &str,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let key: SharedString = format!("tool-{ix}").into();
    let card = tool_card(key.clone(), t, done, live, open, cx, move |v| {
        if let Some(Item::Tool { open, .. }) = v.model.items.get_mut(ix) {
            *open = !*open;
        }
    });
    with_pictures(card, &key, open, images, cx)
}

/// `card` with the pictures its tool gave back under it while it's open, each opening full size
/// on a click.
pub(super) fn with_pictures(
    card: AnyElement,
    key: &str,
    open: bool,
    images: &[PathBuf],
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    if !open || images.is_empty() {
        return card;
    }
    let shown = images.iter().enumerate().map(|(k, p)| {
        let path = p.clone();
        div()
            .id(SharedString::from(format!("{key}-pic-{k}")))
            .cursor_pointer()
            .child(attach::thumb(p, 120.))
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                cx.emit(TranscriptEvent::ShowMedia(path.clone()))
            }))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(card)
        .child(div().pl(px(26.)).flex().flex_wrap().gap_2().children(shown))
        .into_any_element()
}

/// One tool call as a muted line that opens to its input and output. `key` keeps its element
/// ids apart from every other call's; `toggle` flips where its open state lives.
pub(super) fn tool_card(
    key: SharedString,
    t: &Tool,
    done: Option<&(bool, String)>,
    live: &str,
    open: bool,
    cx: &mut Context<TranscriptView>,
    toggle: impl Fn(&mut TranscriptView) + 'static,
) -> AnyElement {
    let state = match done {
        None => Some(spinner::ring((key.clone(), 1), colors::text3())),
        Some((false, _)) => Some(
            div()
                .text_color(if matches!(t, Tool::Edit { .. }) {
                    colors::error()
                } else {
                    colors::text3()
                })
                .child("Failed")
                .into_any_element(),
        ),
        Some((true, _)) => None,
    };
    let counts = match t {
        Tool::Edit { changes } => {
            let (a, d) = tool::counts(changes);
            Some(
                div()
                    .flex()
                    .gap_1()
                    .child(div().text_color(colors::diff_add()).child(format!("+{a}")))
                    .child(div().text_color(colors::diff_del()).child(format!("-{d}"))),
            )
        }
        _ => None,
    };
    let shown = done.map_or(live, |(_, out)| out.as_str());
    let body_id: ElementId = (key.clone(), 0usize).into();
    let head = fold_head(
        (key, 0),
        open,
        tool::label(t).into(),
        Some(
            div()
                .flex()
                .gap(px(6.))
                .items_center()
                .flex_none()
                .children(counts)
                .children(state)
                .into_any_element(),
        ),
        cx,
        toggle,
    );
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(head)
        .when(open || folding(cx, body_id.clone()), |d| {
            d.child(unfold(
                cx,
                body_id,
                open,
                div()
                    .pl(px(26.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(tool::input(t).map(|i| tool::mono(&i)))
                    .when_some(
                        match t {
                            Tool::Edit { changes } => Some(tool::diffs(changes)),
                            _ => None,
                        },
                        |d, diffs| d.child(diffs),
                    )
                    .when(!shown.trim().is_empty(), |d| d.child(tool::mono(shown))),
            ))
        })
        .into_any_element()
}

/// The reply box: one pill with attach on its left, the prompt, and the model and send on its
/// right. Under it, the folder and its branch.
fn composer(
    v: &TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let running = v.model.running();
    let agent = v.launch.agent;
    let (brand, _) = colors::brand(agent);
    let focused = v.input.focus_handle(cx).is_focused(window);
    // a quiet chip that lifts on hover
    let chip = |id: &'static str| {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(6.))
            .h(px(26.))
            .px(px(8.))
            .min_w_0()
            .rounded(px(8.))
            .text_size(px(12.5))
            .cursor_pointer()
            .hover(|s| s.bg(colors::ink(0.06)))
    };
    let model_chip = model_menu::anchored_chip(
        &v.anchor,
        chip("thread-model")
            .child(mark(agent, 13., brand))
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text1())
                    .child(v.model_label()),
            )
            .on_click(cx.listener(|v, _: &ClickEvent, window, cx| {
                if let Some(spec) = v.model_spec() {
                    v.menu = Some(ModelMenu::open(&spec, window, cx));
                    cx.notify();
                }
            })),
    );
    let effort_chip = v
        .model_spec()
        .filter(|s| !s.efforts.is_empty() || s.long.is_some() || s.fast.is_some())
        .map(|spec| {
            model_menu::anchored_chip(
                &v.effort_anchor,
                chip("thread-effort")
                    .text_color(colors::text3())
                    .child(model_menu::effort_chip_label(&spec))
                    .on_click(cx.listener(|v, _: &ClickEvent, window, cx| {
                        if let Some(spec) = v.model_spec() {
                            v.menu = Some(ModelMenu::open_effort(&spec, window, cx));
                            cx.notify();
                        }
                    })),
            )
        });
    // a live run with nothing typed can only be stopped; typed text steers it
    let action = if running && v.empty && v.images.is_empty() {
        widgets::stop("stop")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.interrupt(cx)))
    } else {
        widgets::send("send", "arrow-up")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.submit(cx)))
    };
    // the prompt across the whole width, and under it the model, effort and permission on the
    // left with attach and send on the right
    let pill = div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(14.))
        .border_1()
        .border_color(if focused {
            colors::ink(0.12)
        } else {
            gpui::transparent_black()
        })
        .bg(colors::ink(0.045))
        // files held over the thread land here
        .drag_over::<ExternalPaths>(|s, _, _, _| {
            s.border_color(colors::accent())
                .bg(colors::accent().opacity(0.06))
        })
        .children(attach::peek(&v.images, v.peek))
        .when(!v.images.is_empty(), |d| {
            d.child(div().px(px(14.)).pt(px(12.)).child(attach::tray(
                "thread-img",
                &v.images,
                cx.listener(|_, path: &PathBuf, _, cx| {
                    cx.emit(TranscriptEvent::ShowMedia(path.clone()))
                }),
                cx.listener(|v, ix: &usize, _, cx| {
                    if *ix < v.images.len() {
                        v.images.remove(*ix);
                        attach::unmark(&v.input, *ix, cx);
                        v.keep_draft(cx);
                    }
                    cx.notify();
                }),
            )))
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .px(px(8.))
                .pt(px(12.))
                .pb(px(8.))
                .child(
                    div()
                        .px(px(8.))
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .child(v.input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .gap(px(4.))
                                .child(model_chip)
                                .children(effort_chip)
                                .child(permission_chip(v, cx)),
                        )
                        .child(
                            widgets::icon_button("thread-attach", "paperclip", 28.).on_click(
                                cx.listener(|v, _: &ClickEvent, _, cx| v.pick_images(cx)),
                            ),
                        )
                        .child(action),
                ),
        )
        .when(v.ultra && agent == hyprspace_proto::Agent::Claude, |d| {
            d.relative()
                .child(super::ring::ring(14., focused && window.is_window_active()))
        });
    let foot = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .px(px(12.))
        .pt(px(8.))
        .text_size(px(11.5))
        .text_color(colors::text3())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .child(icon("folder", 12., colors::text3()))
                .child(div().truncate().child(folder_name(v))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .min_w_0()
                .children(v.branch.clone().map(|b| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .min_w_0()
                        .child(icon("git-branch", 12., colors::text3()))
                        .child(div().truncate().child(b))
                }))
                .children(v.model.context.map(context)),
        );
    let suggestions = v.suggest.open.as_ref().map(|open| {
        let view = cx.entity().downgrade();
        crate::suggest::render(open, false, move |ix, _, cx| {
            if let Some(v) = view.upgrade() {
                v.update(cx, |v, cx| v.take_suggestion(ix, cx));
            }
        })
    });
    let pill = div()
        .relative()
        .w_full()
        .when(v.suggest.open.is_some(), |d| d.key_context("Suggest"))
        .capture_action(cx.listener(|v, _: &crate::input::Up, window, cx| {
            if !v.queue.is_empty() && v.input.read(cx).text().is_empty() {
                v.edit_queued(v.queue.len() - 1, window, cx);
                cx.stop_propagation();
            }
        }))
        .on_action(cx.listener(TranscriptView::suggest_prev))
        .on_action(cx.listener(TranscriptView::suggest_next))
        .on_action(cx.listener(TranscriptView::suggest_accept))
        .on_action(cx.listener(TranscriptView::suggest_dismiss))
        .child(pill)
        .children(suggestions);
    let tasks = super::ask::progress(&v.model.items)
        .filter(|_| running)
        .map(|(done, total, now)| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .pb(px(8.))
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(icon("list-checks", 12., colors::text3()))
                .child(
                    div()
                        .flex_none()
                        .text_color(colors::text2())
                        .child(format!("Tasks {done} of {total}")),
                )
                .when(!now.is_empty(), |d| {
                    d.child("\u{b7}")
                        .child(div().min_w_0().truncate().child(now))
                })
        });
    centered(
        column()
            .pb(px(12.))
            .children(tray(v, cx))
            .children(tasks)
            .child(pill)
            .child(foot),
    )
    .into_any_element()
}

fn tray(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Option<AnyElement> {
    let held = !v.model.running();
    let small = |b: gpui::Stateful<Div>| b.h(px(24.)).px(px(9.)).text_size(px(12.));
    let line = |lead: AnyElement, text: String| {
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .min_h(px(40.))
            .py(px(6.))
            .child(lead)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .text_color(colors::text2())
                    .child(text),
            )
    };
    let mut rows: Vec<gpui::Stateful<Div>> = Vec::new();
    if let Some(at) = v.resume_at {
        let when = chrono::Local
            .timestamp_millis_opt(at as i64)
            .single()
            .map(|t| t.format("%-I:%M %p").to_string())
            .unwrap_or_default();
        rows.push(
            line(
                icon("clock", 14., colors::busy()).into_any_element(),
                format!(
                    "{} hit its usage limit. It continues at {when}.",
                    v.launch.agent.name()
                ),
            )
            .child(
                small(widgets::button("limit-now", "Continue now"))
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.continue_now(cx))),
            )
            .child(
                widgets::icon_button("limit-cancel", "x", 24.)
                    .tooltip(widgets::tip("Don't continue"))
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.cancel_resume(cx))),
            )
            .id("limit-row"),
        );
    }
    for (ix, task) in v.model.background.iter().enumerate() {
        let (glyph, kind) = match task.kind {
            TaskKind::Command => ("terminal", "Background command"),
            TaskKind::Monitor => ("gauge", "Monitor"),
            TaskKind::Subagent => ("bot", "Background subagent"),
            TaskKind::Other => ("zap", "Background task"),
        };
        let label = match task.label.trim() {
            "" => kind.to_string(),
            l => l.to_string(),
        };
        let (id, open) = (task.id.clone(), v.task_open.as_ref() == Some(&task.id));
        let toggle = task.id.clone();
        rows.push(
            line(icon(glyph, 14., colors::text3()).into_any_element(), label)
                .child(
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(colors::text3())
                        .child(kind),
                )
                .child(icon(
                    if open { "chevron-up" } else { "chevron-down" },
                    12.,
                    colors::text3(),
                ))
                .child(
                    small(widgets::button(("task-stop", ix), "Stop"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .tooltip(widgets::tip("Stop this task"))
                        .on_click(
                            cx.listener(move |v, _: &ClickEvent, _, cx| {
                                v.stop_task(id.clone(), cx)
                            }),
                        ),
                )
                .id(("task-row", ix))
                .cursor_pointer()
                .on_click(
                    cx.listener(move |v, _: &ClickEvent, _, cx| v.toggle_task(toggle.clone(), cx)),
                ),
        );
        if open {
            rows.push(super::background::panel(v, task, ix, cx));
        }
    }
    for (ix, p) in v.queue.iter().enumerate() {
        let text = p.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text = if text.is_empty() {
            format!("{} image(s)", p.images.len())
        } else {
            text
        };
        let shown = SharedString::from(text.clone());
        let row = line(
            icon("list-checks", 14., colors::text3()).into_any_element(),
            text,
        )
        .child(
            small(widgets::button(("queue-edit", ix), "Edit"))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .tooltip(widgets::tip(
                    "Take it out of the queue and back into the box",
                ))
                .on_click(
                    cx.listener(move |v, _: &ClickEvent, window, cx| v.edit_queued(ix, window, cx)),
                ),
        )
        .child(
            small(widgets::button(
                ("queue-steer", ix),
                if held { "Send" } else { "Steer" },
            ))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .tooltip(widgets::tip(if held {
                "Send this now"
            } else {
                "Send this into the run now instead of after it"
            }))
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.steer(ix, cx))),
        )
        .child(
            widgets::icon_button(("queue-drop", ix), "x", 24.)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .tooltip(widgets::tip("Remove from the queue"))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.unqueue(ix, cx))),
        )
        .id(("queue-row", ix));
        rows.push(if v.queue.len() > 1 {
            row.on_drag(QueueDrag { ix, text: shown }, |d, grab, _, cx| {
                cx.new(|_| QueueGhost {
                    text: d.text.clone(),
                    grab,
                })
            })
            .drag_over::<QueueDrag>(|s, _, _, _| s.bg(colors::accent().opacity(0.1)))
            .on_drop(cx.listener(move |v, d: &QueueDrag, _, cx| v.move_queued(d.ix, ix, cx)))
        } else {
            row
        });
    }
    if rows.is_empty() {
        return None;
    }
    Some(
        div()
            .mx(px(12.))
            .px(px(12.))
            .rounded_t(px(12.))
            .bg(colors::ink(0.035))
            .children(
                rows.into_iter().enumerate().map(|(i, r)| {
                    r.when(i > 0, |d| d.border_t_1().border_color(colors::border1()))
                }),
            )
            .into_any_element(),
    )
}

struct QueueDrag {
    ix: usize,
    text: SharedString,
}

struct QueueGhost {
    text: SharedString,
    grab: gpui::Point<gpui::Pixels>,
}

impl gpui::Render for QueueGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .pl((self.grab.x - px(16.)).max(px(0.)))
            .pt((self.grab.y - px(14.)).max(px(0.)))
            .child(
                div()
                    .max_w(px(320.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors::border2())
                    .bg(colors::surface3())
                    .shadow(colors::shadow())
                    .text_size(px(13.))
                    .text_color(colors::text1())
                    .truncate()
                    .child(self.text.clone()),
            )
    }
}

/// How full the context window is, as a ring and a percent the way zeron shows it under its
/// composer. It warms as the window fills, since a full one makes the CLI compact.
fn context((used, window): (u64, u64)) -> AnyElement {
    let pct = (used as f32 / window.max(1) as f32 * 100.).clamp(0., 100.);
    let color = match pct {
        p if p >= 90. => colors::error(),
        p if p >= 75. => colors::busy(),
        _ => colors::text2(),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .child(crate::usage::ring(pct, color))
        .child(format!("{}% context", pct.round()))
        .into_any_element()
}

fn permission_chip(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> AnyElement {
    use hyprspace_proto::Permission;
    let p = v.launch.permission;
    let glyph = match p {
        Permission::Plan => "list-checks",
        Permission::Ask => "hand",
        Permission::Auto => "file-pen-line",
        Permission::Bypass => "lock-open",
    };
    div()
        .id("thread-permission")
        .flex()
        .items_center()
        .gap(px(5.))
        .h(px(26.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(px(12.5))
        .text_color(colors::text3())
        .cursor_pointer()
        .hover(|s| s.bg(colors::ink(0.06)))
        .on_click(cx.listener(|v, e: &ClickEvent, _, cx| {
            let at = e.position();
            v.perm_menu = Some(gpui::point(at.x - px(40.), at.y));
            cx.notify();
        }))
        .child(icon(glyph, 12., colors::text3()))
        .child(crate::composer::pickers::permission_label(p))
        .into_any_element()
}

pub(super) fn permission_menu(
    v: &TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> Option<AnyElement> {
    use hyprspace_proto::Permission;
    let at = v.perm_menu?;
    let rows = div().w(px(280.)).flex().flex_col().gap(px(1.)).children(
        [
            Permission::Plan,
            Permission::Ask,
            Permission::Auto,
            Permission::Bypass,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, mode)| {
            widgets::menu_item(
                ("thread-perm", i),
                crate::composer::pickers::permission_label(mode),
                Some(crate::composer::pickers::permission_note(mode).into()),
                mode == v.launch.permission,
            )
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.pick_permission(mode, cx)))
        }),
    );
    let close = cx.listener(|v, _: &(), _, cx| {
        v.perm_menu = None;
        cx.notify();
    });
    Some(widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        rows,
    ))
}

fn model_menu(
    v: &TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> Option<AnyElement> {
    let spec = v.model_spec()?;
    let menu = v.menu.as_ref()?;
    let anchor = if menu.is_effort() {
        &v.effort_anchor
    } else {
        &v.anchor
    };
    model_menu::render(menu, &spec, anchor, window, cx)
}
