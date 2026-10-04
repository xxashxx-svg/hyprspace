// The intro's steps after the frame: welcome, agents, the two tours, defaults, and the first
// folder. `mod.rs` draws the frame, the step list and the buttons under them.

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, Div, FontWeight, IntoElement, Window, div,
    prelude::*, px,
};
use hyprspace_proto::state::Scheme;
use hyprspace_proto::{Agent, Command, Permission};
use hyprspace_theme::THEMES;

use super::sketches;
use crate::assets::{icon, mark};
use crate::root::Root;
use crate::{colors, widgets};

const INSTALL: [(Agent, &str); 3] = [
    (Agent::Claude, "npm install -g @anthropic-ai/claude-code"),
    (Agent::Codex, "npm install -g @openai/codex"),
    (Agent::Gemini, "npm install -g @google/gemini-cli"),
];

/// Each mode, most careful first, in the words Settings' Defaults uses.
const MODES: [(Permission, &str, &str, &str); 4] = [
    (
        Permission::Plan,
        "Plan only",
        "list-checks",
        "Reads and plans. It changes nothing.",
    ),
    (
        Permission::Ask,
        "Ask first",
        "hand",
        "Asks before every edit and command.",
    ),
    (
        Permission::Auto,
        "Auto edit",
        "file-pen-line",
        "Edits files in the folder on its own. Asks before anything else.",
    ),
    (
        Permission::Bypass,
        "Full access",
        "shield-off",
        "Never asks. Use it only in folders you trust.",
    ),
];

const SCHEMES: [(Scheme, &str); 3] = [
    (Scheme::Dark, "Dark"),
    (Scheme::Light, "Light"),
    (Scheme::System, "Match the system"),
];

/// Shortcuts that exist in this app, written the Windows way.
const SHORTCUTS: [(&str, &str); 6] = [
    ("Ctrl+K", "Command palette"),
    ("Ctrl+Shift+P", "Command palette, too"),
    ("Ctrl+Shift+G", "Files and git"),
    ("Ctrl+F", "Find in a terminal"),
    ("Ctrl+click", "Open a thread beside the others"),
    ("Esc", "Leave Settings"),
];

struct Topic {
    title: &'static str,
    sub: &'static str,
    hint: &'static str,
    sketch: fn() -> AnyElement,
}

const BASICS: [Topic; 4] = [
    Topic {
        title: "Folders",
        sub: "The sidebar lists them",
        hint: "Click a folder to fold it. The dot on a thread shows if the agent is working or waiting on you.",
        sketch: sketches::folders,
    },
    Topic {
        title: "Threads",
        sub: "One agent, one task",
        hint: "Press the square button and pick a folder. The + above the panes uses the folder you're in.",
        sketch: sketches::threads,
    },
    Topic {
        title: "The composer",
        sub: "Where every thread starts",
        hint: "Pick an agent and effort, type a task, press Enter. Past conversations in the folder show under the box.",
        sketch: sketches::composer,
    },
    Topic {
        title: "Panes",
        sub: "Threads tile into a grid",
        hint: "Drag a header onto another pane to swap them. Double-click a header to maximize, again to restore.",
        sketch: sketches::panes,
    },
];

const TOOLS: [Topic; 4] = [
    Topic {
        title: "Command palette",
        sub: "Ctrl K",
        hint: "Type to filter commands and threads. Two letters or more also searches the text in every terminal.",
        sketch: sketches::palette,
    },
    Topic {
        title: "Terminals",
        sub: "Real ones, with extras",
        hint: "Hold Ctrl and click a file path to open it. Ctrl+F finds text in the scrollback.",
        sketch: sketches::terminal,
    },
    Topic {
        title: "Files and git",
        sub: "Ctrl+Shift+G",
        hint: "Tick files, write a summary, commit. Push shows up once you're ahead.",
        sketch: sketches::git,
    },
    Topic {
        title: "Usage",
        sub: "Above the panes",
        hint: "How much of each plan's limits is left, and when they reset.",
        sketch: sketches::usage,
    },
];

