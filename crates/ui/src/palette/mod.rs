// The command palette (Ctrl+K, or Ctrl+Shift+P, anywhere, terminals too): commands grouped
// into sections, every open thread, and with two letters or more, the text in every terminal.
// Follows the Tauri app's CommandPalette.tsx and command-palette.css. The root builds the items
// and runs the one picked (`commands.rs`); this file filters, moves the highlight and draws.

mod commands;

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, KeyBinding, MouseButton, Render, ScrollHandle, SharedString,
    Subscription, Window, actions, div, prelude::*, px, relative,
};
use hyprspace_proto::Agent;
use hyprspace_theme::MONO;

pub use commands::Cmd;

use crate::assets::{icon, mark};
use crate::colors;
use crate::input::{InputEvent, TextInput};

actions!(palette, [TogglePalette, SelectPrev, SelectNext]);

/// The section terminal hits go under.
pub const IN_TERMINALS: &str = "In terminal output";

/// Binds the palette's keys. Call after `input::bind_keys`, so its arrows win inside the
/// palette's own text box.
pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        // a terminal gives up Ctrl+K, a shell's kill-to-end-of-line, so the palette opens from
        // anywhere (Ash's call)
        KeyBinding::new("secondary-k", TogglePalette, None),
        KeyBinding::new("secondary-shift-p", TogglePalette, None),
        KeyBinding::new("up", SelectPrev, Some("Palette > TextInput")),
        KeyBinding::new("down", SelectNext, Some("Palette > TextInput")),
    ]);
}

#[derive(Clone)]
pub enum Glyph {
    Icon(&'static str),
    Mark(Agent),
}

#[derive(Clone)]
pub struct Item {
    pub section: &'static str,
    pub label: String,
    /// A folder, or a snippet for a terminal hit.
    pub sub: Option<String>,
    /// The shortcut, written the Windows way.
    pub hint: Option<&'static str>,
    pub glyph: Glyph,
    pub cmd: Cmd,
}

impl Item {
    pub fn new(section: &'static str, label: impl Into<String>, glyph: Glyph, cmd: Cmd) -> Self {
        Self {
            section,
            label: label.into(),
            sub: None,
            hint: None,
            glyph,
            cmd,
        }
    }

    pub fn sub(mut self, sub: impl Into<String>) -> Self {
        self.sub = Some(sub.into());
        self
    }

    pub fn hint(mut self, hint: &'static str) -> Self {
        self.hint = Some(hint);
        self
    }
}

/// Every word has to appear somewhere in the label, the section or the line under it.
pub fn filter(items: &[Item], query: &str) -> Vec<Item> {
    let q = query.to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    items
        .iter()
        .filter(|c| {
            let hay = format!(
                "{} {} {}",
                c.label,
                c.section,
                c.sub.as_deref().unwrap_or("")
            )
            .to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .cloned()
        .collect()
}

pub enum PaletteEvent {
    /// The text changed; the root answers with terminal hits.
    Query(String),
    Run(Cmd),
    Close,
}

pub struct Palette {
    input: Entity<TextInput>,
    commands: Vec<Item>,
    shown: Vec<Item>,
    hits: Vec<Item>,
    sel: usize,
    scroll: ScrollHandle,
    /// Where keyboard focus goes back to when the palette closes.
    pub(crate) back: Option<FocusHandle>,
    _sub: Subscription,
}

impl EventEmitter<PaletteEvent> for Palette {}

impl Palette {
    pub fn new(commands: Vec<Item>, back: Option<FocusHandle>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(
                "Type a command, a thread, or text from a terminal",
                false,
                cx,
            )
        });
        let sub = cx.subscribe(&input, |p, input, e: &InputEvent, cx| match e {
            InputEvent::Changed => {
                let q = input.read(cx).text().to_string();
                p.shown = filter(&p.commands, &q);
                p.hits.clear();
                p.sel = 0;
                p.reveal();
                cx.emit(PaletteEvent::Query(q));
                cx.notify();
            }
            InputEvent::Submit => p.run(p.sel, cx),
            InputEvent::Cancel => cx.emit(PaletteEvent::Close),
            InputEvent::Images(_) => {}
        });
        Self {
            input,
            shown: commands.clone(),
            commands,
            hits: Vec::new(),
            sel: 0,
            scroll: ScrollHandle::new(),
            back,
            _sub: sub,
        }
    }

    /// Terminal hits for the query now in the box.
    pub fn set_hits(&mut self, hits: Vec<Item>, cx: &mut Context<Self>) {
        self.hits = hits;
        cx.notify();
    }

    pub fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text().to_string()
    }

    fn items(&self) -> impl Iterator<Item = &Item> {
        self.shown.iter().chain(self.hits.iter())
    }

    fn len(&self) -> usize {
        self.shown.len() + self.hits.len()
    }

    fn run(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.items().nth(ix) {
            cx.emit(PaletteEvent::Run(item.cmd.clone()));
        }
    }

    /// Keeps the highlighted row in view as the arrows walk past the edge of the list. The
    /// list's children are rows and section labels, so the row's child index counts both.
    fn reveal(&self) {
        let mut child = 0;
        let mut section = "";
        for (i, item) in self.items().enumerate() {
            if item.section != section {
                section = item.section;
                child += 1;
            }
            if i == self.sel {
                self.scroll.scroll_to_item(child);
                return;
            }
            child += 1;
        }
    }

    fn prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.sel = self.sel.saturating_sub(1);
        self.reveal();
        cx.notify();
    }

    fn next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.sel = (self.sel + 1).min(self.len().saturating_sub(1));
        self.reveal();
        cx.notify();
    }
}

