// Images in a terminal, after the Tauri app's ImagePeek: when the pointer rests on an image path
// or on Claude's `[Image #N]`, a preview opens beside it, and Ctrl+click opens the image in the
// viewer. A marker's image comes from the images pasted into the prompt and not sent yet, or else
// from the engine, which finds it in Claude's cache or reads it out of the conversation's
// transcript. Which marker a paste became is read off Claude's input box as it redraws.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, ObjectFit, Pixels, Point, Size, StyledImage, Task, anchored, deferred,
    div, img, point, prelude::*, px,
};
use hyprspace_proto::{Agent, Command, Launch};

use super::links::{self, Target};
use super::mouse::Open;
use super::{TerminalEvent, TerminalView};
use crate::colors;

/// How long the pointer rests before the preview opens, so a sweep across output doesn't flash.
const WAIT: Duration = Duration::from_millis(200);
/// A preview whose image seems gone from under the pointer looks again after each of these
/// waits, and closes only if it never finds it. An agent redrawing its screen blanks the line
/// under a still pointer for a moment, on a beat; uneven waits keep the looks from falling in
/// step with it and landing on a blank every time.
const LOOKS: [u64; 7] = [35, 60, 45, 70, 40, 85, 55];
/// A found image holds a minute. A miss is asked again after a few seconds, since Claude writes
/// the transcript a beat after a message goes. After /clear the numbers restart, so even a hit
/// doesn't hold for good.
const HIT_TTL: Duration = Duration::from_secs(60);
const MISS_TTL: Duration = Duration::from_secs(4);
/// The largest the preview draws an image.
const MAX: (f32, f32) = (360., 260.);
/// Claude swaps a pasted path for its marker a beat later: look this often, this many times.
const LEARN_EVERY: Duration = Duration::from_millis(150);
const LEARN_TRIES: usize = 20;

#[derive(Default)]
pub(super) struct Images {
    claude: bool,
    /// The Claude conversation the session is on.
    conversation: Option<String>,
    /// What each marker came to, and when the engine said.
    found: HashMap<u32, (Option<PathBuf>, Instant)>,
    asked: HashSet<u32>,
    /// Images pasted into the prompt and not sent, each with its marker once Claude shows it.
    tray: Vec<(PathBuf, Option<u32>)>,
    /// The image the pointer rests on, and where the pointer is.
    resting: Option<(PathBuf, Point<Pixels>)>,
    /// Where the pointer last moved in the terminal.
    pointer: Point<Pixels>,
    /// A preview about to close, unless its image is under the pointer again by then.
    closing: Option<Task<()>>,
    /// A marker under the pointer whose image the engine is still finding.
    waiting_on: Option<(u32, Point<Pixels>)>,
    /// A marker that was ctrl+clicked before its image was known.
    open_when_found: Option<u32>,
    pub(super) peek: Option<Peek>,
    wait: Option<Task<()>>,
    learning: Vec<Task<()>>,
}

impl Images {
    pub(super) fn new(run: Option<&Launch>) -> Self {
        let claude = run.is_some_and(|r| r.agent == Agent::Claude);
        Self {
            claude,
            conversation: run.filter(|_| claude).and_then(|r| r.resume.clone()),
            ..Default::default()
        }
    }

    /// Markers mean something only in a Claude session.
    pub(super) fn claude(&self) -> bool {
        self.claude
    }

    /// The image behind marker `n`, if it is known now.
    fn marker(&self, n: u32) -> Option<PathBuf> {
        self.tray
            .iter()
            .find(|(_, m)| *m == Some(n))
            .map(|(p, _)| p.clone())
            .or_else(|| {
                self.found
                    .get(&n)
                    .filter(|(p, at)| at.elapsed() < if p.is_some() { HIT_TTL } else { MISS_TTL })
                    .and_then(|(p, _)| p.clone())
            })
    }

