//! Syntax highlighting for the file viewer. A file's path picks a tree-sitter grammar, and the
//! highlighter's captures come back as byte ranges per line, each with a [`Kind`] the UI colors.
//!
//! Adapted from zeron's `crates/syntax` (MIT, see THIRD_PARTY_NOTICES.md): the grammar set, the
//! capture table, the query fixes for Rust and Markdown, and the precedence between captures.

use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Files past this size are shown plain: highlighting them would take longer than reading them.
pub const MAX_SOURCE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    JavaScript,
    Jsx,
    TypeScript,
    Tsx,
    Python,
    Go,
    Json,
    Bash,
    Toml,
    Markdown,
    Html,
    Css,
    Yaml,
    C,
    Cpp,
    CSharp,
    Java,
    Lua,
}

/// What a piece of code is, which is all the viewer needs to pick its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Comment,
    Keyword,
    String,
    Escape,
    Number,
    Constant,
    Type,
    Function,
    Macro,
    Property,
    Variable,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Heading,
    Link,
    Code,
    Emphasis,
}

impl Kind {
    /// Which capture wins where several cover the same text: the more specific one.
    const fn precedence(self) -> u8 {
        match self {
            Self::Escape => 95,
            Self::Macro => 90,
            Self::Property | Self::Attribute => 85,
            Self::Function | Self::Type | Self::Constant | Self::Tag => 70,
            Self::Comment | Self::Keyword | Self::String | Self::Number => 60,
            Self::Variable | Self::Operator => 50,
            Self::Punctuation => 40,
            Self::Heading | Self::Emphasis => 30,
            Self::Link => 33,
            Self::Code => 34,
        }
    }
}

/// One line's colored ranges, byte offsets into that line, sorted and not overlapping.
pub type Line = Vec<(Range<usize>, Kind)>;

/// The grammar for a file, by its name or extension.
pub fn language(path: &Path) -> Option<Lang> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if matches!(name.as_str(), "cargo.lock" | ".gitconfig") {
        return Some(Lang::Toml);
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => Lang::Rust,
        "js" | "mjs" | "cjs" => Lang::JavaScript,
        "jsx" => Lang::Jsx,
        "ts" | "mts" | "cts" => Lang::TypeScript,
        "tsx" => Lang::Tsx,
        "py" | "pyi" => Lang::Python,
        "go" => Lang::Go,
        "json" | "jsonc" | "json5" => Lang::Json,
        "sh" | "bash" | "zsh" => Lang::Bash,
        "toml" => Lang::Toml,
        "md" | "markdown" => Lang::Markdown,
        "html" | "htm" => Lang::Html,
        "css" => Lang::Css,
        "yaml" | "yml" => Lang::Yaml,
        "c" | "h" => Lang::C,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Lang::Cpp,
        "cs" => Lang::CSharp,
        "java" => Lang::Java,
        // Luau is Lua with types; the Lua grammar reads most of it
        "lua" | "luau" => Lang::Lua,
        _ => return None,
    })
}

/// Highlights `source` as the file at `path`, one [`Line`] per line of `source`. None when the
/// language is unknown, the file is too big, or the parser gave up; the viewer then shows plain
/// text.
pub fn highlight(path: &Path, source: &str) -> Option<Vec<Line>> {
    if source.len() > MAX_SOURCE {
        return None;
    }
    let lang = language(path)?;
    let primary = config(lang)?;
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(primary, source.as_bytes(), None, |name| {
            if name == "markdown_inline" {
                return markdown_inline();
            }
            injected(lang, name).and_then(config)
        })
        .ok()?;
    let mut stack: Vec<Kind> = Vec::new();
    let mut spans = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(h) => stack.push(KINDS[h.0]),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(kind) = stack.iter().copied().max_by_key(|k| k.precedence()) {
                    spans.push((start..end, kind));
                }
            }
        }
    }
    Some(split_lines(source, spans))
}

