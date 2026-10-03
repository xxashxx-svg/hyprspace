// The pointer over a terminal: drag, double-click and triple-click selection, the wheel (history,
// or arrow keys and wheel reports for full-screen programs), the scrollbar, a right-click paste,
// and links. A URL, an existing file path or Claude's `[Image #N]` under the pointer is
// underlined; Ctrl+click (Cmd on macOS) opens it, and an image previews while the pointer rests
// on it (images.rs). Selection gestures follow zeron's terminal panel (MIT, see
// THIRD_PARTY_NOTICES.md).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use alacritty_terminal::term::search::Match;
use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, Window,
};

use super::emulator::{SelectionType, Side};
use super::links::{self, Target};
use super::paint::{self, Bar};
use super::{TerminalEvent, TerminalView};

/// Pointer travel before a press becomes a selection, so the click that focuses the pane can't
/// leave a one-cell selection behind (gpui's own drag threshold).
const DRAG_THRESHOLD: f32 = 2.0;
/// How long a path's existence check holds before the disk is asked again.
const EXISTS_TTL: Duration = Duration::from_secs(10);

/// A press that may turn into a selection.
#[derive(Clone, Copy)]
pub(super) struct Drag {
    origin: Point<Pixels>,
    armed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Open {
    Url(String),
    File {
        path: PathBuf,
        line: Option<u32>,
        col: Option<u32>,
    },
    /// Claude's `[Image #N]`.
    Marker(u32),
}

/// The link under the pointer and the cells it covers.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Hover {
    pub range: Match,
    pub open: Open,
}

/// Wheel reports for a program that asked for the mouse: button 64 is up, 65 down.
fn wheel_report(up: bool, row: usize, col: usize, sgr: bool) -> Vec<u8> {
    let button = if up { 64 } else { 65 };
    if sgr {
        format!("\x1b[<{button};{};{}M", col + 1, row + 1).into_bytes()
    } else {
        let at = |n: usize| (32 + 1 + n).min(255) as u8;
        vec![0x1b, b'[', b'M', 32 + button, at(col), at(row)]
    }
}

/// Is this the modifier that opens links: Cmd on macOS, Ctrl elsewhere.
fn link_modifier(mods: &gpui::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        mods.platform
    } else {
        mods.control
    }
}

impl TerminalView {
    pub(super) fn bar(&self) -> Option<Bar> {
        let grid = self.grid?;
        if self.emu.alt_screen() {
            return None;
        }
        paint::bar(&grid, self.emu.history(), self.emu.display_offset())
    }

