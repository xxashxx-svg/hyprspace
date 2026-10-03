// Opening a folder as a space, after T3 Code's Add project: an in-app browser before any system
// dialog. Type or edit a path and the folders in it list under the box, narrowed by what follows
// the last separator. Enter goes into the highlighted folder, Ctrl+Enter (Cmd on macOS) opens it,
// and the footer opens the folder on show or hands over to File Explorer or Finder. Listings come
// from the engine (`FolderCommand::ListDir`), like the dock's file tree.

use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, IntoElement, KeyBinding, MouseButton, Render, ScrollHandle, SharedString,
    Subscription, Window, actions, anchored, deferred, div, point, prelude::*, px, relative,
};
use hyprspace_proto::folder::DirEntry;
use hyprspace_proto::{Client, Command, FolderCommand};

use crate::assets::icon;
use crate::colors;
use crate::input::{InputEvent, TextInput};

actions!(folders, [Prev, Next, OpenPicked]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("up", Prev, Some("Folders > TextInput")),
        KeyBinding::new("down", Next, Some("Folders > TextInput")),
        KeyBinding::new("secondary-enter", OpenPicked, Some("Folders > TextInput")),
    ]);
}

pub enum PickerEvent {
    Open(PathBuf),
    /// Hand over to the system's folder dialog.
    System,
    Close,
}

/// What the system's file manager is called here.
pub fn file_manager() -> &'static str {
    if cfg!(target_os = "macos") {
        "Finder"
    } else if cfg!(windows) {
        "File Explorer"
    } else {
        "Files"
    }
}

/// What ends a folder's name in a typed path. A backslash is a plain character in a macOS name.
const SEPS: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };

/// The folder a typed path lists, and the start of a name being typed after its last separator.
/// `~` is the home folder.
pub fn split(text: &str, home: &Path) -> (PathBuf, String) {
    let t = text.trim();
    let t = match t.strip_prefix('~') {
        Some(rest) => format!("{}{}", home.display(), rest),
        None => t.to_string(),
    };
    if t.is_empty() {
        return (home.to_path_buf(), String::new());
    }
    match t.rfind(SEPS) {
        Some(i) => (PathBuf::from(&t[..=i]), t[i + 1..].to_string()),
        // "C:" alone is that drive's root
        None if cfg!(windows) && t.len() == 2 && t.ends_with(':') => {
            (PathBuf::from(format!("{t}\\")), String::new())
        }
        None => (home.to_path_buf(), t),
    }
}

/// The folders in a listing that match `partial`: names that start with it first, then ones that
/// hold it. Hidden and system folders show only when asked for by name.
pub fn matching(entries: &[DirEntry], partial: &str) -> Vec<String> {
    let p = partial.to_lowercase();
    let hidden = |n: &str| {
        (n.starts_with('.') && !p.starts_with('.'))
            || n.starts_with('$')
            || n == "System Volume Information"
            || n == "node_modules"
    };
    let names = entries
        .iter()
        .filter(|e| e.dir && !hidden(&e.name))
        .map(|e| e.name.clone());
    let (mut starts, mut holds): (Vec<String>, Vec<String>) = names
        .filter(|n| p.is_empty() || n.to_lowercase().contains(&p))
        .partition(|n| n.to_lowercase().starts_with(&p));
    starts.append(&mut holds);
    starts
}

/// A folder as the box shows it: with a separator at the end, ready for the next name.
fn with_sep(path: &Path) -> String {
    let s = path.display().to_string();
    if s.ends_with(SEPS) {
        s
    } else {
        format!("{s}{MAIN_SEPARATOR}")
    }
}

pub struct FolderPicker {
    client: Client,
    input: Entity<TextInput>,
    home: PathBuf,
    /// The folder listed, and what is in it once the engine answered.
    dir: PathBuf,
    entries: Option<Result<Vec<DirEntry>, String>>,
    /// Rows: going up when the folder has a parent, then the matches.
    shown: Vec<String>,
    up: Option<PathBuf>,
    sel: usize,
    scroll: ScrollHandle,
    pub(crate) back: Option<FocusHandle>,
    _sub: Subscription,
}

