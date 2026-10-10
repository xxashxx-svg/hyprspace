use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, MouseButton, SharedString, div,
    prelude::*, px, relative,
};
use hyprspace_proto::Command;

use super::{FolderPicker, PickerEvent, keycap};
use crate::assets::icon;
use crate::colors;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Row {
    Source(usize),
    Project(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Sources,
    Browse,
    Create,
    Clone { github: bool },
}

struct Source {
    icon: &'static str,
    title: &'static str,
    about: &'static str,
    stage: Stage,
}

const SOURCES: [Source; 4] = [
    Source {
        icon: "folder-plus",
        title: "New project",
        about: "Start an empty git repository from a name",
        stage: Stage::Create,
    },
    Source {
        icon: "folder-open",
        title: "Local folder",
        about: "Browse a folder on this computer",
        stage: Stage::Browse,
    },
    Source {
        icon: "link",
        title: "Git URL",
        about: "Clone from a remote URL",
        stage: Stage::Clone { github: false },
    },
    Source {
        icon: "github",
        title: "GitHub repository",
        about: "Clone owner/repo from GitHub",
        stage: Stage::Clone { github: true },
    },
];

static NEXT: AtomicU64 = AtomicU64::new(1 << 48);

pub(super) fn remote(github: bool, text: &str) -> Option<(String, String)> {
    let t = text.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return None;
    }
    if github {
        let lower = t.to_ascii_lowercase();
        let start = ["https://github.com/", "http://github.com/", "github.com/"]
            .iter()
            .find(|p| lower.starts_with(*p))
            .map_or(0, |p| p.len());
        let path = t[start..].trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let (owner, name) = path.split_once('/')?;
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        return Some((
            format!("https://github.com/{owner}/{name}.git"),
            name.into(),
        ));
    }
    if let Some(r) = crate::composer::repo::parse(t) {
        return Some((r.url, r.name));
    }
    if !(t.contains("://") || t.starts_with("git@")) {
        return None;
    }
    let last = t.trim_end_matches('/').rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty()).then(|| (t.to_string(), name.to_string()))
}

pub(super) fn project_path(parent: &Path, text: &str) -> Result<PathBuf, &'static str> {
    let t = text.trim();
    if t.is_empty() {
        return Err("Type a name for the project.");
    }
    let p = Path::new(t);
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    if t.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("A name can't hold / \\ : * ? \" < > or |.");
    }
    Ok(parent.join(t))
}

impl FolderPicker {
    pub(super) fn sources_shown(&self, cx: &gpui::App) -> Vec<Row> {
        let q = self.query.read(cx).text().trim().to_lowercase();
        let sources = (0..SOURCES.len())
            .filter(|&i| {
                q.is_empty()
                    || SOURCES[i].title.to_lowercase().contains(&q)
                    || SOURCES[i].about.to_lowercase().contains(&q)
            })
            .map(Row::Source);
        let projects = (0..self.projects.len())
            .filter(|&i| self.projects[i].0.to_lowercase().contains(&q))
            .map(Row::Project);
        sources.chain(projects).collect()
    }

    pub(super) fn choose(&mut self, stage: Stage, cx: &mut Context<Self>) {
        if self.request.is_some() {
            return;
        }
        self.stage = stage;
        self.error = None;
        self.line = None;
        self.refocus = true;
        match stage {
            Stage::Sources => {
                self.pick = 0;
                self.query.update(cx, |i, cx| i.set_text("", cx));
            }
            Stage::Browse => {}
            Stage::Create | Stage::Clone { .. } => {
                let hint = match stage {
                    Stage::Create => "my-app",
                    Stage::Clone { github: true } => "owner/repo",
                    _ => "https://example.com/team/repo.git",
                };
                self.field.update(cx, |i, cx| {
                    i.set_text("", cx);
                    i.set_placeholder(hint, cx);
                });
            }
        }
        cx.notify();
    }

