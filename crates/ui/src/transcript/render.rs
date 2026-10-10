// Draws a transcript after zeron's: one centered column, the user's prompts as bubbles on the
// right, replies as markdown, thinking and each run of tool calls folded into one muted line
// until clicked, approval prompts with their buttons, and the pill-shaped box to reply or steer.

use chrono::TimeZone;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, Div, ElementId, ExternalPaths,
    Focusable, FontWeight, IntoElement, MouseButton, ScrollWheelEvent, SharedString, StyledText,
    Transformation, Window, div, list, percentage, prelude::*, px, relative,
};
use hyprspace_proto::{RunStatus, Tool};

use super::model::Item;
use super::rows::{self, Row};
use super::{TranscriptView, tool};
use crate::assets::{icon, mark};
use crate::composer::model_menu::{self, Host as _, ModelMenu};
use crate::pace::Looping;
use crate::slide::{self, Glide, ease_out};
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
    if v._focus.is_empty() {
        let input = v.input.focus_handle(cx);
        v._focus = vec![
            cx.on_focus(&input, window, |_, _, cx| cx.notify()),
            cx.on_blur(&input, window, |_, _, cx| cx.notify()),
        ];
    }
    v.follow.step(&v.list, window, cx);
    // rows measure taller than estimated once laid out, which only shows after this frame
    cx.on_next_frame(window, |v, _, cx| {
        if v.follow.behind(&v.list) {
            cx.notify();
        }
    });
    v.partial = v
        .reveal
        .as_mut()
        .and_then(|r| match v.model.items.get(r.ix) {
            Some(Item::Text { source, .. }) => r
                .step(source, window, cx)
                .map(|end| (r.ix, markdown::parse(&source[..end]))),
            _ => None,
        });
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
    if v.working.is_some_and(|at| at.elapsed() < WORKING_IN) {
        crate::pace::next(window, cx);
    }
    let rows = rows::plan(&v.model.items, v.loading, live);
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
    let jump = v.follow.away(&v.list).then(|| jump(cx));
    let ticks = ticks(v, &prompts, window, cx);
    let rows = list(
        v.list.clone(),
        cx.processor(|v, ix: usize, _, cx| list_row(v, ix, cx)),
    )
    .size_full();
    div()
        .id("transcript")
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
                    v.follow.wheel(e.delta.pixel_delta(px(20.)).y > px(0.));
                    cx.notify();
                }))
                // before any text, so selection knows this transcript's elements
                .child(markdown::select::reset(surface.clone()))
                .child(rows)
                .child(markdown::select::tail(surface))
                .children(ticks)
                .children(jump),
        )
        .child(composer(v, window, cx))
        .children(model_menu(v, window, cx))
        .children(permission_menu(v, window, cx))
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
    centered(column().child(body))
        .when(ix == 0, |d| d.pt(px(24.)))
        .pb(px(20.))
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

