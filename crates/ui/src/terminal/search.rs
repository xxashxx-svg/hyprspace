// The find bar (Ctrl+F): a query box over the top of the terminal, matches highlighted in the
// grid, Enter and Shift+Enter (or the arrows) to step through them, Aa for case. Laid out after
// the Tauri app's TerminalSearch and its .term-search styles.

use alacritty_terminal::term::search::Match;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, FontWeight, IntoElement, Subscription, div,
    prelude::*, px,
};

use super::TerminalView;
use super::emulator::Emulator;
use crate::colors;
use crate::input::{InputEvent, Newline, TextInput};
use crate::widgets;

pub struct Find {
    pub input: Entity<TextInput>,
    pub case: bool,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    _sub: Subscription,
}

impl Find {
    pub fn new(cx: &mut Context<TerminalView>) -> Self {
        let input = cx.new(|cx| TextInput::new("Search", false, cx));
        let sub = cx.subscribe(
            &input,
            |v: &mut TerminalView, _, e: &InputEvent, cx| match e {
                InputEvent::Changed => v.refind(true, cx),
                InputEvent::Submit => v.step(true, cx),
                InputEvent::Cancel => v.close_find(cx),
                InputEvent::Images(_) => {}
            },
        );
        Self {
            input,
            case: false,
            matches: Vec::new(),
            current: None,
            _sub: sub,
        }
    }

    /// Runs the query again over the whole scrollback. `jump` moves to the match nearest the
    /// bottom, as typing a new query does; new output keeps the current match where it was.
    pub fn run(&mut self, emu: &mut Emulator, jump: bool, query: &str) {
        let before = self.current.and_then(|i| self.matches.get(i)).cloned();
        self.matches = emu.find(query, self.case);
        self.current = if self.matches.is_empty() {
            None
        } else if jump {
            Some(self.matches.len() - 1)
        } else {
            before
                .and_then(|m| self.matches.iter().position(|x| *x == m))
                .or(Some(self.matches.len() - 1))
        };
        if jump && let Some(m) = self.current.map(|i| &self.matches[i]) {
            emu.reveal_match(m);
        }
    }

    /// The next match down (`down`) or up, wrapping around.
    pub fn step(&mut self, emu: &mut Emulator, down: bool) {
        let n = self.matches.len();
        if n == 0 {
            return;
        }
        let i = self.current.unwrap_or(n - 1);
        let next = if down { (i + 1) % n } else { (i + n - 1) % n };
        self.current = Some(next);
        emu.reveal_match(&self.matches[next]);
    }

    pub fn render(&self, cx: &mut Context<TerminalView>) -> AnyElement {
        let count = match (self.current, self.matches.len()) {
            (_, 0) if !self.input.read(cx).text().is_empty() => "No results".to_string(),
            (_, 0) => String::new(),
            (Some(i), n) => format!("{} of {n}", i + 1),
            (None, n) => n.to_string(),
        };
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .h(px(24.))
                .min_w(px(26.))
                .px(px(7.))
                .rounded(px(5.))
                .border_1()
                .border_color(colors::border2())
                .bg(colors::surface1())
                .text_size(px(12.))
                .text_color(colors::text2())
                .cursor_pointer()
                .hover(|s| {
                    s.text_color(colors::text1())
                        .border_color(colors::border1())
                })
                .child(label)
        };
        div()
            .id("term-find")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .flex()
            .items_center()
            .gap(px(5.))
            .px(px(8.))
            .py(px(6.))
            .bg(colors::surface2().opacity(0.94))
            .border_b_1()
            .border_color(colors::border2())
            // Shift+Enter goes up. The box binds it to Newline, which a one-line box would turn
            // into another Enter, so take the action before it gets there.
            .capture_action(cx.listener(|v, _: &Newline, _, cx| {
                v.step(false, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w(px(100.))
                    .max_w(px(280.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(colors::accent())
                    .bg(colors::surface1())
                    .text_size(px(12.))
                    .text_color(colors::text1())
                    .child(self.input.clone()),
            )
            .child(
                div()
                    .min_w(px(64.))
                    .text_size(px(11.5))
                    .text_color(colors::text3())
                    .child(count),
            )
            .child(
                button("term-find-case", "Aa")
                    .when(self.case, |d| {
                        d.bg(colors::accent_dim())
                            .border_color(colors::text3())
                            .text_color(colors::text1())
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.toggle_case(cx))),
            )
            .child(
                button("term-find-up", "\u{2191}")
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.step(false, cx))),
            )
            .child(
                button("term-find-down", "\u{2193}")
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.step(true, cx))),
            )
            .child(div().flex_1())
            .child(
                widgets::icon_button("term-find-close", "x", 24.)
                    .on_click(cx.listener(|v, _: &ClickEvent, _, cx| v.close_find(cx))),
            )
            .into_any_element()
    }
}
