// Settings, Skills: Claude skills and slash commands, the user's own and the project's (the Tauri
// app's SkillsManager.tsx and skills.css). A list with edit and delete on each row, and a full
// editor for one SKILL.md. The engine reads and writes the files (engine/src/skills.rs).
//
// The Tauri app's snippets (text dragged into a terminal from its dock) are not here: the GPUI
// app has no snippet dock to drag them from.

use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Div, Entity, Focusable, FontWeight, IntoElement,
    Render, Subscription, Window, div, prelude::*, px,
};
use hyprspace_proto::agents::{SkillItem, SkillKind, SkillScope};
use hyprspace_proto::{Client, Command, SkillCommand, SkillEvent};
use hyprspace_theme::MONO;

use crate::assets::icon;
use crate::input::{InputEvent, TextInput};
use crate::root::{Root, Screen};
use crate::{colors, widgets};

const TEMPLATE: &str = "---
description: One line on what this skill does and when Claude should use it
---

# Skill

Write the instructions for Claude here.
";

/// What a name becomes on disk and after the slash: letters, digits, `-` and `_`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

struct Draft {
    /// The skill being edited, None for a new one.
    orig: Option<(SkillScope, String)>,
    kind: SkillKind,
    scope: SkillScope,
    name: Entity<TextInput>,
    body: Entity<TextInput>,
    /// The file's text has not arrived yet.
    loading: bool,
    _subs: Vec<Subscription>,
}

pub struct Skills {
    client: Client,
    cwd: Option<PathBuf>,
    items: Vec<SkillItem>,
    draft: Option<Draft>,
    busy: bool,
    error: Option<String>,
    /// The row whose delete was clicked once; the second click deletes.
    confirm: Option<String>,
}

impl Skills {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            cwd: None,
            items: Vec::new(),
            draft: None,
            busy: false,
            error: None,
            confirm: None,
        }
    }

    fn cwd(&self) -> PathBuf {
        self.cwd.clone().unwrap_or_default()
    }

    /// Lists the skills for `cwd` when the folder changed since the last look.
    pub fn show_for(&mut self, cwd: PathBuf) {
        if self.cwd.as_ref() != Some(&cwd) {
            self.cwd = Some(cwd);
            self.draft = None;
            self.refresh();
        }
    }

    fn refresh(&self) {
        self.client
            .send(Command::Skills(SkillCommand::List { cwd: self.cwd() }));
    }

    pub fn event(&mut self, e: SkillEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e {
            SkillEvent::List { cwd, items } => {
                if Some(&cwd) == self.cwd.as_ref() {
                    self.items = items;
                }
            }
            SkillEvent::Read {
                scope,
                name,
                content,
            } => {
                if let Some(d) = &mut self.draft
                    && d.orig.as_ref() == Some(&(scope, name))
                {
                    d.loading = false;
                    let text = content.unwrap_or_else(|_| TEMPLATE.to_string());
                    d.body.update(cx, |i, cx| i.set_text(text, cx));
                    let focus = d.body.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            SkillEvent::Done { error } => {
                self.busy = false;
                if error.is_none() {
                    self.draft = None;
                }
                self.error = error;
            }
        }
        cx.notify();
    }

    fn open(&mut self, item: Option<&SkillItem>, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| TextInput::new("my-skill", false, cx));
        let body = cx.new(|cx| TextInput::new("", true, cx));
        let subs = vec![
            cx.subscribe(&name, |_, _, _: &InputEvent, cx| cx.notify()),
            // Enter is a new line in a file, not a send
            cx.subscribe(&body, |_, input, e: &InputEvent, cx| {
                if let InputEvent::Submit = e {
                    input.update(cx, |i, cx| i.insert("\n", cx));
                }
            }),
        ];
        let (orig, kind, scope) = match item {
            Some(it) => {
                name.update(cx, |i, cx| i.set_text(it.name.clone(), cx));
                self.client.send(Command::Skills(SkillCommand::Read {
                    cwd: self.cwd(),
                    scope: it.scope,
                    kind: it.kind,
                    name: it.name.clone(),
                }));
                (Some((it.scope, it.name.clone())), it.kind, it.scope)
            }
            None => {
                body.update(cx, |i, cx| i.set_text(TEMPLATE, cx));
                (None, SkillKind::Skill, SkillScope::User)
            }
        };
        let focus = name.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        self.draft = Some(Draft {
            loading: orig.is_some(),
            orig,
            kind,
            scope,
            name,
            body,
            _subs: subs,
        });
        self.error = None;
        self.confirm = None;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.draft else {
            return;
        };
        let name = d.name.read(cx).text().trim().to_string();
        if name.is_empty() || self.busy || d.loading {
            return;
        }
        self.busy = true;
        self.client.send(Command::Skills(SkillCommand::Write {
            cwd: self.cwd(),
            scope: d.scope,
            kind: d.kind,
            name,
            content: d.body.read(cx).text().to_string(),
            replaces: d.orig.clone(),
        }));
        cx.notify();
    }

    fn delete(&mut self, item: &SkillItem, cx: &mut Context<Self>) {
        if self.confirm.as_deref() != Some(item.command.as_str()) {
            self.confirm = Some(item.command.clone());
            cx.notify();
            return;
        }
        self.confirm = None;
        self.client.send(Command::Skills(SkillCommand::Delete {
            cwd: self.cwd(),
            scope: item.scope,
            kind: item.kind,
            name: item.name.clone(),
        }));
        cx.notify();
    }
}