    pub(super) fn reveal_pick(&self, cx: &gpui::App) {
        let shown = self.sources_shown(cx);
        let sources = shown.iter().filter(|r| matches!(r, Row::Source(_))).count();
        let labels = (sources > 0) as usize + (self.pick >= sources) as usize;
        self.list_scroll.scroll_to_item(self.pick + labels);
    }

    pub(super) fn choose_picked(&mut self, cx: &mut Context<Self>) {
        match self.sources_shown(cx).get(self.pick) {
            Some(Row::Source(i)) => self.choose(SOURCES[*i].stage, cx),
            Some(Row::Project(i)) => cx.emit(PickerEvent::Open(self.projects[*i].1.clone())),
            None => {}
        }
    }

    pub(super) fn submit(&mut self, cx: &mut Context<Self>) {
        if self.request.is_some() {
            return;
        }
        let text = self.field.read(cx).text().to_string();
        let request = NEXT.fetch_add(1, Ordering::Relaxed);
        let command = match self.stage {
            Stage::Create => match project_path(&self.parent, &text) {
                Ok(path) => Command::NewProject { request, path },
                Err(e) => return self.fail(e, cx),
            },
            Stage::Clone { github } => match remote(github, &text) {
                Some((url, name)) => Command::Clone {
                    request,
                    url,
                    parent: self.parent.clone(),
                    name,
                    here: false,
                },
                None if github => return self.fail("Type it as owner/repo.", cx),
                None => return self.fail("That isn't a link to a git repository.", cx),
            },
            _ => return,
        };
        self.request = Some(request);
        self.error = None;
        self.client.send(command);
        cx.notify();
    }

    fn fail(&mut self, message: &str, cx: &mut Context<Self>) {
        self.error = Some(message.to_string());
        cx.notify();
    }

    pub fn clone_progress(&mut self, request: u64, line: String, cx: &mut Context<Self>) {
        if self.request == Some(request) {
            self.line = Some(line);
            cx.notify();
        }
    }

    pub fn cloned(
        &mut self,
        request: u64,
        result: Result<PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        if self.request != Some(request) {
            return;
        }
        self.request = None;
        self.line = None;
        match result {
            Ok(path) => cx.emit(PickerEvent::Open(path)),
            Err(e) => self.error = Some(e),
        }
        cx.notify();
    }

    pub(super) fn change_location(&mut self, cx: &mut Context<Self>) {
        if self.request.is_some() {
            return;
        }
        self.for_parent = Some(self.stage);
        self.stage = Stage::Browse;
        self.refocus = true;
        let parent = self.parent.clone();
        self.go(&parent, cx);
    }

    pub(super) fn render_sources(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.stage == Stage::Sources {
            self.render_list(cx)
        } else {
            self.render_form(cx)
        }
    }

    fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let accent = colors::accent();
        let label = |text: &'static str| {
            div()
                .px(px(10.))
                .pt(px(6.))
                .pb(px(4.))
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child(text)
        };
        let mut body = div()
            .id("sources-list")
            .track_scroll(&self.list_scroll)
            .max_h(px(440.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .py(px(6.))
            .px(px(8.));
        let shown = self.sources_shown(cx);
        if shown.iter().any(|r| matches!(r, Row::Source(_))) {
            body = body.child(label("Sources"));
        }
        if shown.is_empty() {
            body = body.child(
                div()
                    .py(px(18.))
                    .flex()
                    .justify_center()
                    .text_size(px(12.5))
                    .text_color(colors::text3())
                    .child("Nothing matches."),
            );
        }
        let mut projects = false;
        for (row, &r) in shown.iter().enumerate() {
            let i = match r {
                Row::Source(i) => i,
                Row::Project(i) => {
                    if !projects {
                        projects = true;
                        body = body.child(label("Projects"));
                    }
                    let (name, path) = &self.projects[i];
                    let on = row == self.pick;
                    let path = path.clone();
                    body = body.child(
                        div()
                            .id(("project", i))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .h(px(36.))
                            .px(px(10.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .when(on, |d| d.bg(accent.opacity(0.14)))
                            .on_mouse_move(cx.listener(move |p, _, _, cx| {
                                if p.pick != row {
                                    p.pick = row;
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                cx.emit(PickerEvent::Open(path.clone()))
                            }))
                            .child(crate::sidebar::tag(name, false))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(colors::text1())
                                    .child(name.clone()),
                            ),
                    );
                    continue;
                }
            };
            let s = &SOURCES[i];
            let on = row == self.pick;
            let stage = s.stage;
            body = body.child(
                div()
                    .id(("source", i))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .h(px(48.))
                    .px(px(10.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(accent.opacity(0.14)))
                    .on_mouse_move(cx.listener(move |p, _, _, cx| {
                        if p.pick != row {
                            p.pick = row;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |p, _: &ClickEvent, _, cx| p.choose(stage, cx)))
                    .child(icon(
                        s.icon,
                        16.,
                        if on { colors::text1() } else { colors::text2() },
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors::text1())
                                    .child(s.title),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.5))
                                    .text_color(colors::text3())
                                    .child(s.about),
                            ),
                    ),
            );
        }
        let head = div()
            .flex()
            .items_center()
            .gap(px(11.))
            .h(px(50.))
            .px(px(18.))
            .border_b_1()
            .border_color(colors::border1())
            .child(icon("search", 15., colors::text3()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(14.))
                    .child(self.query.clone()),
            );
        let foot = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(hint(&["\u{2191}", "\u{2193}"], "navigate"))
            .child(hint(&["\u{21b5}"], "select"))
            .child(hint(&["Esc"], "close"));
        panel(head, body, foot, cx)
    }

    fn render_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let accent = colors::accent();
        let busy = self.request.is_some();
        let (title, label, go, going) = match self.stage {
            Stage::Create => ("New project", "Name", "Create project", "Creating"),
            Stage::Clone { github: true } => {
                ("Clone from GitHub", "Repository", "Clone", "Cloning")
            }
            _ => ("Clone from a URL", "Repository URL", "Clone", "Cloning"),
        };
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(50.))
            .px(px(12.))
            .border_b_1()
            .border_color(colors::border1())
            .child(
                div()
                    .id("source-back")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(7.))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)))
                    .on_click(cx.listener(|p, _: &ClickEvent, _, cx| p.choose(Stage::Sources, cx)))
                    .child(icon("arrow-left", 15., colors::text2())),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::text1())
                    .child(SharedString::from(title)),
            );
        let caption = |text: &'static str| {
            div()
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child(text)
        };
        let boxed = || {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(36.))
                .px(px(11.))
                .rounded(px(8.))
                .bg(colors::ink(0.04))
                .border_1()
                .border_color(colors::border1())
        };
        let location = short(&self.parent, &self.home);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .px(px(18.))
            .pt(px(16.))
            .pb(px(18.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(caption(label))
                    .child(
                        boxed()
                            .border_color(if self.error.is_some() {
                                colors::error()
                            } else {
                                colors::border2()
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(13.5))
                                    .when(self.stage != Stage::Create, |d| {
                                        d.font_family(hyprspace_theme::MONO)
                                    })
                                    .child(self.field.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(caption("Location"))
                    .child(
                        boxed()
                            .child(icon("folder", 14., colors::text3()))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(12.5))
                                    .text_color(colors::text2())
                                    .child(location),
                            )
                            .child(
                                div()
                                    .id("source-where")
                                    .px(px(8.))
                                    .py(px(3.))
                                    .rounded(px(6.))
                                    .text_size(px(12.))
                                    .text_color(colors::text2())
                                    .when(!busy, |d| {
                                        d.cursor_pointer()
                                            .hover(|s| {
                                                s.bg(colors::ink(0.08)).text_color(colors::text1())
                                            })
                                            .on_click(cx.listener(|p, _: &ClickEvent, _, cx| {
                                                p.change_location(cx)
                                            }))
                                    })
                                    .child("Change"),
                            ),
                    ),
            )
            .when_some(self.error.clone(), |d, e| {
                d.child(
                    div()
                        .text_size(px(12.5))
                        .text_color(colors::error())
                        .child(e),
                )
            })
            .when(busy, |d| {
                d.child(
                    div()
                        .truncate()
                        .font_family(hyprspace_theme::MONO)
                        .text_size(px(11.5))
                        .text_color(colors::text3())
                        .child(self.line.clone().unwrap_or_else(|| going.to_string())),
                )
            });
        let foot = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_1()
            .child(hint(&["Esc"], "back"))
            .child(div().flex_1())
            .child(
                div()
                    .id("source-cancel")
                    .px(px(12.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
                    .child("Cancel")
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(PickerEvent::Close))),
            )
            .child(
                div()
                    .id("source-go")
                    .px(px(12.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .bg(accent)
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::on_accent())
                    .when(busy, |d| d.opacity(0.6))
                    .when(!busy, |d| {
                        d.cursor_pointer().hover(|s| s.bg(colors::accent_hover()))
                    })
                    .child(if busy { going } else { go })
                    .on_click(cx.listener(|p, _: &ClickEvent, _, cx| p.submit(cx))),
            );
        panel(head, body, foot, cx)
    }
}

fn short(path: &Path, home: &Path) -> String {
    let full = path.display().to_string();
    let p = match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
        }
        _ => full,
    };
    let parts: Vec<&str> = p.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    if p.chars().count() <= 48 || parts.len() <= 3 {
        return p;
    }
    let sep = std::path::MAIN_SEPARATOR;
    format!(
        "{}{sep}\u{2026}{sep}{}{sep}{}",
        parts[0],
        parts[parts.len() - 2],
        parts[parts.len() - 1]
    )
}

