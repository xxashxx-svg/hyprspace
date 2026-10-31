// Settings, Resources: what HyprSpace and the processes it started use, read by the engine
// (`engine/src/resources.rs`) every two seconds while this view is open and never otherwise. The
// last reading stays after it closes, so coming back shows it dimmed until a fresh one lands.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{AnyElement, ClickEvent, Context, FontWeight, IntoElement, div, prelude::*};
use hyprspace_proto::Command;
use hyprspace_proto::resources::{Load, Resources};

use super::Root;
use super::controls::{block, row, row_with, section, text};
use crate::assets::icon;
use crate::colors;
use crate::root::Screen;
use crate::time::ago;
use crate::zoom::px;

const EVERY: Duration = Duration::from_secs(2);

#[cfg(windows)]
const IO_NOTE: &str = "Disk, pipes and network";
#[cfg(not(windows))]
const IO_NOTE: &str = "Disk";

/// The last reading, kept after the page closes so it shows at once next time, and when the next
/// one was asked for.
#[derive(Default)]
pub(crate) struct Watch {
    reading: Option<Resources>,
    read: Option<Instant>,
    /// The reading came while the page has been open this time.
    fresh: bool,
    asked: Option<Instant>,
}

fn bytes(n: u64) -> String {
    const KB: f64 = 1024.;
    let n = n as f64;
    if n < KB {
        format!("{n} B")
    } else if n < KB * KB {
        format!("{:.0} KB", n / KB)
    } else if n < KB * KB * KB {
        format!("{:.0} MB", n / KB / KB)
    } else {
        format!("{:.1} GB", n / KB / KB / KB)
    }
}

fn cpu(pct: f32) -> String {
    format!("{pct:.1}%")
}

fn plural(n: u32, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "es" })
}

/// One figure: a quiet label, the value, and a line under it.
fn figure(label: &str, value: String, under: String, dim: bool) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .min_w_0()
        .child(
            div()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors::text1())
                .when(dim, |d| d.opacity(0.45))
                .child(value),
        )
        .child(text(under).truncate())
        .into_any_element()
}

/// A row's CPU and memory, in columns that line up down the list, or dashes before a reading.
fn numbers(load: Option<&Load>, dim: bool) -> AnyElement {
    let col = |s: String| {
        div()
            .w(px(72.))
            .flex()
            .justify_end()
            .text_size(px(13.))
            .text_color(if load.is_some() {
                colors::text2()
            } else {
                colors::text3()
            })
            .child(s)
    };
    div()
        .flex()
        .items_center()
        .when(dim, |d| d.opacity(0.45))
        .child(col(load.map_or("-".into(), |l| cpu(l.cpu))))
        .child(col(load.map_or("-".into(), |l| bytes(l.memory))))
        .into_any_element()
}

/// What the page is waiting on, beside its first heading.
fn status(watch: &Watch) -> Option<AnyElement> {
    let line = match watch.read {
        None => "Reading".to_string(),
        Some(_) if watch.fresh => return None,
        Some(at) => match ago(0, at.elapsed().as_millis() as u64).as_str() {
            "now" => "Updating".to_string(),
            since => format!("Updating, read {since} ago"),
        },
    };
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .text_color(colors::text3())
            .child(crate::spinner::ring("resources-reading", colors::text3()))
            .child(line)
            .into_any_element(),
    )
}

impl Root {
    /// Asks for a reading while the view shows, and forgets the last one once it doesn't.
    pub(crate) fn watch_resources(&mut self) {
        let showing = self.screen == Screen::Settings && self.settings.tab == super::Tab::Resources;
        let watch = &mut self.settings.watch;
        if !showing {
            watch.asked = None;
            watch.fresh = false;
            return;
        }
        if watch.asked.is_none_or(|t| t.elapsed() >= EVERY) {
            watch.asked = Some(Instant::now());
            self.client.send(Command::Resources);
        }
    }

    pub(crate) fn resources_arrived(&mut self, reading: Resources, cx: &mut Context<Self>) {
        if self.settings.watch.asked.is_some() {
            let watch = &mut self.settings.watch;
            watch.reading = Some(reading);
            watch.read = Some(Instant::now());
            watch.fresh = true;
            cx.notify();
        }
    }