/// A quiet upper-case label over a section (settings.css `.set-label`).
/// A plain label over a group, in sentence case like the rest of Settings.
fn label(text: &str) -> Div {
    div()
        .text_size(px(13.))
        .text_color(colors::text3())
        .child(text.to_string())
}

fn small_button(id: impl Into<gpui::ElementId>, glyph: &str, danger: bool) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(px(6.))
        .cursor_pointer()
        .when(!danger, |d| d.hover(|s| s.bg(colors::surface3())))
        .when(danger, |d| d.hover(|s| s.bg(colors::error().opacity(0.16))))
        .child(icon(glyph, 14., colors::text3()))
}

impl Skills {
    fn list(&self, cx: &mut Context<Self>) -> AnyElement {
        let project = self
            .cwd
            .as_deref()
            .filter(|p| !p.as_os_str().is_empty())
            .map(crate::root::folder_name);
        let context: AnyElement = match project {
            Some(name) => div()
                .flex()
                .gap(px(4.))
                .child("Your user skills, plus project skills in")
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text2())
                        .child(format!("{name}.")),
                )
                .into_any_element(),
            None => "Your user skills. Open a project to see its skills too.".into_any_element(),
        };
        let rows = self.items.iter().enumerate().map(|(i, it)| {
            let edit = it.clone();
            let del = it.clone();
            let confirming = self.confirm.as_deref() == Some(it.command.as_str());
            let scope = match it.scope {
                SkillScope::User => "User",
                SkillScope::Project => "Project",
            };
            let delete: AnyElement = if confirming {
                div()
                    .id(("skill-del", i))
                    .flex()
                    .items_center()
                    .h(px(28.))
                    .px(px(9.))
                    .rounded(px(6.))
                    .bg(colors::error().opacity(0.14))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::error())
                    .cursor_pointer()
                    .child("Delete")
                    .on_click(cx.listener(move |s, _: &ClickEvent, _, cx| s.delete(&del, cx)))
                    .into_any_element()
            } else {
                small_button(("skill-del", i), "trash-2", true)
                    .on_click(cx.listener(move |s, _: &ClickEvent, _, cx| s.delete(&del, cx)))
                    .into_any_element()
            };
            div()
                .flex()
                .items_center()
                .gap(px(11.))
                .mx(px(16.))
                .py(px(11.))
                .when(i > 0, |d| d.border_t_1().border_color(colors::border1()))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(30.))
                        .rounded(px(6.))
                        .bg(colors::surface3())
                        .child(icon(
                            if it.kind == SkillKind::Command {
                                "square-slash"
                            } else {
                                "sparkles"
                            },
                            15.,
                            colors::text2(),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors::text1())
                                .child(it.command.clone())
                                .child(
                                    div()
                                        .px(px(7.))
                                        .rounded_full()
                                        .bg(colors::ink(0.07))
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(colors::text3())
                                        .child(scope),
                                ),
                        )
                        .child(
                            div()
                                .truncate()
                                .font_family(MONO)
                                .text_size(px(11.5))
                                .text_color(colors::text3())
                                .child(if it.description.is_empty() {
                                    "No description".to_string()
                                } else {
                                    it.description.clone()
                                }),
                        ),
                )
                .child(
                    small_button(("skill-edit", i), "pencil", false).on_click(cx.listener(
                        move |s, _: &ClickEvent, window, cx| s.open(Some(&edit), window, cx),
                    )),
                )
                .child(delete)
        });
        let add = widgets::button_frame("skill-add")
            .gap(px(6.))
            .child(icon("plus", 13., colors::text2()))
            .child("New skill")
            .on_click(cx.listener(|s, _: &ClickEvent, window, cx| s.open(None, window, cx)));
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .mb(px(10.))
                    .px(px(4.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(label("Claude skills"))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(colors::text3())
                                    .child(context),
                            ),
                    )
                    .child(add),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(12.))
                    .bg(colors::ink(0.035))
                    .when(self.items.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(16.))
                                .text_size(px(13.))
                                .text_color(colors::text3())
                                .child("None yet. Create one with New skill."),
                        )
                    })
                    .children(rows),
            )
            .children(self.error.clone().map(error))
            .into_any_element()
    }

    fn editor(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(d) = &self.draft else {
            return div().into_any_element();
        };
        // a box lights up while its input has focus, and a click anywhere in it focuses the input
        // (skills.css `.skm-editor:focus`)
        let frame = |id: &'static str, input: &Entity<TextInput>, cx: &mut Context<Self>| {
            let focus = input.read(cx).focus_handle(cx);
            let on = focus.is_focused(window);
            div()
                .id(id)
                .rounded(px(7.))
                .border_1()
                .border_color(if on {
                    colors::accent()
                } else {
                    colors::border1()
                })
                .bg(colors::bg())
                .cursor_text()
                .on_click(move |_, window, cx| window.focus(&focus, cx))
        };
        let name = d.name.read(cx).text().to_string();
        let has_project = self
            .cwd
            .as_deref()
            .is_some_and(|p| !p.as_os_str().is_empty());
        let scopes = widgets::segments().children(
            [(SkillScope::User, "User"), (SkillScope::Project, "Project")]
                .into_iter()
                .enumerate()
                .map(|(i, (scope, text))| {
                    let off = scope == SkillScope::Project && !has_project;
                    widgets::segment(("skill-scope", i), None, text, d.scope == scope, false)
                        .when(off, |s| s.opacity(0.4))
                        .on_click(cx.listener(move |s, _: &ClickEvent, _, cx| {
                            if let Some(d) = &mut s.draft
                                && !off
                            {
                                d.scope = scope;
                                cx.notify();
                            }
                        }))
                }),
        );
        let field = |row_name: &str, desc: String, control: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(px(24.))
                .py(px(13.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(colors::text1())
                                .child(row_name.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(colors::text3())
                                .child(desc),
                        ),
                )
                .child(div().flex_none().child(control))
        };
        let name_box = frame("skill-name", &d.name, cx)
            .w(px(240.))
            .h(px(32.))
            .flex()
            .items_center()
            .px(px(10.))
            .text_size(px(13.))
            .child(d.name.clone())
            .into_any_element();
        let slugged = slug(&name);
        let can_save = !name.trim().is_empty() && !self.busy && !d.loading;
        let save_label = if self.busy { "Saving" } else { "Save skill" };
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .id("skill-back")
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .text_size(px(13.))
                    .text_color(colors::text2())
                    .cursor_pointer()
                    .hover(|s| s.text_color(colors::text1()))
                    .child(icon("arrow-left", 15., colors::text2()))
                    .child("Back to skills")
                    .on_click(cx.listener(|s, _: &ClickEvent, _, cx| {
                        s.draft = None;
                        s.refresh();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(label(if d.orig.is_some() {
                        "Edit skill"
                    } else {
                        "New skill"
                    }))
                    .child(
                        div()
                            .px(px(16.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(colors::border1())
                            .bg(colors::surface2())
                            .child(field(
                                "Name",
                                format!(
                                    "Becomes /{}",
                                    if slugged.is_empty() { "name" } else { &slugged }
                                ),
                                name_box,
                            ))
                            .child(div().border_t_1().border_color(colors::border1()).child(
                                field(
                                    "Scope",
                                    "Where the SKILL.md is written".into(),
                                    scopes.into_any_element(),
                                ),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(label("SKILL.md"))
                    .child(
                        frame("skill-body", &d.body, cx)
                            .min_h(px(360.))
                            .px(px(14.))
                            .py(px(12.))
                            .font_family(MONO)
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .child(d.body.clone()),
                    ),
            )
            .children(self.error.clone().map(error))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        widgets::button("skill-cancel", "Cancel").on_click(cx.listener(
                            |s, _: &ClickEvent, _, cx| {
                                s.draft = None;
                                s.error = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        widgets::primary("skill-save", save_label)
                            .when(!can_save, |b| b.opacity(0.5))
                            .on_click(cx.listener(|s, _: &ClickEvent, _, cx| s.save(cx))),
                    ),
            )
            .into_any_element()
    }
}

fn error(text: String) -> AnyElement {
    div()
        .mt(px(8.))
        .text_size(px(12.))
        .text_color(colors::error())
        .child(text)
        .into_any_element()
}

impl Render for Skills {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.draft.is_some() {
            self.editor(window, cx)
        } else {
            self.list(cx)
        }
    }
}

impl Root {
    /// The folder whose project skills Settings shows: the thread or space it was opened from.
    fn skills_folder(&self) -> PathBuf {
        match self.back {
            Screen::Thread(id) => self.state.thread(id).map(|(_, t)| t.cwd().clone()),
            Screen::Compose(Some(space)) => self.state.space(space).and_then(|s| s.cwd.clone()),
            _ => None,
        }
        .unwrap_or_default()
    }

    pub(crate) fn skills_page(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let cwd = self.skills_folder();
        self.skills.update(cx, |s, _| s.show_for(cwd));
        self.skills.clone().into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_safe_commands() {
        assert_eq!(slug(" my skill "), "my-skill");
        assert_eq!(slug("fix/the bug!"), "fix-the-bug");
        assert_eq!(slug("--"), "");
    }
}
