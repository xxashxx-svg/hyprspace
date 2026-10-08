// A terminal thread on the phone. The lines come from the desktop's emulator as styled runs;
// the phone sizes the terminal to its screen while it shows it. Tapping the screen brings up the
// keyboard, which types straight into the terminal (`KeyInput`), with a row of keys a phone
// keyboard lacks.

package com.hyprspace.android.ui.thread

import android.content.ClipboardManager
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.model.TermView
import com.hyprspace.android.net.Span
import com.hyprspace.android.ui.Hues
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.TermFont
import com.hyprspace.android.ui.clickableQuiet
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.drop

private val FONT = 12.5.sp

/** The 256-color palette past the theme's sixteen: a 6x6x6 cube, then a gray ramp. */
private fun xterm(i: Int): Color {
    if (i < 232) {
        val n = i - 16
        val step = { v: Int -> if (v == 0) 0 else 55 + v * 40 }
        return Color(step(n / 36), step((n / 6) % 6), step(n % 6))
    }
    val g = 8 + (i - 232) * 10
    return Color(g, g, g)
}

private fun color(c: Long?, h: Hues, fallback: Color): Color = when {
    c == null -> fallback
    c and Span.RGB != 0L -> Color(((c shr 16) and 0xff).toInt(), ((c shr 8) and 0xff).toInt(), (c and 0xff).toInt())
    c < 16 -> h.ansi.getOrNull(c.toInt()) ?: fallback
    c < 256 -> xterm(c.toInt())
    else -> fallback
}

/** A line as styled text, with the cursor drawn at [cursor] when it's on this line. */
private fun line(spans: List<Span>, h: Hues, cursor: Int?): AnnotatedString = buildAnnotatedString {
    var col = 0
    for (s in spans) {
        var fg = color(s.fg, h, h.termFg)
        var bg = color(s.bg, h, Color.Transparent)
        if (s.s and Span.INVERSE != 0) {
            val f = fg
            fg = if (bg == Color.Transparent) h.termBg else bg
            bg = f
        }
        if (s.s and Span.DIM != 0) fg = fg.copy(alpha = fg.alpha * 0.6f)
        if (s.s and Span.HIDDEN != 0) fg = Color.Transparent
        val style = SpanStyle(
            color = fg,
            background = bg,
            fontWeight = if (s.s and Span.BOLD != 0) FontWeight.Bold else null,
            fontStyle = if (s.s and Span.ITALIC != 0) FontStyle.Italic else null,
            textDecoration = when {
                s.s and Span.UNDERLINE != 0 && s.s and Span.STRIKE != 0 ->
                    TextDecoration.combine(listOf(TextDecoration.Underline, TextDecoration.LineThrough))
                s.s and Span.UNDERLINE != 0 -> TextDecoration.Underline
                s.s and Span.STRIKE != 0 -> TextDecoration.LineThrough
                else -> null
            },
        )
        withStyle(style) { append(s.t) }
        col += s.t.length
    }
    if (cursor != null) {
        while (col <= cursor) {
            append(' ')
            col++
        }
        addStyle(SpanStyle(background = h.cursor, color = h.termBg), cursor, cursor + 1)
    }
}

