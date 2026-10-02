// Draws a transcript: the user's prompts, replies as markdown, thinking and tool calls folded
// shut until clicked, approval prompts with their buttons, and the box to reply or steer.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ExternalPaths, Focusable,
    IntoElement, MouseButton, SharedString, Window, div, prelude::*, px, relative,
};
use hyprspace_proto::{RunStatus, Tool};

use super::model::Item;
use super::{TranscriptView, tool};
use crate::assets::{icon, mark};
use crate::{attach, colors, markdown, widgets};

pub fn view(
    v: &mut TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let items: Vec<AnyElement> = v
        .model
        .items
        .iter()
        .enumerate()
        .map(|(ix, item)| self::item(v, ix, item, cx))
        .collect();
    let working = v.model.elapsed().map(working);
    let loading = v.loading.then(|| {
        div()
            .text_xs()
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
                v.launch
                    .cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "this folder".into())
            ))
    });
    div()
        .id("transcript")
        .size_full()
        .flex()
        .flex_col()
        .bg(colors::bg())
        .text_color(colors::text1())
        .on_drop(cx.listener(|v, paths: &ExternalPaths, _, cx| v.drop_paths(paths, cx)))
        .drag_over::<ExternalPaths>(|s, _, _, _| s.bg(colors::accent_dim()))
        .child(
            div()
                .id("transcript-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&v.scroll)
                .child(
                    div().w_full().flex().justify_center().child(
                        div()
                            .w_full()
                            .max_w(px(780.))
                            .px_5()
                            .py_5()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .text_sm()
                            .children(loading)
                            .children(empty)
                            .children(items)
                            .children(working),
                    ),
                ),
        )
        .child(composer(v, window, cx))
        .children(v.menu.map(|at| model_menu(v, at, window, cx)))
        .into_any_element()
}

fn working(secs: u64) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(colors::text2())
        .child(
            div()
                .size(px(7.))
                .rounded_full()
                .bg(colors::busy())
                .with_animation(
                    "working",
                    Animation::new(Duration::from_millis(1200)).repeat(),
                    |d, t| d.opacity(0.35 + 0.65 * (1.0 - (t * 2.0 - 1.0).abs())),
                ),
        )
        .child(format!("Working {secs}s"))
        .child(div().text_color(colors::text3()).child("Esc to stop"))
        .into_any_element()
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
                d.child(div().text_xs().text_color(colors::text3()).child("Steer"))
            })
            .child(
                div()
                    .max_w(relative(0.85))
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(colors::surface2())
                    .border_1()
                    .border_color(colors::border1())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(!text.is_empty(), |d| d.child(text.clone()))
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
            Some(b) => markdown::render(b, &format!("t{ix}")),
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
                .gap_1()
                .child(head)
                .when(*open, |d| {
                    d.child(
                        div()
                            .pl_4()
                            .text_xs()
                            .italic()
                            .text_color(colors::text2())
                            .child(text.trim().to_string()),
                    )
                })
                .into_any_element()
        }
        Item::Tool {
            tool, done, open, ..
        } => tool_card(ix, tool, done.as_ref(), *open, cx),
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
                .gap_2()
                .px_3()
                .py_2()
                .rounded(px(8.))
                .border_1()
                .border_color(colors::error().opacity(0.4))
                .bg(colors::error().opacity(0.08))
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
        Item::Finished {
            status,
            ms,
            error,
            tokens,
        } => {
            let secs = *ms as f32 / 1000.0;
            let mut line = match status {
                RunStatus::Done => format!("Done in {secs:.1}s"),
                RunStatus::Interrupted => format!("Stopped after {secs:.1}s"),
                RunStatus::Failed => match error {
                    Some(e) => format!("Failed: {e}"),
                    None => format!("Failed after {secs:.1}s"),
                },
            };
            if let Some((i, o)) = tokens {
                line.push_str(&format!(", {} in, {} out", short(*i), short(*o)));
            }
            div()
                .text_xs()
                .text_color(if *status == RunStatus::Failed {
                    colors::error()
                } else {
                    colors::text3()
                })
                .child(line)
                .into_any_element()
        }
        Item::Note(text) => div()
            .text_xs()
            .text_color(colors::text3())
            .child(text.clone())
            .into_any_element(),
    }
}

/// Token counts the way people read them: 950, 12.4k, 1.2M.
fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1000..=999_999 => format!("{:.1}k", n as f32 / 1000.0),
        _ => format!("{:.1}M", n as f32 / 1_000_000.0),
    }
}

/// A clickable line with a fold arrow. `toggle` flips the item's open state.
fn fold_head(
    id: (&'static str, usize),
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
        .gap_2()
        .text_xs()
        .text_color(colors::text2())
        .cursor_pointer()
        .hover(|s| s.text_color(colors::text1()))
        .child(
            div()
                .w(px(10.))
                .text_color(colors::text3())
                .child(if open { "▾" } else { "▸" }),
        )
        .child(div().min_w_0().truncate().child(label))
        .children(right)
        .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| {
            toggle(v);
            cx.notify();
        }))
        .into_any_element()
}