impl EventEmitter<PickerEvent> for FolderPicker {}

impl FolderPicker {
    pub fn new(
        client: Client,
        start: PathBuf,
        home: PathBuf,
        back: Option<FocusHandle>,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new("A folder's path", false, cx));
        let sub = cx.subscribe(&input, |p, _, e: &InputEvent, cx| match e {
            InputEvent::Changed => p.refresh(cx),
            InputEvent::Submit => p.enter(cx),
            InputEvent::Cancel => cx.emit(PickerEvent::Close),
            InputEvent::Images(_) => {}
        });
        let mut p = Self {
            client,
            input,
            home,
            dir: PathBuf::new(),
            entries: None,
            shown: Vec::new(),
            up: None,
            sel: 0,
            scroll: ScrollHandle::new(),
            back,
            _sub: sub,
        };
        p.go(&start, cx);
        p
    }

    fn text(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text().to_string()
    }

    /// Puts `dir` in the box, ready to type a name in it.
    fn go(&mut self, dir: &Path, cx: &mut Context<Self>) {
        let text = with_sep(dir);
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        self.refresh(cx);
    }

    /// After the box changed: list the folder it names if that is a new one, and narrow the rows.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let (dir, _) = split(&self.text(cx), &self.home);
        if dir != self.dir {
            self.dir = dir.clone();
            self.entries = None;
            self.up = dir.parent().map(Path::to_path_buf);
            self.client
                .send(Command::Folder(FolderCommand::ListDir { path: dir }));
        }
        self.narrow(cx);
    }

    fn narrow(&mut self, cx: &mut Context<Self>) {
        let (_, partial) = split(&self.text(cx), &self.home);
        self.shown = match &self.entries {
            Some(Ok(entries)) => matching(entries, &partial),
            _ => Vec::new(),
        };
        let rows = self.rows();
        // land on the first match rather than on going up, when there is one
        self.sel = if self.up.is_some() && !self.shown.is_empty() && !partial.is_empty() {
            1
        } else {
            0
        }
        .min(rows.saturating_sub(1));
        self.scroll.scroll_to_item(self.sel);
        cx.notify();
    }

    /// The engine's listing for a folder; one the box has moved on from is dropped.
    pub fn listed(
        &mut self,
        path: &Path,
        entries: &Result<Vec<DirEntry>, String>,
        cx: &mut Context<Self>,
    ) {
        if path == self.dir {
            self.entries = Some(entries.clone());
            self.narrow(cx);
        }
    }

    fn rows(&self) -> usize {
        self.up.is_some() as usize + self.shown.len()
    }

    /// The folder a row stands for.
    fn row_path(&self, ix: usize) -> Option<PathBuf> {
        match (&self.up, ix) {
            (Some(up), 0) => Some(up.clone()),
            (Some(_), i) => self.shown.get(i - 1).map(|n| self.dir.join(n)),
            (None, i) => self.shown.get(i).map(|n| self.dir.join(n)),
        }
    }

    /// Enter: into the highlighted folder, or open the typed one when nothing matches.
    fn enter(&mut self, cx: &mut Context<Self>) {
        match self.row_path(self.sel) {
            Some(path) => self.go(&path, cx),
            None => self.open_typed(cx),
        }
    }

    fn open_typed(&mut self, cx: &mut Context<Self>) {
        let text = self.text(cx);
        let (dir, partial) = split(&text, &self.home);
        let path = if partial.is_empty() {
            dir
        } else {
            dir.join(partial)
        };
        if path.is_dir() {
            cx.emit(PickerEvent::Open(path));
        }
    }

    fn open_picked(&mut self, _: &OpenPicked, _: &mut Window, cx: &mut Context<Self>) {
        // the highlighted folder, or the one listed when that is going up or nothing
        let up = self.up.is_some() && self.sel == 0;
        match self.row_path(self.sel).filter(|_| !up) {
            Some(path) => cx.emit(PickerEvent::Open(path)),
            None => cx.emit(PickerEvent::Open(self.dir.clone())),
        }
    }

    fn prev(&mut self, _: &Prev, _: &mut Window, cx: &mut Context<Self>) {
        self.sel = self.sel.saturating_sub(1);
        self.scroll.scroll_to_item(self.sel);
        cx.notify();
    }

    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        self.sel = (self.sel + 1).min(self.rows().saturating_sub(1));
        self.scroll.scroll_to_item(self.sel);
        cx.notify();
    }
}