fn working(v: &TranscriptView) -> AnyElement {
    let secs = v.model.elapsed().unwrap_or(0);
    let took = if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    };
    let doing = v
        .model
        .running_tool()
        .filter(|_| !v.stopping)
        .map(tool::label);
    let label = div()
        .flex_none()
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(if v.stopping { "Stopping" } else { "Working" })
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
        .child(spinner::eclipse("working", colors::text2()))
        .child(label)
        .children(doing.map(|d| div().min_w_0().truncate().child(d)))
        .child(div().flex_1())
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
    let hover = v.hover_tick.get().filter(|&h| h < prompts.len());
    let previews = previews(&v.model.items);
    let mut glides = v.rail.borrow_mut();
    glides.resize_with(prompts.len(), Glide::default);
    let marks: Vec<AnyElement> = prompts
        .iter()
        .enumerate()
        .map(|(n, &row)| {
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
                .h(px(9.))
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
        let (title, reply) = previews.get(n).cloned().unwrap_or_default();
        div()
            .absolute()
            .left(px(30.))
            .top(px(n as f32 * 9. - 18.))
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
                ("tick-card", n),
                Animation::new(Duration::from_millis(120)).with_easing(gpui::ease_in_out),
                |d, t| d.opacity(t),
            )
    });
    Some(
        div()
            .absolute()
            .left(px(10.))
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .children(marks)
                    .children(card),
            )
            .into_any_element(),
    )
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
                        let styled = StyledText::new(body.clone());
                        let layout = styled.layout().clone();
                        let shown = div()
                            .when(long && !open, |d| d.max_h(px(220.)).overflow_hidden())
                            .child(markdown::select::wrap(
                                &format!("u{ix}"),
                                body,
                                layout,
                                styled.into_any_element(),
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
                        d.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(images.iter().map(|p| attach::thumb(p, 120.))),
                        )
                    }),
            )
            .when(!text.is_empty(), |d| d.child(hover_copy(ix, text)))
            .into_any_element(),
        Item::Text { blocks, source } => match v
            .partial
            .as_ref()
            .filter(|(p, _)| *p == ix)
            .map(|(_, b)| b)
            .or(blocks.as_ref())
        {
            Some(b) => div()
                .group(SharedString::from(format!("msg-{ix}")))
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_size(px(14.))
                .line_height(relative(1.65))
                .text_color(colors::text1())
                .child(markdown::render(b, &format!("t{ix}")))
                .child(hover_copy(ix, source))
                .into_any_element(),
            None => div().into_any_element(),
        },
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
                .when(*open, |d| {
                    d.child(unfold(
                        cx,
                        ("thinking", ix),
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
        Item::Tool { tool, .. } if super::ask::special(&v.model.items[ix]) => {
            match super::ask::named(tool) {
                Some((super::ask::PLAN, input)) => {
                    super::ask::plan_card(v, None, input, None, false, ix, cx)
                }
                Some((super::ask::TODO, input)) => super::ask::todo_card(input),
                _ => main_tool(ix, tool, None, false, cx),
            }
        }
        Item::Tool {
            tool, done, open, ..
        } => main_tool(ix, tool, done.as_ref(), *open, cx),
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
                        .children(hint),
                )
                .into_any_element()
        }
        Item::Finished { status, ms, error } => {
            let secs = *ms as f32 / 1000.0;
            let line = match status {
                // skipped in `items`
                RunStatus::Done => format!("Done in {secs:.1}s"),
                RunStatus::Interrupted => format!("Stopped after {secs:.1}s"),
                RunStatus::Failed => match error {
                    Some(e) => format!("Failed: {e}"),
                    None => format!("Failed after {secs:.1}s"),
                },
            };
            div()
                .text_size(px(11.5))
                .text_color(if *status == RunStatus::Failed {
                    colors::error()
                } else {
                    colors::text3()
                })
                .child(line)
                .into_any_element()
        }
        Item::Note(text) => div()
            .text_size(px(11.5))
            .text_color(colors::text3())
            .child(text.clone())
            .into_any_element(),
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

pub(super) fn unfold(
    cx: &Context<TranscriptView>,
    id: impl Into<ElementId>,
    body: impl IntoElement,
) -> AnyElement {
    let key = fold_key(cx, &id.into());
    let body = div().relative().child(body);
    match slide::flipped(&key, UNFOLD) {
        Some(n) => body
            .with_animation(
                ("unfold", n),
                Animation::new(UNFOLD).with_easing(ease_out),
                |d, t| d.opacity(t).top(px(-6. * (1. - t))),
            )
            .into_any_element(),
        None => body.into_any_element(),
    }
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
        .when(live, |d| {
            d.child(spinner::eclipse(("run-live", start), colors::text3()))
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
        .when(open, |d| {
            let calls: Vec<AnyElement> = (start..end)
                .filter_map(|ix| match &v.model.items[ix] {
                    Item::Tool {
                        tool, done, open, ..
                    } => Some(main_tool(ix, tool, done.as_ref(), *open, cx)),
                    _ => None,
                })
                .collect();
            d.child(unfold(
                cx,
                ("tool-run", start),
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
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    tool_card(format!("tool-{ix}").into(), t, done, open, cx, move |v| {
        if let Some(Item::Tool { open, .. }) = v.model.items.get_mut(ix) {
            *open = !*open;
        }
    })
}

/// One tool call as a muted line that opens to its input and output. `key` keeps its element
/// ids apart from every other call's; `toggle` flips where its open state lives.
pub(super) fn tool_card(
    key: SharedString,
    t: &Tool,
    done: Option<&(bool, String)>,
    open: bool,
    cx: &mut Context<TranscriptView>,
    toggle: impl Fn(&mut TranscriptView) + 'static,
) -> AnyElement {
    let state = match done {
        None => Some(spinner::eclipse((key.clone(), 1), colors::text3())),
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
        .when(open, |d| {
            d.child(unfold(
                cx,
                body_id,
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
                    .children(
                        done.filter(|(_, out)| !out.trim().is_empty())
                            .map(|(_, out)| tool::mono(out)),
                    ),
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
    // a quiet chip that lifts on hover; while a run is live the model is fixed, so it only shows
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
            .when(!running, |d| {
                d.cursor_pointer().hover(|s| s.bg(colors::ink(0.06)))
            })
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
            .when(!running, |d| {
                d.on_click(cx.listener(|v, _: &ClickEvent, window, cx| {
                    if let Some(spec) = v.model_spec() {
                        v.menu = Some(ModelMenu::open(&spec, window, cx));
                        cx.notify();
                    }
                }))
            }),
    );
    let effort_chip = v
        .model_spec()
        .filter(|s| !s.efforts.is_empty() || s.long.is_some())
        .map(|spec| {
            model_menu::anchored_chip(
                &v.effort_anchor,
                chip("thread-effort")
                    .text_color(colors::text3())
                    .child(model_menu::effort_chip_label(&spec))
                    .when(!running, |d| {
                        d.on_click(cx.listener(|v, _: &ClickEvent, window, cx| {
                            if let Some(spec) = v.model_spec() {
                                v.menu = Some(ModelMenu::open_effort(&spec, window, cx));
                                cx.notify();
                            }
                        }))
                    }),
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
    // one row like zeron's: attach, the prompt growing in the middle, then the model and send,
    // the buttons staying on the prompt's last line
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
        .when(!v.images.is_empty(), |d| {
            d.child(div().px(px(14.)).pt(px(12.)).child(attach::tray(
                "thread-img",
                &v.images,
                cx.listener(|v, ix: &usize, _, cx| {
                    if *ix < v.images.len() {
                        v.images.remove(*ix);
                    }
                    cx.notify();
                }),
            )))
        })
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(4.))
                .px(px(8.))
                .py(px(8.))
                .child(
                    widgets::icon_button("thread-attach", "paperclip", 28.)
                        .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.pick_images(cx))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pl(px(2.))
                        .pt(px(6.))
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .child(v.input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(4.))
                        .child(permission_chip(v, running, cx))
                        .child(model_chip)
                        .children(effort_chip)
                        .child(action),
                ),
        );
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
    let mut rows: Vec<Div> = Vec::new();
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
            ),
        );
    }
    for (ix, p) in v.queue.iter().enumerate() {
        let text = p.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text = if text.is_empty() {
            format!("{} image(s)", p.images.len())
        } else {
            text
        };
        rows.push(
            line(
                icon("list-checks", 14., colors::text3()).into_any_element(),
                text,
            )
            .child(
                small(widgets::button(
                    ("queue-steer", ix),
                    if held { "Send" } else { "Steer" },
                ))
                .tooltip(widgets::tip(if held {
                    "Send this now"
                } else {
                    "Send this into the run now instead of after it"
                }))
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.steer(ix, cx))),
            )
            .child(
                widgets::icon_button(("queue-drop", ix), "x", 24.)
                    .tooltip(widgets::tip("Remove from the queue"))
                    .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.unqueue(ix, cx))),
            ),
        );
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

fn permission_chip(
    v: &TranscriptView,
    running: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
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
        .when(!running, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.06)))
                .on_click(cx.listener(|v, e: &ClickEvent, _, cx| {
                    let at = e.position();
                    v.perm_menu = Some(gpui::point(at.x - px(240.), at.y));
                    cx.notify();
                }))
        })
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
