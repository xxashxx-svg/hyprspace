// Draws a transcript after zeron's: one centered column, the user's prompts as bubbles on the
// right, replies as markdown, thinking and each run of tool calls folded into one muted line
// until clicked, approval prompts with their buttons, and the pill-shaped box to reply or steer.

use chrono::TimeZone;
use gpui::{
    AnyElement, ClickEvent, Context, Div, ExternalPaths, Focusable, FontWeight, IntoElement,
    MouseButton, ScrollHandle, ScrollWheelEvent, SharedString, StyledText, Window, div, prelude::*,
    px, relative,
};
use hyprspace_proto::{RunStatus, Tool};

use super::model::Item;
use super::{TranscriptView, tool};
use crate::assets::{icon, mark};
use crate::composer::model_menu::{self, Host as _, ModelMenu};
use crate::{attach, colors, markdown, spinner, widgets};

/// The transcript and the composer share one column, so their edges line up. Text runs 768px
/// wide, as T3 Code's chat does.
const COLUMN: f32 = 816.;
const GUTTER: f32 = 24.;

pub fn view(
    v: &mut TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let items = items(v, cx);
    let working = v.model.elapsed().map(working);
    let loading = v.loading.then(|| {
        div()
            .text_size(px(12.))
            .text_color(colors::text3())
            .child("Loading the conversation...")
    });
    let empty = (!v.loading && v.model.items.is_empty()).then(|| {
        div()
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
    });
    // only while a reply streams: then new text would land out of sight
    let jump = (v.model.running() && !v.at_bottom()).then(|| jump(cx));
    // every row is its own child of the scroll, so the scroll handle knows where each prompt is
    let lead = usize::from(loading.is_some()) + usize::from(empty.is_some());
    let prompts: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, (user, _))| *user)
        .map(|(i, _)| lead + i)
        .collect();
    let rows = loading
        .map(IntoElement::into_any_element)
        .into_iter()
        .chain(empty.map(IntoElement::into_any_element))
        .chain(items.into_iter().map(|(_, row)| row))
        .chain(working)
        .map(|row| centered(column().child(row)));
    let ticks = ticks(v, &prompts, window, cx);
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
                .relative()
                .flex_1()
                .min_h_0()
                // before any text, so selection knows this transcript's elements
                .child(markdown::select::reset(format!("t{}", v.id.0).into()))
                .child(
                    div()
                        .id("transcript-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&v.scroll)
                        .flex()
                        .flex_col()
                        .pt(px(24.))
                        .pb(px(24.))
                        .gap(px(20.))
                        // repaint as the user scrolls, so the jump button comes and goes and
                        // the ticks follow
                        .on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()))
                        .children(rows),
                )
                .children(ticks)
                .children(jump),
        )
        .child(composer(v, window, cx))
        .children(model_menu(v, window, cx))
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

fn working(secs: u64) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.))
        .text_color(colors::text3())
        .child(spinner::eclipse("working", colors::text3()))
        .child(
            div()
                .text_color(colors::text2())
                .child(format!("Working {secs}s")),
        )
        .child("· Esc to stop")
        .into_any_element()
}

fn jump(cx: &mut Context<TranscriptView>) -> AnyElement {
    div()
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
                    v.scroll.scroll_to_bottom();
                    cx.notify();
                })),
        )
        .into_any_element()
}

/// Every item. Thinking and tool calls in a row sit together as one quiet block, and each run
/// of two or more tool calls in it folds into one line.
/// Each row, and whether it is one of the user's prompts.
fn items(v: &TranscriptView, cx: &mut Context<TranscriptView>) -> Vec<(bool, AnyElement)> {
    let all = &v.model.items;
    let mut out = Vec::new();
    let mut ix = 0;
    while ix < all.len() {
        let quiet = all[ix..]
            .iter()
            .take_while(|i| matches!(i, Item::Tool { .. } | Item::Thinking { .. }))
            .count();
        // a run that finished fine needs no line, the way zeron's transcript has none
        if let Item::Finished {
            status: RunStatus::Done,
            ..
        } = all[ix]
        {
            ix += 1;
            continue;
        }
        if quiet == 0 {
            let user = matches!(all[ix], Item::User { .. });
            out.push((user, item(v, ix, &all[ix], cx)));
            ix += 1;
            continue;
        }
        let mut rows = Vec::new();
        let end = ix + quiet;
        while ix < end {
            let calls = all[ix..end]
                .iter()
                .take_while(|i| matches!(i, Item::Tool { .. }))
                .count();
            if calls > 1 {
                rows.push(tool_run(v, ix, ix + calls, cx));
                ix += calls;
            } else {
                rows.push(item(v, ix, &all[ix], cx));
                ix += 1;
            }
        }
        out.push((
            false,
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children(rows)
                .into_any_element(),
        ));
    }
    out
}