    pub(super) fn resources_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let watch = &self.settings.watch;
        let r = watch.reading.as_ref();
        let dim = r.is_some() && !watch.fresh;
        let (threads, app) = match r {
            Some(r) => {
                // a thread can have more than one session, and a session the UI no longer knows
                // is counted as the app's
                let mut app = r.app;
                let mut threads: HashMap<u64, Load> = HashMap::new();
                for (id, load) in &r.sessions {
                    match self.sessions.get(id) {
                        Some(thread) => threads.entry(*thread).or_default().add(load),
                        None => app.add(load),
                    }
                }
                let mut threads: Vec<_> = threads.into_iter().collect();
                threads.sort_by(|a, b| b.1.cpu.total_cmp(&a.1.cpu).then(a.0.cmp(&b.0)));
                let threads: Vec<_> = threads.into_iter().map(|(id, l)| (id, Some(l))).collect();
                (threads, Some(app))
            }
            None => {
                let mut ids: Vec<u64> = self
                    .sessions
                    .values()
                    .copied()
                    .filter(|t| self.machines.remote(*t).is_none())
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                (ids.into_iter().map(|id| (id, None)).collect(), None)
            }
        };
        let total = r.map(|r| r.total());
        let read = |f: &dyn Fn(&Resources, &Load) -> String| {
            r.zip(total.as_ref())
                .map_or("-".to_string(), |(r, t)| f(r, t))
        };

        let figures = div()
            .grid()
            .grid_cols(3)
            .gap_x(px(24.))
            .gap_y(px(18.))
            .py(px(16.))
            .child(figure(
                "CPU",
                read(&|_, t| cpu(t.cpu)),
                format!("This computer: {}", read(&|r, _| cpu(r.machine_cpu))),
                dim,
            ))
            .child(figure(
                "Memory",
                read(&|_, t| bytes(t.memory)),
                format!(
                    "This computer: {}",
                    read(&|r, _| format!(
                        "{} of {}",
                        bytes(r.machine_memory.saturating_sub(r.machine_free)),
                        bytes(r.machine_memory)
                    ))
                ),
                dim,
            ))
            .child(figure(
                "Processes",
                read(&|_, t| t.processes.to_string()),
                "Started by HyprSpace".into(),
                dim,
            ))
            .child(figure(
                "Read",
                read(&|_, t| format!("{}/s", bytes(t.read))),
                IO_NOTE.into(),
                dim,
            ))
            .child(figure(
                "Write",
                read(&|_, t| format!("{}/s", bytes(t.write))),
                IO_NOTE.into(),
                dim,
            ))
            .child(figure(
                "Threads",
                read(&|_, _| threads.len().to_string()),
                "Running on this computer".into(),
                dim,
            ));

        let mut rows: Vec<AnyElement> = threads
            .into_iter()
            .map(|(id, load)| {
                let (space, title) = self
                    .state
                    .thread(id)
                    .map(|(s, t)| (s.name.clone(), t.title.clone()))
                    .unwrap_or_default();
                let title = if title.is_empty() {
                    "New thread".to_string()
                } else {
                    title
                };
                let under = match &load {
                    Some(l) => format!("{space} \u{b7} {}", plural(l.processes, "process")),
                    None => space,
                };
                div()
                    .id(("resources-thread", id))
                    .cursor_pointer()
                    .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.open_thread(id, window, cx)
                    }))
                    .child(row_with(
                        None,
                        title,
                        text(under).truncate(),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(numbers(load.as_ref(), dim))
                            .child(icon("chevron-right", 14., colors::text3())),
                    ))
                    .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            rows.push(row("No threads running on this computer", "", div()));
        }
        rows.push(row_with(
            None,
            "HyprSpace",
            text(match &app {
                Some(app) => format!(
                    "The app, its terminal hosts and helpers \u{b7} {}",
                    plural(app.processes, "process")
                ),
                None => "The app, its terminal hosts and helpers".to_string(),
            }),
            div().pr(px(20.)).child(numbers(app.as_ref(), dim)),
        ));

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(4.))
                            .text_size(px(13.))
                            .text_color(colors::text3())
                            .child("HyprSpace and everything it started")
                            .children(status(watch)),
                    )
                    .child(block(vec![figures.into_any_element()])),
            )
            .child(section("By thread", block(rows)))
            .child(
                div()
                    .px(px(4.))
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(colors::text3())
                    .child(
                        "Read every 2 seconds while this page is open. CPU is a share of the \
                         whole computer, so every core busy is 100%. Threads on your other \
                         computers aren't counted.",
                    ),
            )
            .into_any_element()
    }
}
