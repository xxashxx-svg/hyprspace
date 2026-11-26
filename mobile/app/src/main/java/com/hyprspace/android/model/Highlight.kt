// Colors code by what each piece is, well enough for a reply's code block or a file on a phone:
// comments, strings, numbers, keywords, calls and type names. It reads characters, not grammar,
// so it never fails on half a block while a reply streams in. The desktop's tree-sitter stays
// there: its grammars are native code the app would have to carry for every language.

package com.hyprspace.android.model

enum class Tint { Keyword, Quoted, Number, Comment, Function, Type, Constant, Punctuation }

/** [start] until [end] of the code, drawn as [tint]. What no span covers is plain. */
data class Tinted(val start: Int, val end: Int, val tint: Tint)

private class Lang(
    val keywords: Set<String>,
    val line: List<String> = listOf("//"),
    val block: Pair<String, String>? = "/*" to "*/",
    val quotes: String = "\"'`",
    /** `'a` is a lifetime or a label here, so a quote only opens a one-character literal. */
    val ticks: Boolean = false,
    val triple: Boolean = false,
    val fold: Boolean = false,
)

private fun words(s: String) = s.split(' ').toSet()

private val CONSTANTS = words("true false null nil None True False undefined NaN Infinity self this super")

private val C = Lang(
    words(
        "abstract as async await break case catch class const continue default defer do else enum export extends " +
            "final finally fn for from func fun function go if implements import in instanceof interface internal is let " +
            "match mod mut namespace new object of operator override package private protected pub public return static " +
            "struct switch throw trait try type typealias typeof use val var void when where while yield impl crate dyn " +
            "ref move unsafe extern loop int long float double bool boolean char string byte short unsigned signed " +
            "sizeof template typename using virtual volatile inline goto select chan range map interface suspend " +
            "companion data sealed lateinit by init constructor get set readonly declare keyof satisfies delete guard " +
            "protocol extension throws inout",
    ),
)
private val RUST = Lang(C.keywords, ticks = true)
private val PYTHON = Lang(
    words(
        "and as assert async await break class continue def del elif else except finally for from global if import in " +
            "is lambda match case nonlocal not or pass raise return try while with yield",
    ),
    line = listOf("#"), block = null, quotes = "\"'", triple = true,
)
private val SHELL = Lang(
    words(
        "if then else elif fi for while until do done case esac in function return export local readonly set unset " +
            "echo cd exit source alias param begin end foreach switch try catch throw",
    ),
    line = listOf("#"), block = null,
)
private val RUBY = Lang(
    words(
        "alias and begin break case class def do else elsif end ensure for if in module next not or redo require " +
            "rescue retry return then unless until when while yield attr_accessor attr_reader private public",
    ),
    line = listOf("#"), block = null,
)
private val DATA = Lang(emptySet(), line = listOf("#"), block = null, quotes = "\"'")
private val LUA = Lang(
    words(
        "and break do else elseif end for function goto if in local not or repeat return then until while continue " +
            "export type typeof",
    ),
    line = listOf("--"), block = "--[[" to "]]", quotes = "\"'`",
)
private val SQL = Lang(
    words(
        "select from where insert into values update set delete create table alter drop index view join left right " +
            "inner outer on as and or not null is in like between group by order having limit offset union all distinct " +
            "primary key foreign references default constraint unique begin commit rollback with case when then else end " +
            "exists returning",
    ),
    line = listOf("--"), quotes = "'\"", fold = true,
)
private val MARKUP = Lang(emptySet(), line = emptyList(), block = "<!--" to "-->", quotes = "\"'")
private val STYLE = Lang(emptySet(), line = emptyList())