impl Focusable for Palette {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

/// A shortcut as keycaps: Ctrl, Shift, G on Windows; the glyphs on macOS.
fn keycaps(hint: &str, small: bool) -> AnyElement {
    let caps = hint.split('+').map(|k| {
        let k = if cfg!(target_os = "macos") {
            match k {
                "Ctrl" => "⌘",
                "Shift" => "⇧",
                "Alt" => "⌥",
                k => k,
            }
        } else {
            k
        };
        keycap(k, small)
    });
    div()
        .flex()
        .flex_none()
        .gap(px(3.))
        .children(caps)
        .into_any_element()
}

fn keycap(label: &str, small: bool) -> AnyElement {
    let side = if small { 18. } else { 20. };
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(side))
        .h(px(side))
        .px(px(5.))
        .rounded(px(5.))
        .bg(colors::ink(0.06))
        .border_1()
        .border_b_2()
        .border_color(colors::ink(0.08))
        .text_size(px(if small { 10. } else { 10.5 }))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(SharedString::from(label.to_string()))
        .into_any_element()
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = colors::accent();
        let q = self.query(cx);
        let mut list = div()
            .id("palette-list")
            .track_scroll(&self.scroll)
            .max_h(px(430.).min(window.viewport_size().height * 0.56))
            .overflow_y_scroll()
            .pt(px(6.))
            .px(px(8.))
            .pb(px(8.))
            .flex()
            .flex_col();
        if self.len() == 0 {
            list = list.child(
                div()
                    .py(px(28.))
                    .px(px(16.))
                    .flex()
                    .justify_center()
                    .text_size(px(13.))
                    .text_color(colors::text3())
                    .child(if q.trim().chars().count() < 2 {
                        "No commands match"
                    } else {
                        "Nothing matches, in commands or in any terminal"
                    }),
            );
        }
        let mut section = "";
        let items: Vec<Item> = self.items().cloned().collect();
        for (i, item) in items.into_iter().enumerate() {
            if item.section != section {
                section = item.section;
                list = list.child(
                    div()
                        .pt(px(10.))
                        .px(px(10.))
                        .pb(px(5.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text3())
                        .child(section),
                );
            }
            let on = i == self.sel;
            let glyph_color = if on {
                accent.blend(colors::text1().opacity(0.4))
            } else {
                colors::text2()
            };
            let glyph: AnyElement = match item.glyph {
                Glyph::Icon(name) => icon(name, 15., glyph_color).into_any_element(),
                Glyph::Mark(agent) => mark(agent, 14., colors::brand(agent).0).into_any_element(),
            };
            let mono = item.section == IN_TERMINALS;
            list = list.child(
                div()
                    .id(("palette-item", i))
                    .flex()
                    .items_center()
                    .gap(px(11.))
                    .min_h(px(38.))
                    .py(px(5.))
                    .pl(px(6.))
                    .pr(px(10.))
                    .rounded(px(9.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(accent.opacity(0.14)))
                    .on_mouse_move(cx.listener(move |p, _, _, cx| {
                        if p.sel != i {
                            p.sel = i;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |p, _: &ClickEvent, _, cx| p.run(i, cx)))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(28.))
                            .rounded(px(7.))
                            .bg(if on {
                                accent.opacity(0.22)
                            } else {
                                colors::ink(0.05)
                            })
                            .child(glyph),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.))
                                    .line_height(px(17.))
                                    .text_color(colors::text1())
                                    .child(item.label.clone()),
                            )
                            .children(item.sub.clone().map(|s| {
                                div()
                                    .truncate()
                                    .text_size(px(if mono { 11. } else { 11.5 }))
                                    .line_height(px(15.))
                                    .when(mono, |d| d.font_family(MONO))
                                    .text_color(colors::text3())
                                    .child(s)
                            })),
                    )
                    .children(item.hint.map(|h| keycaps(h, false))),
            );
        }
        let foot_item = |caps: Vec<&str>, what: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .children(caps.into_iter().map(|c| keycap(c, true)))
                .child(what)
        };
        div()
            .id("palette")
            .key_context("Palette")
            .on_action(cx.listener(Self::prev))
            .on_action(cx.listener(Self::next))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .w(px(620.))
            .max_w(relative(0.92))
            .flex()
            .flex_col()
            .rounded(px(14.))
            .border_1()
            .border_color(colors::border2())
            .bg(colors::surface2())
            .shadow(colors::shadow())
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(11.))
                    .h(px(52.))
                    .pl(px(18.))
                    .pr(px(14.))
                    .border_b_1()
                    .border_color(colors::border1())
                    .child(icon("search", 16., accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(15.))
                            .child(self.input.clone()),
                    )
                    .child(keycap("Esc", false)),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .px(px(16.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(colors::border1())
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(foot_item(vec!["↑", "↓"], "move"))
                    .child(foot_item(vec!["↵"], "run"))
                    .child(foot_item(vec!["Esc"], "close")),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_must_match_somewhere() {
        let items = vec![
            Item::new(
                "Start",
                "New thread",
                Glyph::Icon("square-pen"),
                Cmd::NewThread,
            ),
            Item::new(
                "View",
                "Open the git panel",
                Glyph::Icon("git-branch"),
                Cmd::GitPanel,
            ),
            Item::new(
                "Threads",
                "Fix login",
                Glyph::Mark(Agent::Claude),
                Cmd::Thread(4),
            )
            .sub("api-server"),
        ];
        let labels =
            |q: &str| -> Vec<String> { filter(&items, q).into_iter().map(|i| i.label).collect() };
        assert_eq!(labels("").len(), 3);
        assert_eq!(labels("GIT"), ["Open the git panel"]);
        assert_eq!(labels("thread"), ["New thread", "Fix login"]);
        assert_eq!(labels("login api"), ["Fix login"]);
        assert!(labels("login web").is_empty());
    }
}