/// Cuts absolute spans at line ends. Spans come in order and never overlap (the highlighter
/// reports each stretch of source once), so one pass does it.
fn split_lines(source: &str, spans: Vec<(Range<usize>, Kind)>) -> Vec<Line> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let mut lines: Vec<Line> = vec![Vec::new(); starts.len()];
    for (range, kind) in spans {
        let mut line = starts.partition_point(|&s| s <= range.start) - 1;
        let mut at = range.start;
        while at < range.end && line < starts.len() {
            let start = starts[line];
            // the line's text, without its \n or \r\n
            let raw_end = starts.get(line + 1).map_or(source.len(), |s| s - 1);
            let end = if source[start..raw_end].ends_with('\r') {
                raw_end - 1
            } else {
                raw_end
            };
            let to = range.end.min(end);
            if at < to {
                let (a, b) = (at - start, to - start);
                // `format` then `!` are two captures of one macro; draw them as one piece
                match lines[line].last_mut() {
                    Some((r, k)) if *k == kind && r.end == a => r.end = b,
                    _ => lines[line].push((a..b, kind)),
                }
            }
            at = starts.get(line + 1).copied().unwrap_or(range.end);
            line += 1;
        }
    }
    lines
}

/// The languages a parent may pull in for embedded code (a fenced block, a script tag).
fn injected(parent: Lang, name: &str) -> Option<Lang> {
    let lang = match name.to_ascii_lowercase().as_str() {
        "rust" | "rs" => Lang::Rust,
        "javascript" | "js" => Lang::JavaScript,
        "jsx" => Lang::Jsx,
        "typescript" | "ts" => Lang::TypeScript,
        "tsx" => Lang::Tsx,
        "python" | "py" => Lang::Python,
        "go" => Lang::Go,
        "json" => Lang::Json,
        "bash" | "sh" | "shell" | "zsh" | "console" => Lang::Bash,
        "toml" => Lang::Toml,
        "html" => Lang::Html,
        "css" => Lang::Css,
        "yaml" | "yml" => Lang::Yaml,
        "c" => Lang::C,
        "cpp" | "c++" => Lang::Cpp,
        "csharp" | "cs" => Lang::CSharp,
        "java" => Lang::Java,
        "lua" | "luau" => Lang::Lua,
        _ => return None,
    };
    match parent {
        Lang::Markdown => Some(lang),
        Lang::Html => matches!(lang, Lang::JavaScript | Lang::Css).then_some(lang),
        _ => None,
    }
}

fn make(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
    locals: &str,
) -> Option<HighlightConfiguration> {
    let mut config =
        HighlightConfiguration::new(language, name, highlights, injections, locals).ok()?;
    config.configure(CAPTURES);
    Some(config)
}

/// Compiled queries hold no document state, so each grammar compiles once and is shared.
fn config(lang: Lang) -> Option<&'static HighlightConfiguration> {
    static CONFIGS: [OnceLock<Option<HighlightConfiguration>>; 19] =
        [const { OnceLock::new() }; 19];
    CONFIGS[lang as usize].get_or_init(|| build(lang)).as_ref()
}

fn markdown_inline() -> Option<&'static HighlightConfiguration> {
    static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            make(
                tree_sitter_md::INLINE_LANGUAGE.into(),
                "markdown_inline",
                tree_sitter_md::HIGHLIGHT_QUERY_INLINE,
                tree_sitter_md::INJECTION_QUERY_INLINE,
                "",
            )
        })
        .as_ref()
}