impl Focusable for FolderPicker {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

fn keycap(label: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(18.))
        .h(px(18.))
        .px(px(5.))
        .rounded(px(5.))
        .bg(colors::ink(0.06))
        .border_1()
        .border_b_2()
        .border_color(colors::ink(0.08))
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors::text2())
        .child(SharedString::from(label.to_string()))
        .into_any_element()
}

impl Render for FolderPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = colors::accent();
        let mut list = div()
            .id("folders-list")
            .track_scroll(&self.scroll)
            .max_h(px(380.).min(window.viewport_size().height * 0.5))
            .overflow_y_scroll()
            .py(px(6.))
            .px(px(8.))
            .flex()
            .flex_col();
        let note = |text: String| {
            div()
                .py(px(22.))
                .px(px(16.))
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .text_color(colors::text3())
                .child(text)
        };
        for i in 0..self.rows() {
            let up = self.up.is_some() && i == 0;
            let (glyph, label) = if up {
                let name = self
                    .up
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        self.up
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    });
                ("arrow-left", format!("Up to {name}"))
            } else {
                ("folder", self.shown[i - self.up.is_some() as usize].clone())
            };
            let on = i == self.sel;
            let path = self.row_path(i);
            let open_path = path.clone();
            list = list.child(
                div()
                    .id(("folder-row", i))
                    .group("folder-row")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(34.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(accent.opacity(0.14)))
                    .on_mouse_move(cx.listener(move |p, _, _, cx| {
                        if p.sel != i {
                            p.sel = i;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |p, _: &ClickEvent, _, cx| {
                        if let Some(path) = &path {
                            p.go(path, cx);
                        }
                    }))
                    .child(icon(
                        glyph,
                        14.,
                        if on { colors::text1() } else { colors::text3() },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(if up { colors::text2() } else { colors::text1() })
                            .child(label),
                    )
                    .when(!up, |d| {
                        d.child(
                            div()
                                .id(("folder-open", i))
                                .px(px(8.))
                                .py(px(3.))
                                .rounded(px(6.))
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors::text1())
                                .bg(colors::ink(0.08))
                                .hover(|s| s.bg(colors::ink(0.14)))
                                .when(!on, |d| {
                                    d.opacity(0.).group_hover("folder-row", |s| s.opacity(1.))
                                })
                                .child("Open")
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(path) = &open_path {
                                        cx.emit(PickerEvent::Open(path.clone()));
                                    }
                                })),
                        )
                    }),
            );
        }
        match &self.entries {
            None => list = list.child(note("Reading the folder".into())),
            Some(Err(_)) => {
                list = list.child(note("This folder can't be read. Check the path.".into()))
            }
            Some(Ok(_)) if self.shown.is_empty() => {
                list = list.child(note("No folders here match.".into()));
            }
            _ => {}
        }
        let here = self
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.dir.display().to_string());
        let open_key = if cfg!(target_os = "macos") {
            "\u{2318}"
        } else {
            "Ctrl"
        };
        div()
            .id("folders")
            .key_context("Folders")
            .on_action(cx.listener(Self::prev))
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::open_picked))
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
                    .px(px(18.))
                    .pt(px(14.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors::text3())
                    .child("Open a folder"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(11.))
                    .h(px(46.))
                    .pl(px(18.))
                    .pr(px(14.))
                    .border_b_1()
                    .border_color(colors::border1())
                    .child(icon("folder-open", 16., accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(14.))
                            .font_family(hyprspace_theme::MONO)
                            .child(self.input.clone()),
                    ),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .px(px(14.))
                    .py(px(9.))
                    .border_t_1()
                    .border_color(colors::border1())
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(keycap("\u{21b5}"))
                            .child("go in"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(keycap(open_key))
                            .child(keycap("\u{21b5}"))
                            .child("open"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("folders-system")
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(7.))
                            .text_size(px(12.))
                            .text_color(colors::text2())
                            .cursor_pointer()
                            .hover(|s| s.bg(colors::ink(0.08)).text_color(colors::text1()))
                            .child(format!("Open in {}", file_manager()))
                            .on_click(
                                cx.listener(|_, _: &ClickEvent, _, cx| {
                                    cx.emit(PickerEvent::System)
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("folders-open-here")
                            .max_w(px(220.))
                            .truncate()
                            .px(px(12.))
                            .py(px(5.))
                            .rounded(px(7.))
                            .bg(accent)
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::on_accent())
                            .cursor_pointer()
                            .hover(|s| s.bg(colors::accent_hover()))
                            .child(format!("Open {here}"))
                            .on_click(cx.listener(|p, _: &ClickEvent, _, cx| {
                                cx.emit(PickerEvent::Open(p.dir.clone()))
                            })),
                    ),
            )
    }
}