@Composable
fun Terminal(
    view: TermView,
    live: Boolean,
    autoFit: Boolean,
    onFit: (Int, Int) -> Unit,
    onKeys: (String) -> Unit,
    onPaste: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val h = LocalHues.current
    val measure = rememberTextMeasurer()
    val style = TextStyle(fontFamily = TermFont, fontSize = FONT, lineHeight = FONT * 1.25f)
    val cell = remember(style) { measure.measure("M", style).size }
    val density = LocalDensity.current
    var ctrl by remember { mutableStateOf(false) }
    // whether the screen keeps its bottom line in view
    var follow by remember { mutableStateOf(true) }
    // set by the strip's button, cleared once the fit goes out
    var fitNow by remember { mutableStateOf(false) }
    // whether this screen asked for a fit yet
    var asked by remember { mutableStateOf(false) }
    // the keyboard's way into the terminal; tapping the screen brings the keyboard up
    var input by remember { mutableStateOf<KeyInput?>(null) }
    val clipboard = LocalContext.current.getSystemService(ClipboardManager::class.java)

    Column(modifier.background(h.termBg)) {
        if (view.ready && !view.fit && live) {
            Row(
                Modifier.fillMaxWidth().background(h.ink(0.05f)).padding(horizontal = 14.dp, vertical = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "Sized for your computer, ${view.cols} columns wide.",
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = h.text2,
                )
                Text(
                    "Fit to phone",
                    Modifier.clip(RoundedCornerShape(7.dp)).background(h.surface3).clickableQuiet { fitNow = true }.padding(horizontal = 10.dp, vertical = 5.dp),
                    style = MaterialTheme.typography.labelMedium,
                    color = h.text1,
                )
            }
        }
        BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
            val cols = with(density) { ((maxWidth - 16.dp).toPx() / cell.width).toInt() }
            val rows = with(density) { ((maxHeight - 8.dp).toPx() / cell.height).toInt() }
            // The terminal takes this screen's width (the computer keeps its height, which the
            // view scrolls through), asked for again only when the width changes. Once the
            // computer takes it back, only the strip's button takes it again.
            LaunchedEffect(cols, live, autoFit, fitNow) {
                if (!live || cols < 20) return@LaunchedEffect
                if (fitNow || (autoFit && (!asked || (view.fit && cols != view.cols)))) {
                    delay(250)
                    onFit(cols, rows)
                    asked = true
                    fitNow = false
                }
            }
            // The screen follows the cursor, with a few lines under it, where a prompt and an
            // agent's input box sit, unless the reader scrolled up to read. It opens there too.
            val list = rememberLazyListState(initialFirstVisibleItemIndex = (view.lines.size - 1).coerceAtLeast(0))
            LaunchedEffect(list) {
                // only a scroll that happened counts; the first look, before the jump to the
                // bottom, would read as "scrolled up" and stop the following
                snapshotFlow { list.isScrollInProgress }.drop(1).collect { moving ->
                    if (!moving) follow = !list.canScrollForward
                }
            }
            LaunchedEffect(view.lines.size, view.cursor, maxHeight, follow) {
                if (!follow || view.lines.isEmpty()) return@LaunchedEffect
                val last = view.lines.size - 1
                val shown = (rows - 1).coerceAtLeast(1)
                // the cursor's line a few lines from the bottom, or the last line when it hides
                val bottom = view.cursor?.first?.let { minOf(it + 3, last) } ?: last
                list.scrollToItem((bottom - shown + 1).coerceIn(0, last))
            }
            val full = maxWidth
            val wide = view.cols > cols
            val width = with(density) { (cell.width * maxOf(view.cols, cols)).toDp() } + 16.dp
            Box(Modifier.fillMaxSize().then(if (wide) Modifier.horizontalScroll(rememberScrollState()) else Modifier)) {
                LazyColumn(
                    Modifier.width(if (wide) width else full).fillMaxSize().clickableQuiet {
                        // typing happens at the prompt, so the screen goes back to it
                        follow = true
                        input?.show()
                    },
                    state = list,
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                ) {
                    itemsIndexed(view.lines) { i, spans ->
                        val at = view.cursor?.takeIf { it.first == i }?.second
                        Text(line(spans, h, at), style = style.copy(color = h.termFg), softWrap = false, maxLines = 1)
                    }
                }
            }
            if (!view.ready) {
                Text(
                    if (live) "Waiting for the screen." else "Starting the terminal on your computer.",
                    Modifier.align(Alignment.Center),
                    style = MaterialTheme.typography.bodyMedium,
                    color = h.text3,
                )
            }
        }
        AndroidView(
            factory = { KeyInput(it).also { v -> input = v } },
            update = { v ->
                v.onKeys = { follow = true; onKeys(it) }
                v.ctrl = { ctrl }
                v.ctrlUsed = { ctrl = false }
            },
            modifier = Modifier.size(1.dp),
        )
        Keys(
            ctrl,
            onCtrl = { ctrl = !ctrl },
            onKeys = onKeys,
            onKeyboard = {
                follow = true
                input?.show()
            },
            onPaste = {
                clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text?.toString()
                    ?.takeIf { it.isNotEmpty() }?.let(onPaste)
            },
        )
    }
}

@Composable
private fun Keys(
    ctrl: Boolean,
    onCtrl: () -> Unit,
    onKeys: (String) -> Unit,
    onKeyboard: () -> Unit,
    onPaste: () -> Unit,
) {
    val h = LocalHues.current
    Row(
        Modifier
            .fillMaxWidth()
            .background(h.surface2)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 6.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Key(icon = R.drawable.ic_keyboard, onClick = onKeyboard)
        Key("Esc") { onKeys("\u001b") }
        Key("Tab") { onKeys("\t") }
        Key("Ctrl", on = ctrl, onClick = onCtrl)
        Key("Ctrl C") { onKeys("\u0003") }
        Key(icon = R.drawable.ic_arrow_left) { onKeys("\u001b[D") }
        Key(icon = R.drawable.ic_arrow_up) { onKeys("\u001b[A") }
        Key(icon = R.drawable.ic_arrow_down) { onKeys("\u001b[B") }
        Key(icon = R.drawable.ic_arrow_right) { onKeys("\u001b[C") }
        Key("Enter") { onKeys("\r") }
        Key("Paste", onClick = onPaste)
        Key("Shift Tab") { onKeys("\u001b[Z") }
        Key("Ctrl D") { onKeys("\u0004") }
    }
}

@Composable
private fun Key(text: String? = null, icon: Int? = null, on: Boolean = false, onClick: () -> Unit) {
    val h = LocalHues.current
    Box(
        Modifier
            .height(34.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (on) h.accent else h.surface3)
            .border(1.dp, h.border1, RoundedCornerShape(8.dp))
            .clickableQuiet(onClick)
            .padding(horizontal = 11.dp),
        contentAlignment = Alignment.Center,
    ) {
        if (icon != null) {
            Icon(painterResource(icon), null, Modifier.size(15.dp), tint = if (on) h.onAccent else h.text1)
        } else {
            Text(text.orEmpty(), style = MaterialTheme.typography.labelMedium, color = if (on) h.onAccent else h.text1)
        }
    }
}
