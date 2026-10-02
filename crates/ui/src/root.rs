// The window's root: sessions laid out as a grid, structured ones first, and the one task that routes the
// engine's events to the view each one names.

use std::collections::HashMap;
use std::path::PathBuf;

use futures::StreamExt;
use gpui::{
    AnyView, Context, Entity, Focusable, IntoElement, Render, Task, Window, div, prelude::*,
};
use hyprspace_proto::{Agent, Client, Event, Events, SessionId};

use crate::colors;
use crate::terminal::TerminalView;
use crate::transcript::TranscriptView;

/// What the window opens with.
pub struct Layout {
    pub structured: usize,
    /// The agent each structured session runs.
    pub agent: Agent,
    pub terms: usize,
    /// The first prompt for each structured session.
    pub prompt: String,
    /// Typed into each terminal's shell once it starts; empty for a bare shell.
    pub launch: String,
    pub cwd: PathBuf,
}

enum View {
    Structured(Entity<TranscriptView>),
    Terminal(Entity<TerminalView>),
}

pub struct Root {
    panes: Vec<AnyView>,
    views: HashMap<SessionId, View>,
    _pump: Task<()>,
}

impl Root {
    pub fn new(
        layout: Layout,
        client: Client,
        mut events: Events,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panes = Vec::new();
        let mut views = HashMap::new();
        let mut next = 0;
        let mut id = || {
            next += 1;
            SessionId(next)
        };
        for _ in 0..layout.structured {
            let id = id();
            let view = cx.new(|_| {
                TranscriptView::new(
                    id,
                    layout.agent,
                    &layout.prompt,
                    layout.cwd.clone(),
                    &client,
                )
            });
            panes.push(AnyView::from(view.clone()));
            views.insert(id, View::Structured(view));
        }
        for n in 0..layout.terms {
            let id = id();
            let term = cx.new(|cx| {
                TerminalView::new(id, client.clone(), layout.cwd.clone(), &layout.launch, cx)
            });
            if n == 0 {
                window.focus(&term.focus_handle(cx), cx);
            }
            panes.push(AnyView::from(term.clone()));
            views.insert(id, View::Terminal(term));
        }
        let pump = cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                if this.update(cx, |root, cx| root.route(event, cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            panes,
            views,
            _pump: pump,
        }
    }

    fn route(&mut self, event: Event, cx: &mut Context<Self>) {
        let id = match &event {
            Event::Run { id, .. }
            | Event::TerminalOutput { id, .. }
            | Event::TerminalExit { id, .. }
            | Event::Failed { id, .. } => *id,
        };
        // an event for a session whose view is gone is stale, so it is dropped
        let Some(view) = self.views.get(&id) else {
            return;
        };
        match (view, event) {
            (View::Structured(v), Event::Run { event, .. }) => {
                v.update(cx, |v, cx| v.apply(event, cx))
            }
            (View::Structured(v), Event::Failed { message, .. }) => {
                v.update(cx, |v, cx| v.fail(message, cx))
            }
            (View::Terminal(v), Event::TerminalOutput { bytes, .. }) => {
                v.update(cx, |v, cx| v.output(&bytes, cx))
            }
            (View::Terminal(v), Event::TerminalExit { code, .. }) => {
                v.update(cx, |v, cx| v.exit(code, cx))
            }
            (View::Terminal(v), Event::Failed { message, .. }) => {
                v.update(cx, |v, cx| v.fail(message, cx))
            }
            _ => {}
        }
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let cols = (self.panes.len() as f32).sqrt().ceil().max(1.0) as usize;
        let rows = self.panes.chunks(cols).map(|row| {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .children(row.iter().map(|pane| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_1()
                        .border_color(colors::border())
                        .child(pane.clone())
                }))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::bg())
            .font_family(".SystemUIFont")
            .text_color(colors::text())
            .children(rows)
    }
}