fn js_family(lang: Lang) -> Option<HighlightConfiguration> {
    use tree_sitter_javascript as js;
    use tree_sitter_typescript as ts;
    let (grammar, name, queries, injections, locals): (tree_sitter::Language, _, &[&str], _, _) =
        match lang {
            Lang::JavaScript => (
                js::LANGUAGE.into(),
                "javascript",
                &[js::HIGHLIGHT_QUERY],
                js::INJECTIONS_QUERY,
                js::LOCALS_QUERY,
            ),
            Lang::Jsx => (
                js::LANGUAGE.into(),
                "jsx",
                &[js::HIGHLIGHT_QUERY, js::JSX_HIGHLIGHT_QUERY],
                js::INJECTIONS_QUERY,
                js::LOCALS_QUERY,
            ),
            Lang::TypeScript => (
                ts::LANGUAGE_TYPESCRIPT.into(),
                "typescript",
                &[js::HIGHLIGHT_QUERY, ts::HIGHLIGHTS_QUERY],
                "",
                ts::LOCALS_QUERY,
            ),
            _ => (
                ts::LANGUAGE_TSX.into(),
                "tsx",
                &[
                    js::HIGHLIGHT_QUERY,
                    js::JSX_HIGHLIGHT_QUERY,
                    ts::HIGHLIGHTS_QUERY,
                ],
                "",
                ts::LOCALS_QUERY,
            ),
        };
    make(grammar, name, &queries.join("\n"), injections, locals)
}

