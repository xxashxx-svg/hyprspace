// A terminal session: the engine's PTY output folded through the emulator and painted as a grid.
// The shell can run an agent CLI interactively; the engine types its launch command in and wires
// Claude's hooks to the sidebar. Keys, typed text, the pointer, copy and paste, and the find bar
// each have their own module.

mod clipboard;
mod emulator;
mod glyphs;
mod input;
mod keys;
mod links;
mod mouse;
mod paint;
mod search;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    App, Context, CursorStyle, ElementInputHandler, EventEmitter, FocusHandle, Focusable, Hsla,
    IntoElement, KeyDownEvent, MouseButton, Pixels, Render, Task, Window, canvas, div, prelude::*,
    px,
};
use hyprspace_proto::{Agent, Client, Command, Launch, SessionId};

use crate::colors::{self, hsla, theme};
use emulator::{CellColor, Emulator, Marks};
use mouse::{Drag, Hover};
use paint::Grid;
use search::Find;

/// xterm's blink rate.
const BLINK: Duration = Duration::from_millis(530);
/// The PTY hears about a new size once the pane has stopped changing for this long. A storm of
/// resizes is what smeared claude's screen in the Tauri app.
const RESIZE_SETTLE: Duration = Duration::from_millis(80);

pub enum TerminalEvent {
    /// A file path was ctrl+clicked. The root decides where it opens.
    OpenFile {
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
}

pub struct TerminalView {
    id: SessionId,
    client: Client,
    emu: Emulator,
    focus: FocusHandle,
    /// Relative paths in the output resolve against this.
    cwd: PathBuf,
    status: Option<String>,
    /// The geometry the last frame painted with, for mapping the pointer onto cells.
    grid: Option<Grid>,
    resize: Option<Task<()>>,
    drag: Option<Drag>,
    /// Where on the scrollbar thumb the pointer holds it.
    bar_grab: Option<Pixels>,
    bar_hot: bool,
    /// The pointer is over the grid. The scrollbar only shows then, as in the Tauri app.
    hovered: bool,
    hover: Option<Hover>,
    /// Wheel movement short of a whole line.
    wheel: f32,
    exists: HashMap<PathBuf, (bool, Instant)>,
    find: Option<Find>,
    /// IME text not committed yet.
    preedit: String,
    focused: bool,
    /// Take focus back on the next frame (after the find bar closes).
    refocus: bool,
    blink_on: bool,
    /// When the user last typed. The cursor holds solid while they type, like xterm.
    typed: Instant,
    _blink: Task<()>,
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl TerminalView {
    /// Opens a shell in `cwd`. With `run`, the engine starts that agent in it, and types `prompt`
    /// once the agent is ready.
    pub fn new(
        id: SessionId,
        client: Client,
        cwd: PathBuf,
        run: Option<Launch>,
        prompt: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (cols, rows) = (80, 24);
        // claude draws its own cursor and the Tauri app never blinked over it
        let blink = run.as_ref().is_none_or(|r| r.agent != Agent::Claude);
        client.send(Command::OpenTerminal {
            id,
            cwd: cwd.clone(),
            cols,
            rows,
            run,
            prompt,
        });
        let blinker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let alive = this.update(cx, |v, cx| {
                    if v.focused
                        && v.emu.cursor().is_some_and(|c| c.blink)
                        && v.typed.elapsed() >= BLINK
                    {
                        v.blink_on = !v.blink_on;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            id,
            client,
            emu: Emulator::new(cols, rows, blink, palette),
            focus: cx.focus_handle(),
            cwd,
            status: None,
            grid: None,
            resize: None,
            drag: None,
            bar_grab: None,
            bar_hot: false,
            hovered: false,
            hover: None,
            wheel: 0.0,
            exists: HashMap::new(),
            find: None,
            preedit: String::new(),
            focused: false,
            refocus: false,
            blink_on: true,
            typed: Instant::now(),
            _blink: blinker,
        }
    }

    fn write(&self, bytes: Vec<u8>) {
        self.client
            .send(Command::WriteTerminal { id: self.id, bytes });
    }

    /// Bytes the user sent: the view jumps back to the live bottom, like xterm.
    fn input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.emu.scroll_to_bottom();
        self.blink_on = true;
        self.typed = Instant::now();
        self.write(bytes);
        cx.notify();
    }

    /// A line of this terminal's scrollback holding `query`, for the command palette.
    pub fn snippet(&self, query: &str) -> Option<String> {
        self.emu.snippet(query, 72)
    }

    pub fn output(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let reply = self.emu.feed(bytes);
        if !reply.is_empty() {
            self.write(reply);
        }
        // the text under the pointer may have moved
        self.hover = None;
        if self.find.is_some() {
            self.refind(false, cx);
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

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let m = &ks.modifiers;
        // Cmd on macOS; Ctrl elsewhere, where Ctrl+C still interrupts when nothing is selected
        let mac = cfg!(target_os = "macos");
        let primary = if mac { m.platform } else { m.control };
        match ks.key.as_str() {
            // with nothing selected, Ctrl+Shift+C does nothing, as in the Tauri app
            "c" if primary && (mac || m.shift || self.emu.selection_text().is_some()) => {
                if self.copy(cx) {
                    self.emu.clear_selection();
                    cx.notify();
                }
                cx.stop_propagation();
                return;
            }
            "v" if primary => {
                self.paste(cx);
                cx.stop_propagation();
                return;
            }
            "v" if m.alt && !m.control => {
                self.paste_image_or_key(cx);
                cx.stop_propagation();
                return;
            }
            "f" if primary => {
                self.open_find(window, cx);
                cx.stop_propagation();
                return;
            }
            _ => {}
        }
        // typed text arrives through the input handler, IME and all
        if keys::is_text(ks.key_char.as_deref(), m) {
            return;
        }
        if let Some(b) = keys::bytes(&ks.key, ks.key_char.as_deref(), m, self.emu.app_cursor()) {
            self.input(b, cx);
            cx.stop_propagation();
        }
    }

    /// Called from prepaint with the grid that fits the pane. The emulator reflows now; the PTY
    /// hears once the size settles.
    fn fit(&mut self, grid: Grid, cx: &mut Context<Self>) {
        self.grid = Some(grid);
        if self.emu.size() == (grid.cols, grid.rows) {
            return;
        }
        self.emu.resize(grid.cols, grid.rows);
        let (id, client) = (self.id, self.client.clone());
        self.resize = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(RESIZE_SETTLE).await;
            if let Ok((cols, rows)) = this.read_with(cx, |v, _| v.emu.size()) {
                client.send(Command::ResizeTerminal { id, cols, rows });
            }
        }));
    }

