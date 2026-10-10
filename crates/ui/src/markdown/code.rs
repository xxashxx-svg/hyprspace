// A fenced code block, colored by the file viewer's tree-sitter highlighter in the theme's code
// colors. Highlighting runs on the background executor, so a long block never holds up a frame:
// the block shows plain until its colors arrive. While a reply streams, the text only grows, so
// the colors of the last pass stay valid for the part already there and the block doesn't
// flicker back to plain on every chunk.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    App, Context, IntoElement, RenderOnce, SharedString, StyledText, Task, Window, div, prelude::*,
    px,
};
use hyprspace_syntax::{Kind, Line};
use hyprspace_theme::MONO;

use crate::colors;

#[derive(IntoElement)]
pub struct CodeBlock {
    key: SharedString,
    lang: String,
    text: String,
}

impl CodeBlock {
    pub fn new(key: &str, lang: &str, text: &str) -> Self {
        Self {
            key: format!("code-{key}").into(),
            lang: lang.to_string(),
            text: text.trim_end().to_string(),
        }
    }
}

/// A made-up file name the highlighter knows the fence's language by, or None for a language it
/// has no grammar for.
fn path_for(lang: &str) -> Option<PathBuf> {
    // `rust,ignore` and `js {title="x"}` name their language first
    let lang = lang
        .split([',', '{'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let ext = match lang.as_str() {
        "" => return None,
        "rust" => "rs",
        "typescript" => "ts",
        "javascript" | "node" => "js",
        "python" | "python3" => "py",
        "shell" | "bash" | "zsh" | "console" | "shellscript" => "sh",
        "golang" => "go",
        "markdown" => "md",
        "csharp" | "c#" => "cs",
        "c++" => "cpp",
        other => other,
    };
    let path = PathBuf::from(format!("code.{ext}"));
    hyprspace_syntax::language(&path).map(|_| path)
}

thread_local! {
    static DONE: RefCell<HashMap<(PathBuf, String), Vec<Line>>> = RefCell::default();
}

fn remember(path: PathBuf, text: String, lines: Vec<Line>) {
    DONE.with(|d| {
        let mut d = d.borrow_mut();
        if d.len() > 300 {
            d.clear();
        }
        d.insert((path, text), lines);
    });
}

/// The colored ranges of one block, kept across frames for as long as the block is drawn.
#[derive(Default)]
struct Colors {
    /// The text `lines` were made from.
    text: String,
    lines: Vec<Line>,
    /// The text the block shows now, which may be ahead of `text` while a pass runs.
    wanted: String,
    job: Option<Task<()>>,
    last: Option<Instant>,
}

impl Colors {
    /// Starts a pass for `text` unless one is running; a running pass starts the next when it
    /// lands, so a streaming block gets one pass at a time rather than one per chunk.
    fn want(&mut self, path: PathBuf, text: &str, cx: &mut Context<Self>) {
        self.wanted = text.to_string();
        if self.job.is_some() || self.text == text {
            return;
        }
        // a block streaming in changes every frame, and each pass that lands draws one more
        let wait = self.last.map_or(Duration::ZERO, |l| {
            Duration::from_millis(100).saturating_sub(l.elapsed())
        });
        self.job = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let Ok(source) = this.read_with(cx, |c, _| c.wanted.clone()) else {
                return;
            };
            let lines = cx
                .background_executor()
                .spawn({
                    let (path, source) = (path.clone(), source.clone());
                    async move { hyprspace_syntax::highlight(&path, &source).unwrap_or_default() }
                })
                .await;
            let _ = this.update(cx, |c, cx| {
                if c.wanted == source {
                    remember(path.clone(), source.clone(), lines.clone());
                }
                c.text = source;
                c.lines = lines;
                c.job = None;
                c.last = Some(Instant::now());
                if c.wanted != c.text {
                    let wanted = c.wanted.clone();
                    c.want(path, &wanted, cx);
                }
                cx.notify();
            });
        }));
    }

    /// Ranges over `text` with their kinds, from the last pass that fits it.
    fn spans(&self, text: &str) -> Vec<(Range<usize>, Kind)> {
        // a grown block keeps its old colors: every old line is a prefix of the new one
        if self.lines.is_empty() || !text.starts_with(&self.text) {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut start = 0;
        for (line, spans) in self.text.split('\n').zip(&self.lines) {
            out.extend(
                spans
                    .iter()
                    .map(|(r, k)| (start + r.start..start + r.end, *k)),
            );
            start += line.len() + 1;
        }
        out
    }
}

impl RenderOnce for CodeBlock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = self.key.to_string();
        let path = path_for(&self.lang);
        let highlights = match path {
            Some(path) => {
                let colors = window.use_keyed_state(self.key, cx, |_, _| {
                    let key = (path.clone(), self.text.clone());
                    let lines = DONE.with(|d| d.borrow().get(&key).cloned());
                    match lines {
                        Some(lines) => Colors {
                            text: key.1,
                            lines,
                            ..Colors::default()
                        },
                        None => Colors::default(),
                    }
                });
                colors.update(cx, |c, cx| c.want(path, &self.text, cx));
                colors
                    .read(cx)
                    .spans(&self.text)
                    .into_iter()
                    .map(|(r, k)| (r, colors::syntax(k)))
                    .collect()
            }
            None => Vec::new(),
        };
        let lang = self.lang;
        let copy = crate::widgets::CopyButton::new(format!("{key}-copy"), self.text.clone());
        div()
            .flex()
            .flex_col()
            .rounded(px(10.))
            .bg(colors::ink(0.035))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl(px(14.))
                    .pr(px(6.))
                    .pt(px(5.))
                    .text_size(px(11.))
                    .text_color(colors::text3())
                    .child(lang)
                    .child(copy),
            )
            .child(
                div()
                    .px(px(14.))
                    .pt(px(2.))
                    .pb(px(10.))
                    .font_family(MONO)
                    .text_size(px(12.))
                    .line_height(px(19.))
                    .text_color(colors::hsla(colors::theme().syntax.plain))
                    .child({
                        let text = SharedString::from(self.text);
                        let styled = StyledText::new(text.clone()).with_highlights(highlights);
                        let layout = styled.layout().clone();
                        super::select::wrap(&key, text, layout, styled.into_any_element())
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fence_names_find_their_grammar() {
        for lang in [
            "rust",
            "rs",
            "rust,ignore",
            "TypeScript",
            "js",
            "python",
            "json",
            "toml",
            "bash",
            "shell",
            "lua",
        ] {
            assert!(path_for(lang).is_some(), "{lang}");
        }
        assert!(path_for("").is_none());
        assert!(path_for("text").is_none());
        assert!(path_for("brainfuck").is_none());
    }

    #[test]
    fn a_grown_block_keeps_its_old_colors() {
        let text = "let a = 1;\nlet b";
        let c = Colors {
            text: text.into(),
            lines: hyprspace_syntax::highlight(&path_for("rust").unwrap(), text).unwrap(),
            ..Default::default()
        };
        let grown = "let a = 1;\nlet b = 2;";
        let spans = c.spans(grown);
        assert!(spans.contains(&(0..3, Kind::Keyword)), "{spans:?}");
        // the second line's `let` sits after the first line and its newline
        assert!(spans.contains(&(11..14, Kind::Keyword)), "{spans:?}");
        assert!(c.spans("fn other() {}").is_empty());
    }
}