/// Where the intro is.
fn h1(text: &str) -> Div {
    div()
        .mt(px(-6.))
        .text_size(px(24.))
        .line_height(px(29.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text1())
        .child(text.to_string())
}

fn lead(text: &str) -> Div {
    div()
        .max_w(px(560.))
        .text_size(px(13.5))
        .line_height(px(21.))
        .text_color(colors::text2())
        .child(text.to_string())
}

fn aside(text: &str) -> Div {
    div()
        .max_w(px(620.))
        .text_size(px(12.5))
        .line_height(px(20.))
        .text_color(colors::text3())
        .child(text.to_string())
}

fn sub_heading(text: &str) -> Div {
    div()
        .mt(px(6.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors::text3())
        .child(text.to_uppercase())
}

fn keys(k: &str) -> AnyElement {
    let k = if cfg!(target_os = "macos") {
        k.replace("Ctrl", "⌘").replace("Shift", "⇧")
    } else {
        k.to_string()
    };
    let caps = k.split(['+', ' ']).map(|c| {
        div()
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(20.))
            .h(px(20.))
            .px(px(5.))
            .rounded(px(5.))
            .bg(colors::ink(0.06))
            .border_1()
            .border_b_2()
            .border_color(colors::ink(0.08))
            .text_size(px(10.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors::text2())
            .child(c.to_string())
    });
    div()
        .flex()
        .flex_none()
        .gap(px(3.))
        .children(caps)
        .into_any_element()
}

impl Root {
    pub(super) fn intro_welcome(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(h1("Run your coding agents side by side"))
            .child(lead(
                "HyprSpace is one window for the AI coding tools you already use. Each thread runs Claude Code, Codex or Gemini in a folder you choose, as a transcript you can read and steer or as a real terminal.",
            ))
            .child(sketches::app())
            .child(aside(
                "There's no new account. Agents use the logins your CLIs already have. HyprSpace starts the CLI, and your prompts go from the CLI straight to its provider. It all runs on this computer.",
            ))
            .into_any_element()
    }

    pub(super) fn intro_agents(&self, cx: &mut Context<Self>) -> AnyElement {
        let copied = self.intro.as_ref().and_then(|i| i.copied);
        let installed = self.agents.iter().filter(|a| a.status.installed).count();
        let rows = INSTALL.iter().map(|&(agent, cmd)| {
            let info = self.agents.iter().find(|a| a.agent == agent);
            let s = info.map(|a| &a.status);
            let (tone, state) = match s {
                None => (colors::text3(), ""),
                Some(s) if !s.installed => (colors::text3(), "Not installed"),
                Some(s) if s.account.is_some() || s.plan.is_some() => (colors::ok(), "Ready"),
                Some(_) => (colors::busy(), "Sign in"),
            };
            let detail: AnyElement = match s {
                None => small("Checking").into_any_element(),
                Some(s) if s.installed => {
                    let version = s.version.as_deref().map_or("Installed".to_string(), |v| {
                        format!("v{}", v.trim_start_matches('v'))
                    });
                    let who = match (&s.account, &s.plan) {
                        (Some(a), _) => crate::usage::masked(a),
                        (None, Some(p)) => p.clone(),
                        _ => "Run it once in a terminal to sign in".into(),
                    };
                    small(&format!("{version} · {who}")).into_any_element()
                }
                Some(_) => div()
                    .id(("intro-copy", agent as usize))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mt(px(2.))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colors::border1())
                    .bg(colors::bg())
                    .cursor_pointer()
                    .hover(|s| s.border_color(colors::border2()))
                    .font_family(hyprspace_theme::MONO)
                    .text_size(px(11.))
                    .text_color(colors::text2())
                    .child(cmd)
                    .child(icon(
                        if copied == Some(agent) {
                            "check"
                        } else {
                            "copy"
                        },
                        12.,
                        colors::text3(),
                    ))
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(cmd.to_string()));
                        if let Some(i) = &mut r.intro {
                            i.copied = Some(agent);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            };
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(14.))
                .py(px(11.))
                .rounded(px(10.))
                .border_1()
                .border_color(colors::border1())
                .bg(colors::surface1())
                .child(mark(agent, 20., colors::brand(agent).0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::text1())
                                .child(agent.name()),
                        )
                        .child(detail),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tone)
                        .child(state),
                )
        });
        let note = if self.agents.is_empty() {
            "Checking this machine".to_string()
        } else if installed > 0 {
            format!("{installed} of {} installed", INSTALL.len())
        } else {
            "None found. Install one, then check again.".to_string()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(h1("Check your agents"))
            .child(lead(
                "HyprSpace has no AI of its own. It starts these command-line tools for you, signed in as you. You need at least one installed and signed in.",
            ))
            .child(div().flex().flex_col().gap(px(8.)).children(rows))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(small(&note))
                    .child(
                        widgets::button("intro-check", "Check again")
                            .child(icon("rotate-cw", 13., colors::text2()))
                            .flex_row_reverse()
                            .on_click(cx.listener(|r, _: &ClickEvent, _, _| {
                                r.client.send(Command::LoadAgents)
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn intro_tour(&self, basics: bool, cx: &mut Context<Self>) -> AnyElement {
        let topics: &[Topic; 4] = if basics { &BASICS } else { &TOOLS };
        let open = self
            .intro
            .as_ref()
            .map_or(0, |i| if basics { i.basics } else { i.tools });
        let list = topics.iter().enumerate().map(|(i, t)| {
            let on = i == open;
            div()
                .id(("intro-topic", i))
                .relative()
                .flex()
                .flex_col()
                .gap(px(2.))
                .pl(px(14.))
                .pr(px(10.))
                .py(px(9.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.04)))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(10.))
                        .bottom(px(10.))
                        .w(px(2.))
                        .rounded(px(2.))
                        .when(on, |d| d.bg(colors::text1())),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if on { colors::text1() } else { colors::text3() })
                        .child(t.title),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(t.sub),
                )
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    if let Some(intro) = &mut r.intro {
                        if basics {
                            intro.basics = i;
                        } else {
                            intro.tools = i;
                        }
                    }
                    cx.notify();
                }))
        });
        let topic = &topics[open];
        let stage = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex_1()
                    .min_h(px(250.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(px(18.))
                    .rounded(px(12.))
                    .bg(colors::bg())
                    .child((topic.sketch)()),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .text_size(px(12.5))
                    .line_height(px(19.))
                    .text_color(colors::text2())
                    .child(div().mt(px(2.)).child(icon(
                        "mouse-pointer-click",
                        13.,
                        colors::text3(),
                    )))
                    .child(div().flex_1().min_w_0().child(topic.hint)),
            );
        let (title, text) = if basics {
            (
                "Folders, threads and panes",
                "Four ideas cover most of the app.",
            )
        } else {
            (
                "The tools around them",
                "Everything else is a shortcut away.",
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(h1(title))
            .child(lead(text))
            .child(
                div()
                    .flex()
                    .gap(px(22.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(170.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .children(list),
                    )
                    .child(stage),
            )
            .when(!basics, |d| {
                d.child(aside(
                    "Also in Settings: Usage shows what each agent has used, and Skills keeps reusable instructions for Claude.",
                ))
            })
            .into_any_element()
    }

    pub(super) fn intro_defaults(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let current = self.state.composer.permission;
        let perms = MODES.iter().map(|&(mode, name, glyph, says)| {
            let on = mode == current;
            let risky = mode == Permission::Bypass;
            let fg = if risky && on {
                colors::error()
            } else {
                colors::text1()
            };
            div()
                .id(("intro-perm", mode as usize))
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(12.))
                .rounded(px(10.))
                .border_1()
                .cursor_pointer()
                .map(|d| match (on, risky) {
                    (true, true) => d
                        .border_color(colors::error().opacity(0.4))
                        .bg(colors::error().opacity(0.1)),
                    (true, false) => d.border_color(colors::text3()).bg(colors::surface3()),
                    _ => d
                        .border_color(colors::border1())
                        .bg(colors::surface1())
                        .hover(|s| s.border_color(colors::border2())),
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .child(icon(glyph, 13., fg))
                        .child(name)
                        .when(mode == Permission::Auto, |d| {
                            d.child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors::text3())
                                    .child("Recommended"),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(colors::text3())
                        .child(says),
                )
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    r.state.composer.permission = mode;
                    r.save();
                    let prefs = r.state.composer.clone();
                    r.composer.update(cx, |c, cx| c.set_prefs(prefs, cx));
                    cx.notify();
                }))
        });
        let scheme = self.state.appearance.scheme;
        let schemes =
            widgets::segments().children(SCHEMES.iter().enumerate().map(|(i, &(s, name))| {
                widgets::segment(("intro-scheme", i), None, name, scheme == s, false).on_click(
                    cx.listener(move |r, _: &ClickEvent, window, cx| {
                        r.state.appearance.scheme = s;
                        r.apply_theme(window);
                        r.save();
                        cx.notify();
                    }),
                )
            }));
        let dark = colors::dark(scheme, window.appearance());
        let themes = THEMES.iter().enumerate().map(|(i, t)| {
            let on = t.id == self.state.appearance.theme;
            let id = t.id;
            let swatch = colors::hsla(hyprspace_theme::build(t.id, dark).accent);
            div()
                .id(("intro-theme", i))
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(32.))
                .px(px(10.))
                .rounded(px(8.))
                .border_1()
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(colors::text1())
                .map(|d| {
                    if on {
                        d.border_color(colors::text3()).bg(colors::surface3())
                    } else {
                        d.border_color(colors::border1())
                            .hover(|s| s.border_color(colors::border2()))
                    }
                })
                .child(div().size(px(12.)).rounded_full().bg(swatch))
                .child(t.name)
                .on_click(cx.listener(move |r, _: &ClickEvent, window, cx| {
                    r.state.appearance.theme = id.to_string();
                    r.apply_theme(window);
                    r.save();
                    cx.notify();
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(h1("Set your defaults"))
            .child(lead(
                "How much an agent may do without asking. Every new thread starts with this, and the composer can change it for one thread. You can change all of it later in Settings.",
            ))
            .child(div().grid().grid_cols(2).gap(px(8.)).children(perms))
            .child(sub_heading("Look"))
            .child(div().flex().child(schemes))
            .child(div().flex().flex_wrap().gap(px(8.)).children(themes))
            .into_any_element()
    }

    pub(super) fn intro_start(&self, cx: &mut Context<Self>) -> AnyElement {
        let start = div()
            .id("intro-folder")
            .flex()
            .items_center()
            .gap(px(14.))
            .max_w(px(480.))
            .px(px(16.))
            .py(px(14.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors::border1())
            .bg(colors::surface1())
            .cursor_pointer()
            .hover(|s| s.border_color(colors::border2()).bg(colors::surface3()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(40.))
                    .rounded(px(10.))
                    .bg(colors::accent().opacity(0.14))
                    .child(icon("folder-open", 20., colors::accent())),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors::text1())
                            .child("Choose a folder"),
                    )
                    .child(small("A project you want an agent to work on")),
            )
            .child(icon("arrow-right", 16., colors::text3()))
            .on_click(
                cx.listener(|r, _: &ClickEvent, window, cx| r.pick_thread_folder(window, cx)),
            );
        let shortcuts = SHORTCUTS.iter().map(|(k, what)| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(12.5))
                .text_color(colors::text2())
                .child(div().flex_none().w(px(110.)).flex().child(keys(k)))
                .child(div().flex_1().min_w_0().child(*what))
        });
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(h1("Pick a folder to start"))
            .child(lead(
                "It opens in the sidebar with a composer waiting. Type a task and press Enter.",
            ))
            .child(start)
            .child(sub_heading("Shortcuts worth learning"))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_y(px(10.))
                    .gap_x(px(24.))
                    .max_w(px(680.))
                    .children(shortcuts),
            )
            .into_any_element()
    }
}

fn small(text: &str) -> Div {
    div()
        .text_size(px(12.))
        .text_color(colors::text3())
        .child(text.to_string())
}