    pub(super) fn on_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let Some(grid) = self.grid else {
            return;
        };
        if let Some(bar) = self.bar()
            && bar.track.contains(&e.position)
        {
            let grab = if bar.thumb.contains(&e.position) {
                e.position.y - bar.thumb.top()
            } else {
                bar.thumb.size.height / 2.
            };
            self.bar_grab = Some(grab);
            self.drag_bar(e.position.y, cx);
            return;
        }
        if link_modifier(&e.modifiers)
            && let Some(hover) = self.link_at(e.position)
        {
            self.open(hover.open, cx);
            return;
        }
        let (row, col, right) = grid.cell(e.position);
        let side = if right { Side::Right } else { Side::Left };
        let point = self.emu.grid_point(row, col);
        let armed = match e.click_count {
            0 | 1 if e.modifiers.shift && self.emu.selection_text().is_some() => {
                self.emu.update_selection(point, side);
                true
            }
            0 | 1 => {
                self.emu.clear_selection();
                false
            }
            2 => {
                self.emu
                    .start_selection(SelectionType::Semantic, point, side);
                true
            }
            _ => {
                self.emu.start_selection(SelectionType::Lines, point, side);
                true
            }
        };
        self.drag = Some(Drag {
            origin: e.position,
            armed,
        });
        cx.notify();
    }

    pub(super) fn on_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.bar_grab.is_some() {
            self.drag_bar(e.position.y, cx);
            return;
        }
        let bar_hot = self.bar().is_some_and(|b| b.track.contains(&e.position));
        if bar_hot != self.bar_hot {
            self.bar_hot = bar_hot;
            cx.notify();
        }
        let (Some(mut drag), Some(MouseButton::Left)) = (self.drag, e.pressed_button) else {
            self.hover_at(e.position, cx);
            return;
        };
        let Some(grid) = self.grid else {
            return;
        };
        if !drag.armed {
            let d = e.position - drag.origin;
            if f32::from(d.x).hypot(f32::from(d.y)) < DRAG_THRESHOLD {
                return;
            }
            // anchor at the press, so the selection covers the whole gesture
            let (row, col, right) = grid.cell(drag.origin);
            let side = if right { Side::Right } else { Side::Left };
            let at = self.emu.grid_point(row, col);
            self.emu.start_selection(SelectionType::Simple, at, side);
            drag.armed = true;
            self.drag = Some(drag);
        }
        // past the top or bottom edge, the view scrolls to take more
        let bottom = grid.origin.y + grid.line_h * grid.rows as f32;
        if e.position.y < grid.origin.y {
            self.emu.scroll(1);
        } else if e.position.y > bottom {
            self.emu.scroll(-1);
        }
        let (row, col, right) = grid.cell(e.position);
        let side = if right { Side::Right } else { Side::Left };
        let at = self.emu.grid_point(row, col);
        self.emu.update_selection(at, side);
        cx.notify();
    }

    pub(super) fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.drag = None;
        self.bar_grab = None;
    }

    /// Right-click pastes, as it did in the Tauri app.
    pub(super) fn on_right(
        &mut self,
        _: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.paste(cx);
    }

    pub(super) fn on_wheel(
        &mut self,
        e: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let Some(grid) = self.grid else {
            return;
        };
        let lines = match e.delta {
            ScrollDelta::Lines(d) => d.y,
            ScrollDelta::Pixels(d) => f32::from(d.y) / f32::from(grid.line_h),
        };
        // a trackpad sends fractions of a line; keep the rest for the next event
        self.wheel += lines;
        let step = self.wheel.trunc() as i32;
        self.wheel -= step as f32;
        if step == 0 {
            return;
        }
        let up = step > 0;
        let n = step.unsigned_abs() as usize;
        if let Some(sgr) = self.emu.mouse_mode() {
            let (row, col, _) = grid.cell(e.position);
            self.write(wheel_report(up, row, col, sgr).repeat(n));
        } else if self.emu.alt_screen() && self.emu.alternate_scroll() {
            // less, vim and friends have no scrollback here: the wheel moves their own view
            let key = match (up, self.emu.app_cursor()) {
                (true, false) => b"\x1b[A",
                (true, true) => b"\x1bOA",
                (false, false) => b"\x1b[B",
                (false, true) => b"\x1bOB",
            };
            self.write(key.repeat(n));
        } else {
            self.emu.scroll(step);
            self.hover = None;
            self.rest_on(None, cx);
            cx.notify();
        }
    }

    fn drag_bar(&mut self, y: Pixels, cx: &mut Context<Self>) {
        let (Some(bar), Some(grab)) = (self.bar(), self.bar_grab) else {
            return;
        };
        let offset = paint::offset_at(&bar, y - grab, self.emu.history());
        self.emu.scroll_to_offset(offset);
        cx.notify();
    }

    fn hover_at(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let found = self.link_at(at);
        self.hover_image(found.as_ref().map(|h| &h.open), at, cx);
        if found != self.hover {
            self.hover = found;
            cx.notify();
        }
    }

    /// The link under a window position, if any. A path counts only if it exists.
    fn link_at(&mut self, at: Point<Pixels>) -> Option<Hover> {
        let grid = self.grid?;
        let text_area = gpui::Bounds::new(
            grid.origin,
            gpui::size(
                grid.cell_w * grid.cols as f32,
                grid.line_h * grid.rows as f32,
            ),
        );
        if !text_area.contains(&at) {
            return None;
        }
        let (row, col, _) = grid.cell(at);
        let line = self.emu.line_text(row);
        let point = self.emu.grid_point(row, col);
        let idx = line.points.iter().position(|p| *p == point)?;
        let link = links::at(&line.text, idx)?;
        let open = match link.target {
            Target::Url(url) => Open::Url(url),
            Target::File { path, line, col } => {
                let path = links::resolve(&path, &self.cwd);
                if !self.exists(&path) {
                    return None;
                }
                Open::File { path, line, col }
            }
            Target::Marker(n) => {
                // a marker known to have no image isn't worth underlining
                if !self.images.claude() || self.images.missing(n) {
                    return None;
                }
                self.ask_marker(n);
                Open::Marker(n)
            }
        };
        Some(Hover {
            range: line.points[link.start]..=line.points[link.end - 1],
            open,
        })
    }

    /// A cached disk check, so sweeping the pointer over output does not stat on every move.
    fn exists(&mut self, path: &PathBuf) -> bool {
        let now = Instant::now();
        if let Some((ok, at)) = self.exists.get(path)
            && now.duration_since(*at) < EXISTS_TTL
        {
            return *ok;
        }
        if self.exists.len() > 500 {
            self.exists.clear();
        }
        let ok = path.is_file();
        self.exists.insert(path.clone(), (ok, now));
        ok
    }

    fn open(&mut self, open: Open, cx: &mut Context<Self>) {
        match open {
            Open::Url(url) => cx.open_url(&url),
            Open::File { path, line, col } => cx.emit(TerminalEvent::OpenFile { path, line, col }),
            Open::Marker(n) => self.open_marker(n, cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_reports_in_both_encodings() {
        assert_eq!(wheel_report(true, 2, 4, true), b"\x1b[<64;5;3M");
        assert_eq!(wheel_report(false, 0, 0, false), b"\x1b[M\x61\x21\x21");
    }
}