private fun lang(name: String): Lang? = when (name.lowercase().substringBefore(' ').substringAfterLast('.')) {
    "js", "jsx", "ts", "tsx", "javascript", "typescript", "mjs", "cjs", "java", "kotlin", "kt", "kts", "go", "golang",
    "c", "h", "cpp", "cc", "hpp", "c++", "cs", "csharp", "swift", "dart", "scala", "php", "zig", "groovy", "gradle",
    "json", "jsonc", "json5", "proto", "glsl", "wgsl", "objc",
    -> C
    "rust", "rs" -> RUST
    "python", "py", "pyi" -> PYTHON
    "sh", "bash", "zsh", "shell", "fish", "console", "powershell", "ps1", "pwsh", "makefile", "make", "dockerfile",
    "docker", "nix", "r", "perl", "pl",
    -> SHELL
    "ruby", "rb", "elixir", "ex", "exs" -> RUBY
    "yaml", "yml", "toml", "ini", "conf", "env", "properties" -> DATA
    "lua", "luau" -> LUA
    "sql", "psql", "sqlite", "mysql", "postgres" -> SQL
    "html", "xml", "svg", "vue", "svelte", "astro", "htm", "xaml", "plist" -> MARKUP
    "css", "scss", "less", "sass" -> STYLE
    else -> null
}

private const val MOST = 60_000

/** Whether [name] is a language this colors, by a code fence's label or a file's name. */
fun colored(name: String): Boolean = lang(name) != null

/** The spans of [code] to color, in order. Empty for a language it doesn't know, and for huge inputs. */
fun tint(code: String, name: String): List<Tinted> {
    val l = lang(name) ?: return emptyList()
    if (code.length > MOST) return emptyList()
    val out = ArrayList<Tinted>()
    val n = code.length
    var i = 0
    fun add(from: Int, to: Int, t: Tint) {
        if (to > from) out += Tinted(from, to, t)
    }
    while (i < n) {
        val c = code[i]
        val block = l.block
        when {
            block != null && code.startsWith(block.first, i) -> {
                val end = code.indexOf(block.second, i + block.first.length)
                val to = if (end < 0) n else end + block.second.length
                add(i, to, Tint.Comment)
                i = to
            }
            // a `#` inside a word is a color, a fragment or a shell `$#`, not a comment
            l.line.any { code.startsWith(it, i) && (it != "#" || i == 0 || code[i - 1].isWhitespace()) } -> {
                val end = code.indexOf('\n', i).let { if (it < 0) n else it }
                add(i, end, Tint.Comment)
                i = end
            }
            l.triple && (code.startsWith("\"\"\"", i) || code.startsWith("'''", i)) -> {
                val end = code.indexOf(code.substring(i, i + 3), i + 3)
                val to = if (end < 0) n else end + 3
                add(i, to, Tint.Quoted)
                i = to
            }
            c == '\'' && l.ticks -> {
                val to = when {
                    i + 2 < n && code[i + 1] != '\\' && code[i + 2] == '\'' -> i + 3
                    i + 3 < n && code[i + 1] == '\\' && code[i + 3] == '\'' -> i + 4
                    else -> i
                }
                if (to > i) add(i, to, Tint.Quoted)
                i = maxOf(to, i + 1)
            }
            c in l.quotes -> {
                var j = i + 1
                // a template literal runs over lines; the others end with theirs
                while (j < n && code[j] != c && (c == '`' || code[j] != '\n')) j += if (code[j] == '\\') 2 else 1
                val closed = j < n && code[j] == c
                // an apostrophe in a comment-less language's prose shouldn't swallow the line
                if (closed || c != '\'') add(i, minOf(j + 1, n), Tint.Quoted)
                i = if (closed || c != '\'') minOf(j + 1, n) else i + 1
            }
            c.isDigit() -> {
                var j = i + 1
                while (j < n && (code[j].isLetterOrDigit() || code[j] == '.' || code[j] == '_')) j++
                add(i, j, Tint.Number)
                i = j
            }
            c.isLetter() || c == '_' || c == '$' || c == '@' -> {
                var j = i + 1
                while (j < n && (code[j].isLetterOrDigit() || code[j] == '_')) j++
                val word = code.substring(i, j)
                val key = if (l.fold) word.lowercase() else word
                var k = j
                while (k < n && code[k] == ' ') k++
                val t = when {
                    key in l.keywords -> Tint.Keyword
                    word in CONSTANTS -> Tint.Constant
                    k < n && code[k] == '(' -> Tint.Function
                    word.length > 1 && word.all { it.isUpperCase() || it.isDigit() || it == '_' } -> Tint.Constant
                    word[0].isUpperCase() -> Tint.Type
                    else -> null
                }
                if (t != null) add(i, j, t)
                i = j
            }
            c in "{}()[];,.:" -> {
                add(i, i + 1, Tint.Punctuation)
                i++
            }
            else -> i++
        }
    }
    return out
}
