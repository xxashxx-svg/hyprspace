// A terminal session: the engine's PTY output folded through the emulator and painted as a grid.

mod emulator;
mod keys;
mod paint;

use std::path::PathBuf;

use gpui::{
    App, Context, FocusHandle, Focusable, Hsla, IntoElement, KeyDownEvent, MouseButton, Render,
    Window, canvas, div, prelude::*,
};
use hyprspace_proto::{Client, Command, SessionId};

use crate::colors::{self, hsla, theme};
use emulator::{CellColor, Emulator};

pub struct TerminalView {
    id: SessionId,
    client: Client,
    emu: Emulator,
    focus: FocusHandle,
    /// Typed into the shell once it first prints, the way the Tauri app launches agents.
    launch: Option<String>,
    status: Option<String>,
}

impl TerminalView {
    pub fn new(
        id: SessionId,
        client: Client,
        cwd: PathBuf,
        launch: &str,
        cx: &mut Context<Self>,
    ) -> Self {
        let (cols, rows) = (80, 24);
        client.send(Command::OpenTerminal {
            id,
            cwd,
            cols,
            rows,
        });
        Self {
            id,
            client,
            emu: Emulator::new(cols, rows),
            focus: cx.focus_handle(),
            launch: Some(launch.to_string()).filter(|l| !l.is_empty()),
            status: None,
        }
    }

    fn write(&self, bytes: Vec<u8>) {
        self.client
            .send(Command::WriteTerminal { id: self.id, bytes });
    }

    pub fn output(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let reply = self.emu.feed(bytes);
        if !reply.is_empty() {
            self.write(reply);
        }
        if let Some(cmd) = self.launch.take() {
            self.write(format!("{cmd}\r").into_bytes());
        }
        cx.notify();
    }

    pub fn exit(&mut self, code: i32, cx: &mut Context<Self>) {
        self.status = Some(format!("The shell exited with code {code}."));
        cx.notify();
    }

    pub fn fail(&mut self, message: String, cx: &mut Context<Self>) {
        self.status = Some(message);
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
            self.write(b);
            cx.stop_propagation();
        }
    }

    /// Called from prepaint with the grid that fits the pane.
    fn fit(&mut self, cols: u16, rows: u16) {
        if self.emu.size() != (cols, rows) {
            self.emu.resize(cols, rows);
            self.client.send(Command::ResizeTerminal {
                id: self.id,
                cols,
                rows,
            });
        }
    }
}

/// A cell color through the theme's terminal palette.
fn cell_color(color: CellColor) -> Hsla {
    match color {
        CellColor::Foreground => hsla(theme().term_fg),
        CellColor::Background => hsla(theme().term_bg),
        CellColor::Rgb(r, g, b) => hsla(hyprspace_theme::Color::rgb(u32::from_be_bytes([
            0, r, g, b,
        ]))),
        CellColor::Indexed(ix) => hsla(theme().indexed(ix)),
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
            .id(("terminal", self.id.0))
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(theme().term_bg))
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
                    .map(|s| div().px_2().text_xs().text_color(colors::text2()).child(s)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_use_the_theme_palette() {
        assert_eq!(cell_color(CellColor::Indexed(1)), hsla(theme().ansi[1]));
        assert_eq!(cell_color(CellColor::Background), hsla(theme().term_bg));
        assert_eq!(
            cell_color(CellColor::Rgb(0x12, 0x34, 0x56)),
            hsla(hyprspace_theme::Color::rgb(0x123456))
        );
    }
}