fn hint(keys: &[&str], what: &'static str) -> impl IntoElement {
    let mut d = div().flex().items_center().gap(px(4.));
    for k in keys {
        d = d.child(keycap(k));
    }
    d.child(what)
}

fn panel(
    head: impl IntoElement,
    body: impl IntoElement,
    foot: impl IntoElement,
    cx: &mut Context<FolderPicker>,
) -> AnyElement {
    div()
        .id("folders")
        .key_context("Folders")
        .on_action(cx.listener(FolderPicker::prev))
        .on_action(cx.listener(FolderPicker::next))
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(px(560.))
        .max_w(relative(0.92))
        .flex()
        .flex_col()
        .rounded(px(14.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .overflow_hidden()
        .child(head)
        .child(body)
        .child(
            div()
                .flex()
                .items_center()
                .px(px(14.))
                .py(px(9.))
                .border_t_1()
                .border_color(colors::border1())
                .text_size(px(11.))
                .text_color(colors::text3())
                .child(foot),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_takes_owner_and_repo_in_any_usual_form() {
        let want = Some((
            "https://github.com/ash/app.git".to_string(),
            "app".to_string(),
        ));
        assert_eq!(remote(true, "ash/app"), want);
        assert_eq!(remote(true, "https://github.com/ash/app"), want);
        assert_eq!(remote(true, "github.com/ash/app.git"), want);
        assert_eq!(remote(true, "ash"), None);
        assert_eq!(remote(true, "ash/app/tree"), None);
    }

    #[test]
    fn a_url_clones_any_git_remote() {
        assert_eq!(
            remote(false, "https://gitlab.com/team/tool").map(|r| r.1),
            Some("tool".into())
        );
        assert_eq!(
            remote(false, "git@example.com:team/tool.git").map(|r| r.1),
            Some("tool".into())
        );
        assert_eq!(remote(false, "not a url"), None);
        assert_eq!(remote(false, "tool"), None);
    }

    #[test]
    fn a_project_is_a_name_in_the_parent_or_a_whole_path() {
        let parent = Path::new("/work");
        assert_eq!(project_path(parent, "app"), Ok(PathBuf::from("/work/app")));
        assert!(project_path(parent, "").is_err());
        assert!(project_path(parent, "a/b").is_err());
        if !cfg!(windows) {
            assert_eq!(project_path(parent, "/x/app"), Ok(PathBuf::from("/x/app")));
        }
    }
}
