// A terminal thread on the phone. The lines come from the desktop's emulator as styled runs;
// the phone sizes the terminal to its screen while it shows it, and typing goes straight to the
// terminal as you type, with a row of keys a phone keyboard lacks.

package com.hyprspace.android.ui.thread

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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
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
    // set by the strip's button, cleared once the fit goes out
    var fitNow by remember { mutableStateOf(false) }
    // whether this screen asked for a fit yet
    var asked by remember { mutableStateOf(false) }
    val typing = remember { FocusRequester() }

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
            // ask for the terminal at this size once the size settles (the keyboard slides in and out)
            // Once the computer takes it back, only the strip's button takes it again; the
            // keyboard sliding away mustn't snatch it from someone typing there.
            LaunchedEffect(cols, rows, live, autoFit, fitNow) {
                if (!live || cols < 20 || rows < 5) return@LaunchedEffect
                val first = !asked
                if (fitNow || (autoFit && (first || view.fit))) {
                    delay(250)
                    onFit(cols, rows)
                    asked = true
                    fitNow = false
                }
            }
            val list = rememberLazyListState()
            val atEnd by remember { derivedStateOf { !list.canScrollForward } }
            LaunchedEffect(view.lines.size, view.cursor) {
                if (view.lines.isNotEmpty() && (atEnd || list.firstVisibleItemIndex == 0)) {
                    list.scrollToItem(view.lines.size - 1)
                }
            }
            val full = maxWidth
            val wide = view.cols > cols
            val width = with(density) { (cell.width * maxOf(view.cols, cols)).toDp() } + 16.dp
            Box(Modifier.fillMaxSize().then(if (wide) Modifier.horizontalScroll(rememberScrollState()) else Modifier)) {
                LazyColumn(
                    Modifier.width(if (wide) width else full).fillMaxSize().clickableQuiet { typing.requestFocus() },
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
        Keys(ctrl, onCtrl = { ctrl = !ctrl }, onKeys = onKeys)
        TypeBox(typing, ctrl, onCtrlUsed = { ctrl = false }, onKeys = onKeys, onPaste = onPaste)
    }
}

@Composable
private fun Keys(ctrl: Boolean, onCtrl: () -> Unit, onKeys: (String) -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier
            .fillMaxWidth()
            .background(h.surface2)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 6.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Key("Esc") { onKeys("\u001b") }
        Key("Tab") { onKeys("\t") }
        Key("Ctrl", on = ctrl, onClick = onCtrl)
        Key("Ctrl C") { onKeys("\u0003") }
        Key(icon = R.drawable.ic_arrow_left) { onKeys("\u001b[D") }
        Key(icon = R.drawable.ic_arrow_up) { onKeys("\u001b[A") }
        Key(icon = R.drawable.ic_arrow_down) { onKeys("\u001b[B") }
        Key(icon = R.drawable.ic_arrow_right) { onKeys("\u001b[C") }
        Key("Enter") { onKeys("\r") }
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

/**
 * What the phone keyboard types goes to the terminal as it is typed: letters as they come, a
 * deleted letter as a backspace, Enter as Enter. A pasted block with line breaks goes in as one
 * paste. The box keeps only the current line, so autocorrect has something to work on.
 */
@Composable
private fun TypeBox(
    focus: FocusRequester,
    ctrl: Boolean,
    onCtrlUsed: () -> Unit,
    onKeys: (String) -> Unit,
    onPaste: (String) -> Unit,
) {
    val h = LocalHues.current
    var text by remember { mutableStateOf("") }
    Row(
        Modifier.fillMaxWidth().background(h.surface2).padding(start = 12.dp, end = 12.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .weight(1f)
                .clip(RoundedCornerShape(10.dp))
                .background(h.surface1)
                .border(1.dp, h.border1, RoundedCornerShape(10.dp))
                .padding(horizontal = 12.dp, vertical = 10.dp),
        ) {
            if (text.isEmpty()) Text("Type here", style = MaterialTheme.typography.bodyMedium, color = h.text3)
            BasicTextField(
                value = text,
                onValueChange = { next ->
                    if (ctrl && next.length == text.length + 1 && next.startsWith(text)) {
                        val c = next.last().lowercaseChar()
                        if (c in 'a'..'z') onKeys(((c - 'a') + 1).toChar().toString())
                        onCtrlUsed()
                        return@BasicTextField
                    }
                    val common = text.commonPrefixWith(next).length
                    val removed = text.length - common
                    val added = next.substring(common)
                    if (removed > 0) onKeys("\u007f".repeat(removed))
                    when {
                        added.contains('\n') -> {
                            onPaste(added.trimEnd('\n'))
                            if (added.endsWith('\n')) onKeys("\r")
                            text = ""
                            return@BasicTextField
                        }
                        added.isNotEmpty() -> onKeys(added)
                    }
                    text = next
                },
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium.copy(color = h.text1, fontFamily = TermFont),
                cursorBrush = SolidColor(h.accent),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    autoCorrectEnabled = false,
                    keyboardType = KeyboardType.Ascii,
                    imeAction = ImeAction.Send,
                ),
                keyboardActions = KeyboardActions(onSend = {
                    onKeys("\r")
                    text = ""
                }),
            )
        }
        Box(
            Modifier
                .padding(start = 8.dp)
                .size(40.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(h.accent)
                .clickableQuiet {
                    onKeys("\r")
                    text = ""
                },
            contentAlignment = Alignment.Center,
        ) {
            Icon(painterResource(R.drawable.ic_corner_down_left), "Enter", Modifier.size(16.dp), tint = h.onAccent)
        }
    }
}
