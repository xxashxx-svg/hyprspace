// The phone's copy of a terminal thread's lines, scrollback included, kept current by the
// desktop's frames (engine/src/phone/mirror.rs): drop lines off the top, cut or pad to the new
// length, then put each changed line in place.

package com.hyprspace.android.model

import com.hyprspace.android.net.Span
import com.hyprspace.android.net.TermFrame

data class TermView(
    val lines: List<List<Span>> = emptyList(),
    val cols: Int = 0,
    val rows: Int = 0,
    /** [line, column] of the cursor, when it shows. */
    val cursor: Pair<Int, Int>? = null,
    /** Sized for this phone right now. */
    val fit: Boolean = false,
    /** The program there takes pastes marked as pastes. */
    val paste: Boolean = false,
    /** Has had a frame yet. */
    val ready: Boolean = false,
)

class TermBuffer {
    private val lines = ArrayList<List<Span>>()
    private var view = TermView()

    fun apply(f: TermFrame): TermView {
        if (f.reset) lines.clear()
        lines.subList(0, minOf(f.drop, lines.size)).clear()
        if (lines.size > f.len) lines.subList(f.len, lines.size).clear()
        while (lines.size < f.len) lines += emptyList<Span>()
        for ((i, spans) in f.changed()) if (i in lines.indices) lines[i] = spans
        view = TermView(
            lines = lines.toList(),
            cols = f.cols,
            rows = f.rows,
            cursor = f.cursor?.takeIf { it.size == 2 }?.let { it[0] to it[1] },
            fit = f.fit,
            paste = f.paste,
            ready = true,
        )
        return view
    }

    fun clear(): TermView {
        lines.clear()
        view = TermView()
        return view
    }
}
