// What the palette offers and what each entry does, built fresh each time it opens from the
// root's state. Starting work goes through the composer: New thread opens it there, and it picks
// the agent, model and effort, so there are no per-agent entries.

use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, IntoElement, Window, anchored, deferred,
    div, point, prelude::*, px,
};
use hyprspace_proto::state::{DockTab, Scheme};
use hyprspace_proto::{Opener, Pane};
use hyprspace_theme::THEMES;

use super::{Glyph, IN_TERMINALS, Item, Palette, PaletteEvent, TogglePalette};
use crate::panes::layout;
use crate::root::{Action, Root, Screen, View};

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    NewThread,
    NewTerminal,
    OpenFolder,
    Thread(u64),
    ClosePane,
    MaxPane,
    Layout(&'static str),
    ToggleDock,
    GitPanel,
    Scheme(Scheme),
    Theme(&'static str),
    OpenIn(Opener),
    Settings,
    Intro,
}

impl Root {
    /// The focused pane of the space on screen, when a thread is on screen.
    fn focused_pane(&self) -> Option<(u64, Pane)> {
        let Screen::Thread(id) = self.screen else {
            return None;
        };
        let space = self.state.thread(id)?.0;
        let pane = space
            .grid
            .focus
            .clone()
            .filter(|p| space.grid.has(p))
            .unwrap_or(Pane::Thread { id });
        Some((space.id, pane))
    }

    fn palette_items(&self, window: &Window) -> Vec<Item> {
        let space = self.current_space();
        let mut out = vec![
            Item::new(
                "Start",
                "New thread",
                Glyph::Icon("square-pen"),
                Cmd::NewThread,
            ),
            Item::new(
                "Start",
                "New terminal",
                Glyph::Icon("square-terminal"),
                Cmd::NewTerminal,
            ),
            Item::new(
                "Start",
                "Open a folder",
                Glyph::Icon("folder-plus"),
                Cmd::OpenFolder,
            )
            .sub("Adds it to the sidebar"),
        ];
        // every thread by its title, under its folder. Archived ones are parked on purpose.
        let current = match self.screen {
            Screen::Thread(id) => Some(id),
            _ => None,
        };
        for s in self.state.spaces.iter().filter(|s| !s.archived) {
            for t in s.threads.iter().filter(|t| !t.archived) {
                let glyph = match t.agent() {
                    Some(l) => Glyph::Mark(l.agent),
                    None => Glyph::Icon("square-terminal"),
                };
                let title = if t.title.is_empty() {
                    "New thread"
                } else {
                    &t.title
                };
                let sub = if current == Some(t.id) {
                    format!("{} · current", s.name)
                } else {
                    s.name.clone()
                };
                out.push(Item::new("Threads", title, glyph, Cmd::Thread(t.id)).sub(sub));
            }
        }
        if let Some((space, _)) = self.focused_pane() {
            out.push(Item::new(
                "Panes",
                "Close the focused pane",
                Glyph::Icon("x"),
                Cmd::ClosePane,
            ));
            out.push(Item::new(
                "Panes",
                "Maximize or restore the focused pane",
                Glyph::Icon("maximize-2"),
                Cmd::MaxPane,
            ));
            let n = self.live_panes(space).len();
            let picked = self
                .state
                .space(space)
                .and_then(|s| s.grid.layouts.get(&n).cloned());
            let now = layout::resolve(n, picked.as_deref()).id;
            for l in layout::presets(n) {
                let item = Item::new(
                    "Layout",
                    format!("Layout: {}", l.label),
                    Glyph::Icon("layout-grid"),
                    Cmd::Layout(l.id),
                );
                out.push(if l.id == now {
                    item.sub("Current")
                } else {
                    item
                });
            }
        }
        let dark = crate::colors::dark(self.state.appearance.scheme, window.appearance());
        out.push(
            Item::new(
                "View",
                "Show or hide files and git",
                Glyph::Icon("panel-right"),
                Cmd::ToggleDock,
            )
            .hint("Ctrl+Shift+G"),
        );
        out.push(Item::new(
            "View",
            "Open the git panel",
            Glyph::Icon("git-branch"),
            Cmd::GitPanel,
        ));
        out.push(if dark {
            Item::new(
                "View",
                "Switch to light mode",
                Glyph::Icon("sun"),
                Cmd::Scheme(Scheme::Light),
            )
        } else {
            Item::new(
                "View",
                "Switch to dark mode",
                Glyph::Icon("moon"),
                Cmd::Scheme(Scheme::Dark),
            )
        });
        for t in THEMES
            .iter()
            .filter(|t| t.id != self.state.appearance.theme)
        {
            out.push(
                Item::new(
                    "Theme",
                    format!("{} theme", t.name),
                    Glyph::Icon("palette"),
                    Cmd::Theme(t.id),
                )
                .sub(t.blurb),
            );
        }
        if let Some(folder) = space.and_then(|s| self.state.space(s)?.cwd.clone()) {
            for o in &self.work.openers {
                out.push(
                    Item::new(
                        "Open",
                        format!("Open in {}", o.name()),
                        Glyph::Icon("external-link"),
                        Cmd::OpenIn(*o),
                    )
                    .sub(crate::panes::short(&folder)),
                );
            }
        }
        out.push(Item::new(
            "Settings",
            "Settings",
            Glyph::Icon("settings"),
            Cmd::Settings,
        ));
        out.push(
            Item::new(
                "Settings",
                "Replay the intro",
                Glyph::Icon("sparkles"),
                Cmd::Intro,
            )
            .sub("What HyprSpace is and how it fits together"),
        );
        out
    }

    /// Terminal threads whose scrollback holds `query`, newest line per thread.
    fn terminal_hits(&self, query: &str, cx: &gpui::App) -> Vec<Item> {
        if query.trim().chars().count() < 2 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for s in &self.state.spaces {
            for t in &s.threads {
                let Some(View::Terminal(v)) = self.views.get(&t.id) else {
                    continue;
                };
                if let Some(snippet) = v.read(cx).snippet(query.trim()) {
                    let title = if t.title.is_empty() {
                        "Terminal"
                    } else {
                        &t.title
                    };
                    out.push(
                        Item::new(
                            IN_TERMINALS,
                            format!("{} › {title}", s.name),
                            Glyph::Icon("text-search"),
                            Cmd::Thread(t.id),
                        )
                        .sub(snippet),
                    );
                }
            }
        }
        out
    }

    pub(crate) fn toggle_palette(
        &mut self,
        _: &TogglePalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(p) = self.palette.take() {
            let back = p.read(cx).back.clone();
            if let Some(f) = back {
                window.focus(&f, cx);
            }
            cx.notify();
            return;
        }
        let items = self.palette_items(window);
        let back = window.focused(cx);
        let palette = cx.new(|cx| Palette::new(items, back, cx));
        cx.subscribe_in(&palette, window, Self::on_palette).detach();
        let focus = palette.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.palette = Some(palette);
        self.menu = None;
        cx.notify();
    }

    fn on_palette(
        &mut self,
        palette: &Entity<Palette>,
        e: &PaletteEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match e {
            PaletteEvent::Query(q) => {
                let hits = self.terminal_hits(q, cx);
                palette.update(cx, |p, cx| p.set_hits(hits, cx));
            }
            PaletteEvent::Close => self.toggle_palette(&TogglePalette, window, cx),
            PaletteEvent::Run(cmd) => {
                let cmd = cmd.clone();
                self.toggle_palette(&TogglePalette, window, cx);
                self.run_command(cmd, window, cx);
            }
        }
    }

    fn run_command(&mut self, cmd: Cmd, window: &mut Window, cx: &mut Context<Self>) {
        let space = self.current_space();
        match cmd {
            Cmd::NewThread => match space {
                Some(s) => self.act(Action::NewThread(s), window, cx),
                None => self.pick_thread_folder(window, cx),
            },
            Cmd::NewTerminal => match space {
                Some(s) => self.act(Action::NewTerminal(s), window, cx),
                None => self.pick_thread_folder(window, cx),
            },
            Cmd::OpenFolder => self.pick_thread_folder(window, cx),
            Cmd::Thread(id) => self.open_thread(id, window, cx),
            Cmd::ClosePane => {
                if let Some((space, pane)) = self.focused_pane() {
                    self.close_pane(space, pane, window, cx);
                }
            }
            Cmd::MaxPane => {
                if let Some((space, pane)) = self.focused_pane() {
                    self.toggle_max(space, pane, cx);
                }
            }
            Cmd::Layout(id) => {
                if let Some((space, _)) = self.focused_pane() {
                    let n = self.live_panes(space).len();
                    self.set_layout(space, n, id, cx);
                }
            }
            Cmd::ToggleDock => self.toggle_dock(&crate::panes::ToggleDock, window, cx),
            Cmd::GitPanel => {
                self.state.dock.open = true;
                self.state.dock.tab = DockTab::Git;
                self.save();
            }
            Cmd::Scheme(scheme) => {
                self.state.appearance.scheme = scheme;
                self.apply_theme(window);
                self.save();
            }
            Cmd::Theme(id) => {
                self.state.appearance.theme = id.to_string();
                self.apply_theme(window);
                self.save();
            }
            Cmd::OpenIn(opener) => {
                if let Some(folder) = space.and_then(|s| self.state.space(s)?.cwd.clone()) {
                    self.open_in(opener, &folder, cx);
                }
            }
            Cmd::Settings => self.open_settings(window, cx),
            Cmd::Intro => self.show_intro(window, cx),
        }
        cx.notify();
    }

    /// The palette over a dimmed window, a little above center.
    pub(crate) fn palette_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let palette = self.palette.clone()?;
        let size = window.viewport_size();
        let scrim = crate::colors::hsla(crate::colors::theme().shadow);
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("palette-scrim")
                        .w(size.width)
                        .h(size.height)
                        .occlude()
                        .bg(scrim.opacity(0.64))
                        .flex()
                        .items_start()
                        .justify_center()
                        .pt(size.height * 0.13)
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|r, _, window, cx| {
                                r.toggle_palette(&TogglePalette, window, cx)
                            }),
                        )
                        .child(palette),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}
