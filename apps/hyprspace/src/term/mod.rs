// A terminal session: a PTY running a shell, folded through the emulator and painted as a grid.

mod emulator;
mod keys;
mod paint;

use futures::StreamExt;
use futures::channel::mpsc;
use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton, Render, Task,
    Window, canvas, div, prelude::*,
};

use crate::pty::{PtyEvent, PtyManager};
use crate::theme;
pub use emulator::CellColor;
use emulator::Emulator;

pub struct TerminalView {
    id: u64,
    ptys: PtyManager,
    emu: Emulator,
    focus: FocusHandle,
    /// Typed into the shell once it first prints, the way the Tauri app launches agents.
    launch: Option<String>,
    status: Option<String>,
    _pump: Task<()>,
}

impl TerminalView {
    pub fn new(id: u64, ptys: PtyManager, cwd: &str, launch: &str, cx: &mut Context<Self>) -> Self {
        let (cols, rows) = (80, 24);
        let (tx, mut rx) = mpsc::unbounded();
        let status = ptys
            .create(id, cwd, cols, rows, tx)
            .err()
            .map(|e| format!("Could not start the shell: {e}"));
        let pump = cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                if this.update(cx, |view, cx| view.on_pty(event, cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            id,
            ptys,
            emu: Emulator::new(cols, rows),
            focus: cx.focus_handle(),
            launch: Some(launch.to_string()),
            status,
            _pump: pump,
        }
    }

    fn on_pty(&mut self, event: PtyEvent, cx: &mut Context<Self>) {
        match event {
            PtyEvent::Data(bytes) => {
                let reply = self.emu.feed(&bytes);
                if !reply.is_empty() {
                    self.ptys.write(self.id, &reply);
                }
                if let Some(cmd) = self.launch.take() {
                    self.ptys.write(self.id, format!("{cmd}\r").as_bytes());
                }
            }
            PtyEvent::Exit(code) => {
                self.status = Some(format!("The shell exited with code {code}."))
            }
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        if let Some(b) = keys::bytes(
            &ks.key,
            ks.key_char.as_deref(),
            &ks.modifiers,
            self.emu.app_cursor(),
        ) {
            self.ptys.write(self.id, &b);
            cx.stop_propagation();
        }
    }

    /// Called from prepaint with the grid that fits the pane.
    fn fit(&mut self, cols: u16, rows: u16) {
        if self.emu.size() != (cols, rows) {
            self.emu.resize(cols, rows);
            self.ptys.resize(self.id, cols, rows);
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let focused = self.focus.clone();
        div()
            .id(("terminal", self.id))
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::term_bg())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focused.focus(window, cx)
            })
            .on_key_down(cx.listener(Self::on_key))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let grid = paint::measure(bounds, window);
                        view.update(cx, |v, _| {
                            v.fit(grid.cols, grid.rows);
                            paint::prepare(&grid, &v.emu.lines(), v.emu.cursor(), window)
                        })
                    },
                    paint::paint,
                )
                .flex_1(),
            )
            .children(
                self.status
                    .clone()
                    .map(|s| div().px_2().text_xs().text_color(theme::muted()).child(s)),
            )
    }
}