fn build(lang: Lang) -> Option<HighlightConfiguration> {
    match lang {
        Lang::Rust => {
            // the upstream query lumps numbers and booleans in with builtin constants
            let q = tree_sitter_rust::HIGHLIGHTS_QUERY
                .replace(
                    "(integer_literal) @constant.builtin",
                    "(integer_literal) @number",
                )
                .replace(
                    "(float_literal) @constant.builtin",
                    "(float_literal) @number",
                );
            make(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                &q,
                tree_sitter_rust::INJECTIONS_QUERY,
                "",
            )
        }
        Lang::JavaScript | Lang::Jsx | Lang::TypeScript | Lang::Tsx => js_family(lang),
        Lang::Python => make(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Go => make(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Json => make(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Bash => make(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Toml => make(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Markdown => {
            // the block grammar's `inline` node owns children covering its text, which the
            // highlighter leaves out of an injection unless told to keep them
            let injections = tree_sitter_md::INJECTION_QUERY_BLOCK.replace(
                "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\"))",
                "((inline) @injection.content\n  (#set! injection.language \"markdown_inline\")\n  (#set! injection.include-children))",
            );
            make(
                tree_sitter_md::LANGUAGE.into(),
                "markdown",
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                &injections,
                "",
            )
        }
        Lang::Html => make(
            tree_sitter_html::LANGUAGE.into(),
            "html",
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
        ),
        Lang::Css => make(
            tree_sitter_css::LANGUAGE.into(),
            "css",
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Yaml => make(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::C => make(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Cpp => make(
            tree_sitter_cpp::LANGUAGE.into(),
            "cpp",
            &format!(
                "{}\n{}",
                tree_sitter_c::HIGHLIGHT_QUERY,
                tree_sitter_cpp::HIGHLIGHT_QUERY
            ),
            "",
            "",
        ),
        Lang::CSharp => make(
            tree_sitter_c_sharp::LANGUAGE.into(),
            "csharp",
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Java => make(
            tree_sitter_java::LANGUAGE.into(),
            "java",
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Lua => make(
            tree_sitter_lua::LANGUAGE.into(),
            "lua",
            tree_sitter_lua::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_lua::LOCALS_QUERY,
        ),
    }
}

// Generic to specific. `configure` resolves a dotted capture (`function.method.call`) to the
// longest name here that prefixes it.
const CAPTURES: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "string.special",
    "string.escape",
    "escape",
    "number",
    "boolean",
    "constant",
    "constant.builtin",
    "type",
    "type.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "property",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "label",
    "text.title",
    "text.literal",
    "text.uri",
    "text.reference",
    "text.emphasis",
    "text.strong",
];

const KINDS: &[Kind] = &[
    Kind::Comment,
    Kind::Keyword,
    Kind::String,
    Kind::String,
    Kind::Escape,
    Kind::Escape,
    Kind::Number,
    Kind::Constant,
    Kind::Constant,
    Kind::Constant,
    Kind::Type,
    Kind::Type,
    Kind::Type,
    Kind::Function,
    Kind::Function,
    Kind::Macro,
    Kind::Property,
    Kind::Variable,
    Kind::Keyword,
    Kind::Variable,
    Kind::Operator,
    Kind::Punctuation,
    Kind::Tag,
    Kind::Attribute,
    Kind::Constant,
    Kind::Heading,
    Kind::Code,
    Kind::Link,
    Kind::Link,
    Kind::Emphasis,
    Kind::Emphasis,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(path: &str, source: &str) -> Vec<(String, Kind)> {
        let lines = highlight(Path::new(path), source).unwrap();
        assert_eq!(lines.len(), source.split('\n').count());
        source
            .split('\n')
            .zip(lines)
            .flat_map(|(text, spans)| {
                spans
                    .into_iter()
                    .map(move |(r, k)| (text[r].to_string(), k))
            })
            .collect()
    }

    #[test]
    fn the_tables_line_up() {
        assert_eq!(CAPTURES.len(), KINDS.len());
    }

    #[test]
    fn picks_grammars_by_name() {
        assert_eq!(language(Path::new("src/main.rs")), Some(Lang::Rust));
        assert_eq!(language(Path::new("App.TSX")), Some(Lang::Tsx));
        assert_eq!(language(Path::new("Cargo.lock")), Some(Lang::Toml));
        assert_eq!(language(Path::new("init.luau")), Some(Lang::Lua));
        assert_eq!(language(Path::new("README")), None);
        assert_eq!(language(Path::new("shot.png")), None);
    }

    #[test]
    fn rust_gets_its_categories() {
        let got = pieces(
            "a.rs",
            "// hi\npub fn build(n: usize) -> Widget {\n    format!(\"x{n}\");\n    42\n}",
        );
        for want in [
            ("// hi", Kind::Comment),
            ("pub", Kind::Keyword),
            ("build", Kind::Function),
            ("Widget", Kind::Type),
            ("format!", Kind::Macro),
            ("42", Kind::Number),
        ] {
            assert!(
                got.contains(&(want.0.to_string(), want.1)),
                "{want:?} not in {got:?}"
            );
        }
    }

    #[test]
    fn every_grammar_loads() {
        for (path, src) in [
            ("a.js", "const x = 1;"),
            ("a.jsx", "const x = <div a=\"b\" />;"),
            ("a.ts", "let x: number = 1;"),
            ("a.tsx", "let x = <A />;"),
            ("a.py", "def f():\n    return 'x'"),
            ("a.go", "package main\nfunc main() {}"),
            ("a.json", "{\"a\": 1}"),
            ("a.sh", "echo \"$HOME\""),
            ("a.toml", "[a]\nb = 1"),
            ("a.md", "# Title\n\n```rust\nfn a() {}\n```\n*em*"),
            ("a.html", "<p class=\"x\">hi</p><script>let a = 1</script>"),
            ("a.css", ".a { color: red; }"),
            ("a.yaml", "a: 1"),
            ("a.c", "int main() { return 0; }"),
            ("a.cpp", "class A {};"),
            ("a.cs", "class A { }"),
            ("a.java", "class A { }"),
            ("a.lua", "local x = \"hi\""),
        ] {
            let got = pieces(path, src);
            assert!(!got.is_empty(), "{path} drew nothing");
        }
    }

    #[test]
    fn markdown_reaches_into_fences() {
        let got = pieces("a.md", "# Title\n\n```rust\nfn a() {}\n```");
        assert!(got.iter().any(|(_, k)| *k == Kind::Heading), "{got:?}");
        assert!(got.contains(&("fn".to_string(), Kind::Keyword)), "{got:?}");
    }

    #[test]
    fn ranges_are_per_line_and_skip_crlf() {
        let src = "/* a\r\nb */\r\nlet x = 1;";
        let lines = highlight(Path::new("a.js"), src).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], [(0..4, Kind::Comment)]);
        assert_eq!(lines[1], [(0..4, Kind::Comment)]);
        for line in &lines {
            assert!(line.windows(2).all(|w| w[0].0.end <= w[1].0.start));
        }
    }

    #[test]
    fn big_or_unknown_files_stay_plain() {
        assert!(highlight(Path::new("a.txt"), "x").is_none());
        let big = "a".repeat(MAX_SOURCE + 1);
        assert!(highlight(Path::new("a.rs"), &big).is_none());
    }
}