impl crate::root::Root {
    /// The folder browser over the app, dimming it like the palette does.
    pub(crate) fn folder_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.folder_picker.clone()?;
        let size = window.viewport_size();
        let scrim = crate::colors::hsla(crate::colors::theme().shadow);
        Some(
            deferred(
                anchored().position(point(px(0.), px(0.))).child(
                    div()
                        .id("folders-scrim")
                        .w(size.width)
                        .h(size.height)
                        .occlude()
                        .bg(scrim.opacity(0.64))
                        .flex()
                        .items_start()
                        .justify_center()
                        .pt(size.height * 0.13)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|r, _, window, cx| r.close_folder_picker(window, cx)),
                        )
                        .child(crate::slide::rise_in(picker, "folders-in"))
                        .map(|scrim| {
                            crate::slide::ease_in(scrim, "folders-scrim-in", 140, |d, t| {
                                d.opacity(t)
                            })
                        }),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> DirEntry {
        DirEntry {
            name: name.into(),
            dir: true,
        }
    }

    #[test]
    fn a_path_splits_into_the_folder_and_the_name_being_typed() {
        let home = Path::new("/home/me");
        assert_eq!(
            split("/work/api/", home),
            (PathBuf::from("/work/api/"), String::new())
        );
        assert_eq!(
            split("/work/ap", home),
            (PathBuf::from("/work/"), "ap".into())
        );
        if cfg!(windows) {
            assert_eq!(
                split(r"C:\Main\Hyp", home),
                (PathBuf::from(r"C:\Main\"), "Hyp".into())
            );
            assert_eq!(split("C:", home), (PathBuf::from("C:\\"), String::new()));
        } else {
            // a backslash is part of the name on macOS
            assert_eq!(split(r"/a\b", home), (PathBuf::from("/"), r"a\b".into()));
        }
        assert_eq!(
            split("~/code", home),
            (PathBuf::from("/home/me/"), "code".into())
        );
        assert_eq!(split("", home), (PathBuf::from("/home/me"), String::new()));
    }

    #[test]
    fn names_that_start_with_the_text_come_first_and_hidden_ones_wait_to_be_asked() {
        let entries = [
            dir("api"),
            dir("hyprspace"),
            dir("myapp"),
            dir(".config"),
            dir("$RECYCLE.BIN"),
            DirEntry {
                name: "app.txt".into(),
                dir: false,
            },
        ];
        assert_eq!(matching(&entries, "ap"), ["api", "myapp"]);
        assert_eq!(matching(&entries, ""), ["api", "hyprspace", "myapp"]);
        assert_eq!(matching(&entries, ".c"), [".config"]);
    }
}