fn tool_card(
    ix: usize,
    t: &Tool,
    done: Option<&(bool, String)>,
    open: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let mark = match done {
        None => div().text_color(colors::busy()).child("•"),
        Some((true, _)) => div().text_color(colors::ok()).child("✓"),
        Some((false, _)) => div().text_color(colors::error()).child("✕"),
    };
    let counts = match t {
        Tool::Edit { changes } => {
            let (a, d) = tool::counts(changes);
            Some(
                div()
                    .flex()
                    .gap_1()
                    .flex_none()
                    .child(div().text_color(colors::diff_add()).child(format!("+{a}")))
                    .child(div().text_color(colors::diff_del()).child(format!("-{d}")))
                    .into_any_element(),
            )
        }
        _ => None,
    };
    let head = fold_head(
        ("tool", ix),
        open,
        tool::label(t).into(),
        Some(
            div()
                .flex()
                .gap_2()
                .items_center()
                .flex_none()
                .child(mark)
                .children(counts)
                .into_any_element(),
        ),
        cx,
        move |v| {
            if let Some(Item::Tool { open, .. }) = v.model.items.get_mut(ix) {
                *open = !*open;
            }
        },
    );
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(head)
        .when(open, |d| {
            d.child(
                div()
                    .pl_4()
                    .flex()
                    .flex_col()
                    .gap_1()
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

fn composer(
    v: &TranscriptView,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let running = v.model.running();
    let agent = v.launch.agent;
    let (brand, _) = colors::brand(agent);
    let focused = v.input.focus_handle(cx).is_focused(window);
    let chip = widgets::chip("thread-model")
        .child(mark(agent, 13., brand))
        .child(div().truncate().child(v.model_label()));
    // the model is fixed while a run is live; it can change between runs
    let model_chip = if running {
        chip.cursor_default().into_any_element()
    } else {
        chip.child(widgets::caret())
            .on_click(cx.listener(|v, e: &ClickEvent, _, cx| {
                v.menu = Some(e.position());
                cx.notify();
            }))
            .into_any_element()
    };
    div()
        .w_full()
        .flex()
        .justify_center()
        .px_5()
        .pb_4()
        .child(
            div()
                .w_full()
                .max_w(px(780.))
                .flex()
                .flex_col()
                .rounded(px(14.))
                .border_1()
                .border_color(if focused {
                    brand.opacity(0.45)
                } else {
                    colors::border2()
                })
                .bg(colors::surface2().opacity(0.85))
                .shadow(colors::shadow())
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
                        .min_h(px(44.))
                        .px(px(14.))
                        .pt(px(12.))
                        .pb(px(8.))
                        .text_size(px(14.5))
                        .line_height(px(22.))
                        .child(v.input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px(px(10.))
                        .py(px(8.))
                        .border_t_1()
                        .border_color(colors::border1())
                        .child(model_chip)
                        .child(div().flex_1())
                        .when(running, |d| {
                            d.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(colors::text3())
                                    .child("Enter steers the run"),
                            )
                            .child(
                                widgets::button("stop", "Stop").on_click(
                                    cx.listener(|v, _: &ClickEvent, _, cx| v.interrupt(cx)),
                                ),
                            )
                        })
                        .child(
                            widgets::send("send", "arrow-up")
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.submit(cx))),
                        ),
                ),
        )
        .into_any_element()
}

fn model_menu(
    v: &TranscriptView,
    at: gpui::Point<gpui::Pixels>,
    window: &mut Window,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    let current = v.launch.model.clone().unwrap_or_default();
    let rows: Vec<AnyElement> = match &v.catalog {
        Some(cat) => cat
            .models
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let id = m.id.clone();
                widgets::menu_item(
                    ("thread-model-row", i),
                    m.label.clone(),
                    m.note.clone().map(Into::into),
                    m.id == current,
                )
                .on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.pick_model(id.clone(), cx)))
                .into_any_element()
            })
            .collect(),
        None => vec![
            div()
                .p_2()
                .text_xs()
                .text_color(colors::text3())
                .child("Loading models...")
                .into_any_element(),
        ],
    };
    let close = cx.listener(|v, _: &(), _, cx| {
        v.menu = None;
        cx.notify();
    });
    widgets::popup(
        at,
        widgets::Open::Up,
        window,
        move |w, cx| close(&(), w, cx),
        div()
            .flex()
            .flex_col()
            .child(widgets::menu_heading("Model for this thread"))
            .children(rows)
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .italic()
                    .text_color(colors::text3())
                    .child("Changing it resumes the conversation on the new model."),
            ),
    )
}