    /// The engine said lately that marker `n` has no image.
    pub(super) fn missing(&self, n: u32) -> bool {
        self.marker(n).is_none()
            && self
                .found
                .get(&n)
                .is_some_and(|(p, at)| p.is_none() && at.elapsed() < MISS_TTL)
    }

    /// Enter sends the prompt and Ctrl+C clears it: either way the pasted images are done.
    pub(super) fn sent(&mut self) {
        self.tray.clear();
        self.learning.clear();
    }
}

/// The preview: an image fitted into the largest size, with its size and how to open it.
pub(super) struct Peek {
    path: PathBuf,
    at: Point<Pixels>,
    size: Option<(u32, u32)>,
}

/// An image's size fitted into `MAX`, never scaled up. A size the header didn't give draws in a
/// middling box.
fn fit(size: Option<(u32, u32)>) -> (f32, f32) {
    let Some((w, h)) = size.filter(|(w, h)| *w > 0 && *h > 0) else {
        return (320., 200.);
    };
    let scale = (MAX.0 / w as f32).min(MAX.1 / h as f32).min(1.);
    ((w as f32 * scale).max(24.), (h as f32 * scale).max(24.))
}

impl Peek {
    pub(super) fn render(&self, viewport: Size<Pixels>) -> AnyElement {
        let (w, h) = fit(self.size);
        // the card goes below right of the pointer, flipped near the window's edges
        let card = (w + 14., h + 36.);
        let x = if f32::from(self.at.x) + 16. + card.0 > f32::from(viewport.width) - 8. {
            f32::from(self.at.x) - 16. - card.0
        } else {
            f32::from(self.at.x) + 16.
        };
        let y = if f32::from(self.at.y) + 16. + card.1 > f32::from(viewport.height) - 8. {
            f32::from(self.at.y) - 16. - card.1
        } else {
            f32::from(self.at.y) + 16.
        };
        let size = self.size.map(|(w, h)| format!("{w} \u{d7} {h}"));
        let open = if cfg!(target_os = "macos") {
            "Cmd+click to open"
        } else {
            "Ctrl+click to open"
        };
        deferred(
            anchored()
                .position(point(px(x.max(8.)), px(y.max(8.))))
                .child(
                    div()
                        .p(px(6.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .rounded(px(10.))
                        .border_1()
                        .border_color(colors::border2())
                        .bg(colors::surface2())
                        .shadow(colors::shadow())
                        .child(
                            img(self.path.clone())
                                .w(px(w))
                                .h(px(h))
                                .rounded(px(6.))
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .gap(px(12.))
                                .px(px(2.))
                                .text_size(px(11.))
                                .text_color(colors::text3())
                                .child(size.unwrap_or_default())
                                .child(open),
                        )
                        .map(|card| crate::slide::ease_in(card, "peek", 120, |d, t| d.opacity(t))),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

impl TerminalView {
    /// Asks the engine for marker `n`'s image unless it is known or already asked for.
    pub(super) fn ask_marker(&mut self, n: u32) {
        let i = &mut self.images;
        if !i.claude || i.asked.contains(&n) || i.marker(n).is_some() || i.missing(n) {
            return;
        }
        i.asked.insert(n);
        self.client.send(Command::FindImage {
            id: self.id,
            cwd: self.cwd.clone(),
            conversation: i.conversation.clone(),
            n,
        });
    }

    /// The engine's answer for marker `n`.
    pub fn image_found(&mut self, n: u32, path: Option<PathBuf>, cx: &mut Context<Self>) {
        let i = &mut self.images;
        i.asked.remove(&n);
        i.found.insert(n, (path.clone(), Instant::now()));
        if i.open_when_found == Some(n) {
            i.open_when_found = None;
            if let Some(path) = path.clone() {
                cx.emit(TerminalEvent::OpenFile {
                    path,
                    line: None,
                    col: None,
                });
            }
        }
        if let Some((m, at)) = i.waiting_on
            && m == n
        {
            i.waiting_on = None;
            self.rest_on(path.map(|p| (p, at)), cx);
        }
        cx.notify();
    }

    /// What the pointer is over now: an image path or a marker opens the preview after a wait.
    pub(super) fn hover_image(
        &mut self,
        open: Option<&Open>,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.images.pointer = at;
        let image = self.image_in(open, at);
        if image.is_none() && self.images.resting.is_some() {
            self.linger(cx);
            return;
        }
        self.rest_on(image.map(|p| (p, at)), cx);
    }

    /// A preview, open or about to open, whose image seems gone from under the pointer stays while
    /// it looks again a few times. An image found under the pointer keeps it, or moves it to that
    /// image; finding none every time closes it.
    fn linger(&mut self, cx: &mut Context<Self>) {
        if self.images.closing.is_some() {
            return;
        }
        self.images.closing = Some(cx.spawn(async move |this, cx| {
            for ms in LOOKS {
                cx.background_executor()
                    .timer(Duration::from_millis(ms))
                    .await;
                let back = this
                    .update(cx, |v, cx| {
                        let at = v.images.pointer;
                        let found = v.link_at(at);
                        let image = v.image_in(found.as_ref().map(|h| &h.open), at)?;
                        v.rest_on(Some((image, at)), cx);
                        Some(())
                    })
                    .map_or(true, |b| b.is_some());
                if back {
                    return;
                }
            }
            let _ = this.update(cx, |v, cx| v.rest_on(None, cx));
        }));
    }

    /// The image a link under the pointer stands for. A marker whose image isn't known yet is
    /// asked for, and opens when the engine answers.
    fn image_in(&mut self, open: Option<&Open>, at: Point<Pixels>) -> Option<PathBuf> {
        self.images.waiting_on = None;
        match open {
            Some(Open::File { path, .. }) if crate::attach::is_image(path) => Some(path.clone()),
            Some(Open::Marker(n)) => {
                let known = self.images.marker(*n);
                if known.is_none() {
                    self.images.waiting_on = Some((*n, at));
                    self.ask_marker(*n);
                }
                known
            }
            _ => None,
        }
    }

    /// The pointer came to rest on an image, or left one. Resting on the same image again keeps
    /// the preview where it is.
    pub(super) fn rest_on(
        &mut self,
        image: Option<(PathBuf, Point<Pixels>)>,
        cx: &mut Context<Self>,
    ) {
        self.images.closing = None;
        let same = match (&image, &self.images.resting) {
            (Some((a, _)), Some((b, _))) => a == b,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.images.resting = image.clone();
        self.images.peek = None;
        self.images.wait = None;
        cx.notify();
        let Some((path, at)) = image else {
            return;
        };
        self.images.wait = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(WAIT).await;
            // the size is read off the file's header, away from the UI thread
            let file = path.clone();
            let size = cx
                .background_executor()
                .spawn(async move { image::image_dimensions(&file).ok() })
                .await;
            let _ = this.update(cx, |v, cx| {
                if v.images.resting.as_ref().is_some_and(|(r, _)| *r == path) {
                    v.images.peek = Some(Peek { path, at, size });
                    cx.notify();
                }
            });
        }));
    }

    /// Ctrl+click on a marker: its image opens in the viewer, now or once it is found.
    pub(super) fn open_marker(&mut self, n: u32, cx: &mut Context<Self>) {
        match self.images.marker(n) {
            Some(path) => cx.emit(TerminalEvent::OpenFile {
                path,
                line: None,
                col: None,
            }),
            None => {
                self.images.open_when_found = Some(n);
                self.images.found.remove(&n);
                self.ask_marker(n);
            }
        }
    }

    /// The markers in Claude's input box before a paste, so the one the paste becomes stands out.
    pub(super) fn before_paste(&self) -> Option<HashSet<u32>> {
        self.images.claude.then(|| self.prompt_markers()).flatten()
    }

    /// An image went into Claude's prompt: keep it, and learn the marker Claude gives it.
    pub(super) fn pasted(
        &mut self,
        path: PathBuf,
        before: Option<HashSet<u32>>,
        cx: &mut Context<Self>,
    ) {
        if !self.images.claude {
            return;
        }
        self.images.tray.push((path.clone(), None));
        let Some(before) = before else {
            return;
        };
        let learn = cx.spawn(async move |this, cx| {
            for _ in 0..LEARN_TRIES {
                cx.background_executor().timer(LEARN_EVERY).await;
                let done = this
                    .update(cx, |v, _| {
                        let Some(now) = v.prompt_markers() else {
                            return false;
                        };
                        let taken: HashSet<u32> =
                            v.images.tray.iter().filter_map(|(_, m)| *m).collect();
                        let mut new: Vec<u32> = now
                            .difference(&before)
                            .copied()
                            .filter(|n| !taken.contains(n))
                            .collect();
                        new.sort();
                        let Some(&n) = new.first() else {
                            return false;
                        };
                        if let Some(entry) = v
                            .images
                            .tray
                            .iter_mut()
                            .find(|(p, m)| *p == path && m.is_none())
                        {
                            entry.1 = Some(n);
                        }
                        true
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        });
        self.images.learning.push(learn);
    }

    /// One screen row's text, without the rest of its wrapped line.
    fn row_text(&self, row: usize) -> String {
        let line = self.emu.line_text(row);
        let here = self.emu.grid_point(row, 0).line;
        line.text
            .chars()
            .zip(&line.points)
            .filter(|(_, p)| p.line == here)
            .map(|(c, _)| c)
            .collect()
    }

    /// The markers in Claude's input box as drawn: the `❯` line right under a rule, down to the
    /// next rule. None when no input box is on screen (a permission dialog, a picker).
    fn prompt_markers(&self) -> Option<HashSet<u32>> {
        let rows = self.emu.size().1 as usize;
        let rule = |t: &str| t.chars().filter(|c| matches!(c, '─' | '━' | '═')).count() >= 8;
        for y in (1..rows).rev() {
            let text = self.row_text(y);
            let lead = text.trim_start_matches(|c: char| c.is_whitespace() || c == '│');
            let mut chars = lead.chars();
            let prompt = matches!(chars.next(), Some('❯' | '>'))
                && chars.next().is_some_and(char::is_whitespace);
            if !prompt || !rule(&self.row_text(y - 1)) {
                continue;
            }
            // Claude wraps its input at spaces, so a marker can break as "[Image" and "#14]"
            let mut joined = Vec::new();
            for k in y..rows {
                let row = self.row_text(k);
                if rule(&row) {
                    break;
                }
                joined.push(row.trim().to_string());
            }
            let joined = joined.join(" ");
            return Some(
                links::all(&joined)
                    .into_iter()
                    .filter_map(|l| match l.target {
                        Target::Marker(n) => Some(n),
                        _ => None,
                    })
                    .collect(),
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_shrink_to_fit_and_never_grow() {
        assert_eq!(fit(Some((1920, 1080))), (360., 202.5));
        assert_eq!(fit(Some((100, 50))), (100., 50.));
        assert_eq!(fit(Some((400, 2600))), (40., 260.));
        assert_eq!(fit(None), (320., 200.));
    }

    #[test]
    fn a_marker_comes_from_the_tray_first_and_a_miss_is_remembered_briefly() {
        let mut i = Images {
            claude: true,
            ..Default::default()
        };
        i.tray.push((PathBuf::from("/t/clip-1.png"), Some(4)));
        i.found
            .insert(4, (Some(PathBuf::from("/c/4.png")), Instant::now()));
        i.found.insert(5, (None, Instant::now()));
        assert_eq!(i.marker(4), Some(PathBuf::from("/t/clip-1.png")));
        assert!(i.missing(5) && !i.missing(4) && !i.missing(6));
        i.sent();
        assert_eq!(i.marker(4), Some(PathBuf::from("/c/4.png")));
    }
}
