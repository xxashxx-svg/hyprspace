// How a tool call reads in one line, the desktop's words (crates/ui/src/transcript/tool.rs).

package com.hyprspace.android.model

import com.hyprspace.android.net.ChangeKind
import com.hyprspace.android.net.FileChange
import com.hyprspace.android.net.Tool

fun label(tool: Tool): String = when (tool) {
    is Tool.Command -> "Run ${firstLine(withoutCd(tool.command))}"
    is Tool.Read -> "Read ${short(tool.path)}"
    is Tool.Edit -> {
        val names = tool.changes.map { short(it.path) }
        val verb = when {
            tool.changes.size == 1 && tool.changes[0].kind == ChangeKind.Add -> "Create"
            tool.changes.size == 1 && tool.changes[0].kind == ChangeKind.Delete -> "Delete"
            else -> "Edit"
        }
        if (names.isEmpty()) "Edit files" else "$verb ${names.joinToString(", ")}"
    }
    is Tool.Search -> tool.path?.let { "Search ${short(it)} for ${tool.pattern}" } ?: "Search for ${tool.pattern}"
    is Tool.Web -> "Look up ${tool.target}"
    is Tool.Mcp -> "${tool.server}: ${tool.tool}"
    is Tool.AgentCall -> tool.description.ifEmpty { "Subagent" }
    is Tool.Other -> tool.name
}

/** What a call was given, shown when it is opened. Null when the label says it all. */
fun input(tool: Tool): String? = when (tool) {
    is Tool.Command -> tool.command
    is Tool.Read -> tool.path
    is Tool.Search -> tool.path?.let { "${tool.pattern}\nin $it" } ?: tool.pattern
    is Tool.Web -> tool.target
    is Tool.AgentCall -> tool.prompt.ifEmpty { null }
    is Tool.Mcp -> tool.input.takeIf { it.isNotEmpty() && it != "{}" }
    is Tool.Other -> tool.input.takeIf { it.isNotEmpty() && it != "{}" }
    is Tool.Edit -> null
}

/** Lines added and removed across an edit's diffs. */
fun counts(changes: List<FileChange>): Pair<Int, Int> {
    var add = 0
    var del = 0
    for (line in changes.flatMap { it.diff.lines() }) {
        if (line.startsWith("+++") || line.startsWith("---")) continue
        if (line.startsWith("+")) add++ else if (line.startsWith("-")) del++
    }
    return add to del
}

/** One line counting a run of calls by kind: "Ran 4 commands · read 1 file". */
fun summary(tools: List<Tool>): String {
    val n = IntArray(6)
    for (t in tools) when (t) {
        is Tool.Command -> n[0]++
        is Tool.Read -> n[1]++
        is Tool.Edit -> n[2] += maxOf(t.changes.size, 1)
        is Tool.Search -> n[3]++
        is Tool.Web -> n[4]++
        else -> n[5]++
    }
    fun p(k: Int, one: String, many: String) = if (k == 1) one else many
    val parts = listOf(
        n[0] to "ran ${n[0]} ${p(n[0], "command", "commands")}",
        n[1] to "read ${n[1]} ${p(n[1], "file", "files")}",
        n[2] to "edited ${n[2]} ${p(n[2], "file", "files")}",
        n[3] to "searched ${n[3]} ${p(n[3], "time", "times")}",
        n[4] to "looked up ${n[4]} ${p(n[4], "page", "pages")}",
        n[5] to "called ${n[5]} ${p(n[5], "tool", "tools")}",
    ).filter { it.first > 0 }.joinToString(" · ") { it.second }
    return parts.replaceFirstChar { it.uppercase() }
}

/** The file's name and the folder it sits in. */
fun short(path: String): String {
    val parts = path.split('/', '\\').filter { it.isNotEmpty() }
    return when (parts.size) {
        0 -> path
        1, 2 -> parts.joinToString("/")
        else -> parts.takeLast(2).joinToString("/")
    }
}

private fun firstLine(s: String): String {
    val line = s.lineSequence().firstOrNull().orEmpty()
    return if (line.length > 90) line.take(90) + "..." else line
}

/**
 * Agents often open a command by moving into the thread's own folder. That prefix pushes the
 * real command off the line, so the label drops it; the full command shows when it's opened.
 */
fun withoutCd(cmd: String): String {
    val rest = cmd.trimStart().removePrefix("cd ").takeIf { cmd.trimStart().startsWith("cd ") }?.trimStart()
        ?: return cmd
    val after = if (rest.startsWith('"')) {
        val q = rest.indexOf('"', 1)
        if (q < 0) return cmd else rest.substring(q + 1)
    } else {
        val i = rest.indexOfFirst { it.isWhitespace() || it == ';' || it == '&' }
        if (i < 0) return cmd else rest.substring(i)
    }.trimStart()
    for (sep in listOf("&&", ";")) {
        if (after.startsWith(sep)) {
            val next = after.removePrefix(sep).trimStart()
            if (next.isNotEmpty()) return next
        }
    }
    return cmd
}