/// The prompt being read: the last one above a line a third of the way down the view, or the last
/// one at all once the view is at the bottom, where the latest prompt can sit below the line.
fn lit(scroll: &ScrollHandle, prompts: &[usize]) -> usize {
    if -scroll.offset().y >= scroll.max_offset().y - px(4.) {
        return prompts.len().saturating_sub(1);
    }
    let view = scroll.bounds();
    // a row's bounds are where it sits unscrolled, so the line moves down by the scroll instead
    let line = view.top() + view.size.height / 3. - scroll.offset().y;
    prompts
        .iter()
        .rposition(|&row| scroll.bounds_for_item(row).is_some_and(|b| b.top() <= line))
        .unwrap_or(0)
}

/// One tick per prompt on the left edge, like T3 Code's and zeron's: the one for the part being
/// read lit, a click scrolling to its prompt. `prompts` are the prompts' rows in the scroll.
fn ticks(
    v: &TranscriptView,
    prompts: &[usize],
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> Option<AnyElement> {
    // two prompts at least, and room beside the column so the ticks don't sit on the text
    if prompts.len() < 2 || v.scroll.bounds().size.width < px(COLUMN + 80.) {
        return None;
    }
    // positions come from the last layout, and a scroll set during this one (to the bottom, say)
    // only shows in the next, so check again then and redraw if the lit tick moved
    let lit = lit(&v.scroll, prompts);
    let rows = prompts.to_vec();
    cx.on_next_frame(window, move |v, _, cx| {
        if self::lit(&v.scroll, &rows) != lit {
            cx.notify();
        }
    });
    let marks = prompts.iter().enumerate().map(|(n, &row)| {
        let on = n == lit;
        let group = SharedString::from(format!("tick-{n}"));
        div()
            .id(("tick", n))
            .group(group.clone())
            .flex()
            .items_center()
            .w(px(16.))
            .h(px(8.))
            .cursor_pointer()
            .child(
                div()
                    .h(px(1.5))
                    .w(px(if on { 10. } else { 7. }))
                    .rounded_full()
                    .bg(if on {
                        colors::text1()
                    } else {
                        colors::text3().opacity(0.6)
                    })
                    .group_hover(group, |s| s.bg(colors::text1()).w(px(10.))),
            )
            .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
                v.scroll.scroll_to_top_of_item(row);
                cx.notify();
            }))
    });
    Some(
        div()
            .absolute()
            .left(px(14.))
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(div().flex().flex_col().children(marks))
            .into_any_element(),
    )
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
                        let text = SharedString::from(text.clone());
                        let styled = StyledText::new(text.clone());
                        let layout = styled.layout().clone();
                        d.child(markdown::select::wrap(
                            &format!("u{ix}"),
                            text,
                            layout,
                            styled.into_any_element(),
                        ))
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
            .into_any_element(),
        Item::Text { blocks, .. } => match blocks {
            Some(b) => div()
                .text_size(px(14.))
                .line_height(relative(1.65))
                .text_color(colors::text1())
                .child(markdown::render(b, &format!("t{ix}")))
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
                    d.child(
                        div()
                            .pl(px(26.))
                            .text_size(px(12.5))
                            .line_height(relative(1.55))
                            .italic()
                            .text_color(colors::text3())
                            .child(text.trim().to_string()),
                    )
                })
                .into_any_element()
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
                .child(icon(
                    if open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    12.,
                    colors::text3(),
                )),
        )
        .child(div().min_w_0().truncate().child(label))
        .children(right)
        .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
            toggle(v);
            cx.notify();
        }))
        .into_any_element()
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
            d.child(div().pl(px(26.)).flex().flex_col().gap(px(6.)).children(
                (start..end).filter_map(|ix| match &v.model.items[ix] {
                    Item::Tool {
                        tool, done, open, ..
                    } => Some(main_tool(ix, tool, done.as_ref(), *open, cx)),
                    _ => None,
                }),
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
            d.child(
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
            )
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
    centered(
        column()
            .pb(px(12.))
            .children(tray(v, cx))
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