    // ---- find ----

    fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            self.find = Some(Find::new(cx));
        }
        if let Some(f) = &self.find {
            let input = f.input.clone();
            input.update(cx, |i, cx| i.select_all_text(cx));
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn close_find(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        // focus comes back to the grid on the next frame (see render)
        self.refocus = true;
        cx.notify();
    }

    fn refind(&mut self, jump: bool, cx: &mut Context<Self>) {
        let Some(f) = self.find.as_mut() else {
            return;
        };
        let query = f.input.read(cx).text().to_string();
        f.run(&mut self.emu, jump, &query);
        cx.notify();
    }

    fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        if let Some(f) = self.find.as_mut() {
            f.step(&mut self.emu, down);
            cx.notify();
        }
    }

    fn toggle_case(&mut self, cx: &mut Context<Self>) {
        if let Some(f) = self.find.as_mut() {
            f.case = !f.case;
        }
        self.refind(true, cx);
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

/// The theme's colors by xterm's numbering, for programs that ask (OSC 4, 10, 11, 12).
fn palette(ix: usize) -> (u8, u8, u8) {
    let t = theme();
    let c = match ix {
        0..=255 => t.indexed(ix as u8),
        257 => t.term_bg,
        258 => t.cursor,
        _ => t.term_fg,
    };
    let [r, g, b, _] = c.0.to_be_bytes();
    (r, g, b)
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.refocus) {
            window.focus(&self.focus, cx);
        }
        let focused = self.focus.is_focused(window);
        if focused != self.focused {
            self.focused = focused;
            self.blink_on = true;
        }
        let view = cx.entity();
        let handler = view.clone();
        let focus = self.focus.clone();
        let pointer = if self.hover.is_some() {
            CursorStyle::PointingHand
        } else if self.bar_hot || self.bar_grab.is_some() {
            CursorStyle::Arrow
        } else {
            CursorStyle::IBeam
        };
        div()
            .id(("terminal", self.id.0))
            .key_context("Terminal")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(theme().term_bg))
            .on_key_down(cx.listener(Self::on_key))
            .child(
                div()
                    .id(("terminal-grid", self.id.0))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .cursor(pointer)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right))
                    .on_mouse_move(cx.listener(Self::on_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up))
                    .on_scroll_wheel(cx.listener(Self::on_wheel))
                    .on_hover(cx.listener(|v, hovered: &bool, _, cx| {
                        v.hovered = *hovered;
                        cx.notify();
                    }))
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                let grid = paint::measure(bounds, window);
                                view.update(cx, |v, cx| {
                                    v.fit(grid, cx);
                                    v.frame(&grid, window)
                                })
                            },
                            move |bounds, frame, window, cx| {
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, handler),
                                    cx,
                                );
                                paint::paint(bounds, frame, window, cx)
                            },
                        )
                        .size_full(),
                    )
                    .children(self.find.as_ref().map(|f| f.render(cx))),
            )
            .children(self.status.clone().map(|s| {
                div()
                    .px(px(paint::PAD_X))
                    .pb(px(6.))
                    .text_xs()
                    .text_color(colors::text2())
                    .child(s)
            }))
    }
}

impl TerminalView {
    fn frame(&self, grid: &Grid, window: &Window) -> paint::Frame {
        let (found, current) = self
            .find
            .as_ref()
            .map_or((&[][..], None), |f| (&f.matches[..], f.current));
        let marks = Marks {
            found,
            current,
            link: self.hover.as_ref().map(|h| &h.range),
        };
        let lines = self.emu.lines(&marks);
        let cursor = self.emu.cursor();
        let visible = self.focused && (self.blink_on || !cursor.is_some_and(|c| c.blink));
        let preedit = cursor
            .filter(|_| !self.preedit.is_empty())
            .map(|c| paint::Preedit {
                text: &self.preedit,
                row: c.row,
                col: c.col,
            });
        paint::prepare(
            grid,
            &lines,
            paint::Extras {
                cursor: cursor.filter(|_| visible && self.preedit.is_empty()),
                preedit,
                bar: self
                    .bar()
                    .filter(|_| self.hovered || self.bar_grab.is_some()),
                bar_hot: self.bar_hot || self.bar_grab.is_some(),
            },
            window,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_asking_for_colors_get_the_themes() {
        let [r, g, b, _] = theme().term_bg.0.to_be_bytes();
        assert_eq!(palette(257), (r, g, b));
        let [r, g, b, _] = theme().ansi[1].0.to_be_bytes();
        assert_eq!(palette(1), (r, g, b));
    }

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
