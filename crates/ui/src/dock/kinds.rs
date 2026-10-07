// A file's icon and its color in the dock, by name, after T3 Code's and zeron's trees: a tinted
// glyph tells a TypeScript file from a stylesheet or an image at a glance. The tints are the
// theme's code colors, so every theme keeps them in its own key.

use gpui::Hsla;

use crate::colors::{self, hsla};

/// The glyph's color: one of the theme's code colors, or the quiet grey of a file that needs none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tint {
    Keyword,
    Function,
    Type,
    String,
    Constant,
    Number,
    Plain,
}

/// The Lucide icon and its tint for a file called `name`.
pub fn file_icon(name: &str) -> (&'static str, Hsla) {
    let (glyph, tint) = kind(name);
    let s = colors::theme().syntax;
    let color = match tint {
        Tint::Keyword => hsla(s.keyword),
        Tint::Function => hsla(s.function),
        Tint::Type => hsla(s.ty),
        Tint::String => hsla(s.string),
        Tint::Constant => hsla(s.constant),
        Tint::Number => hsla(s.number),
        Tint::Plain => colors::text3(),
    };
    (glyph, color)
}

fn kind(name: &str) -> (&'static str, Tint) {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lockb"
        | "bun.lock" => return ("file-lock", Tint::Plain),
        "dockerfile" | "makefile" | "justfile" | "license" => return ("file-cog", Tint::Plain),
        n if n.starts_with(".env") || n.starts_with(".git") || n.starts_with(".editorconfig") => {
            return ("file-cog", Tint::Plain);
        }
        _ => {}
    }
    let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
    match ext {
        "rs" | "js" | "mjs" | "cjs" | "html" | "htm" | "vue" | "svelte" | "astro" => {
            ("file-code", Tint::Number)
        }
        "ts" | "mts" | "cts" | "py" | "lua" | "luau" => ("file-code", Tint::Function),
        "tsx" | "jsx" | "go" => ("file-code", Tint::Type),
        "c" | "h" | "cc" | "cpp" | "hpp" | "cs" | "java" | "kt" | "swift" | "rb" | "php"
        | "dart" | "zig" | "css" | "scss" | "sass" | "less" => ("file-code", Tint::Keyword),
        "json" | "jsonc" | "json5" => ("file-braces", Tint::Number),
        "md" | "mdx" => ("file-text", Tint::String),
        "txt" | "rst" | "log" => ("file-text", Tint::Plain),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "avif" => {
            ("file-image", Tint::Constant)
        }
        "sh" | "bash" | "zsh" | "fish" | "ps1" | "psm1" | "bat" | "cmd" => {
            ("file-terminal", Tint::String)
        }
        "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "xml" | "plist" => {
            ("file-cog", Tint::Plain)
        }
        "lock" => ("file-lock", Tint::Plain),
        _ => ("file", Tint::Plain),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_pick_a_glyph_and_a_tint() {
        assert_eq!(kind("main.rs"), ("file-code", Tint::Number));
        assert_eq!(kind("App.TSX"), ("file-code", Tint::Type));
        assert_eq!(kind("package.json"), ("file-braces", Tint::Number));
        assert_eq!(kind("README.md"), ("file-text", Tint::String));
        assert_eq!(kind("logo.png"), ("file-image", Tint::Constant));
        assert_eq!(kind("deploy.ps1"), ("file-terminal", Tint::String));
        // a lock file is its own thing, whatever its extension says
        assert_eq!(kind("package-lock.json"), ("file-lock", Tint::Plain));
        assert_eq!(kind(".gitignore"), ("file-cog", Tint::Plain));
        assert_eq!(kind(".env.local"), ("file-cog", Tint::Plain));
        assert_eq!(kind("Makefile"), ("file-cog", Tint::Plain));
        assert_eq!(kind("notes"), ("file", Tint::Plain));
    }
}
