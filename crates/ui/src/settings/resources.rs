// Settings, Resources: what HyprSpace and the processes it started use, read by the engine
// (`engine/src/resources.rs`) every two seconds while this view is open and never otherwise.

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
use crate::zoom::px;

const EVERY: Duration = Duration::from_secs(2);

#[cfg(windows)]
const IO_NOTE: &str = "Disk, pipes and network";
#[cfg(not(windows))]
const IO_NOTE: &str = "Disk";

/// The last reading, and when the next one was asked for.
#[derive(Default)]
pub(crate) struct Watch {
    reading: Option<Resources>,
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
fn figure(label: &str, value: String, under: String) -> AnyElement {
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
                .child(value),
        )
        .child(text(under).truncate())
        .into_any_element()
}

/// A row's CPU and memory, in columns that line up down the list.
fn numbers(load: &Load) -> AnyElement {
    let col = |s: String| {
        div()
            .w(px(72.))
            .flex()
            .justify_end()
            .text_size(px(13.))
            .text_color(colors::text2())
            .child(s)
    };
    div()
        .flex()
        .items_center()
        .child(col(cpu(load.cpu)))
        .child(col(bytes(load.memory)))
        .into_any_element()
}

impl Root {
    /// Asks for a reading while the view shows, and forgets the last one once it doesn't.
    pub(crate) fn watch_resources(&mut self) {
        let showing = self.screen == Screen::Settings && self.settings.tab == super::Tab::Resources;
        let watch = &mut self.settings.watch;
        if !showing {
            *watch = Watch::default();
            return;
        }
        if watch.asked.is_none_or(|t| t.elapsed() >= EVERY) {
            watch.asked = Some(Instant::now());
            self.client.send(Command::Resources);
        }
    }

    pub(crate) fn resources_arrived(&mut self, reading: Resources, cx: &mut Context<Self>) {
        if self.settings.watch.asked.is_some() {
            self.settings.watch.reading = Some(reading);
            cx.notify();
        }
    }

    pub(super) fn resources_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(r) = &self.settings.watch.reading else {
            return block(vec![row("Reading this computer's processes", "", div())])
                .into_any_element();
        };
        let total = r.total();
        let used = r.machine_memory.saturating_sub(r.machine_free);
        // a thread can have more than one session, and a session the UI no longer knows is
        // counted as the app's
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

        let figures = div()
            .grid()
            .grid_cols(3)
            .gap_x(px(24.))
            .gap_y(px(18.))
            .py(px(16.))
            .child(figure(
                "CPU",
                cpu(total.cpu),
                format!("This computer: {}", cpu(r.machine_cpu)),
            ))
            .child(figure(
                "Memory",
                bytes(total.memory),
                format!(
                    "This computer: {} of {}",
                    bytes(used),
                    bytes(r.machine_memory)
                ),
            ))
            .child(figure(
                "Processes",
                total.processes.to_string(),
                "Started by HyprSpace".into(),
            ))
            .child(figure(
                "Read",
                format!("{}/s", bytes(total.read)),
                IO_NOTE.into(),
            ))
            .child(figure(
                "Write",
                format!("{}/s", bytes(total.write)),
                IO_NOTE.into(),
            ))
            .child(figure(
                "Threads",
                threads.len().to_string(),
                "Running on this computer".into(),
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
                let under = format!("{space} \u{b7} {}", plural(load.processes, "process"));
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
                            .child(numbers(&load))
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
            text(format!(
                "The app, its terminal hosts and helpers \u{b7} {}",
                plural(app.processes, "process")
            )),
            div().pr(px(20.)).child(numbers(&app)),
        ));

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .child(section(
                "HyprSpace and everything it started",
                block(vec![figures.into_any_element()]),
            ))
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
