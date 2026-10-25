// "ultrathink" in a Claude message: the box gets the desktop's turning rainbow ring, and the word
// itself its colors (crates/ui/src/transcript/ring.rs).

package com.hyprspace.android.ui.thread

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.sin

private const val WORD = "ultrathink"

private fun Char.inWord() = isLetterOrDigit() || this == '_'

/** Where "ultrathink" stands as a whole word in [text], any case. */
fun ultraWords(text: String): List<IntRange> {
    val lower = text.lowercase()
    val out = ArrayList<IntRange>()
    var at = lower.indexOf(WORD)
    while (at >= 0) {
        val end = at + WORD.length
        if ((at == 0 || !lower[at - 1].inWord()) && (end == lower.length || !lower[end].inWord())) out += at until end
        at = lower.indexOf(WORD, end)
    }
    return out
}

fun saysUltrathink(text: String) = ultraWords(text).isNotEmpty()

/** The theme's rainbow at [hue] degrees: OKLCH, lighter on dark ground (hyprspace_theme::rainbow). */
fun rainbow(hue: Float, dark: Boolean): Color {
    val (l, c) = if (dark) 0.76 to 0.15 else 0.62 to 0.18
    val h = Math.toRadians(hue.toDouble())
    val a = c * cos(h)
    val b = c * sin(h)
    val l1 = (l + 0.3963377774 * a + 0.2158037573 * b).pow(3)
    val m1 = (l - 0.1055613458 * a - 0.0638541728 * b).pow(3)
    val s1 = (l - 0.0894841775 * a - 1.2914855480 * b).pow(3)
    fun gamma(x: Double): Float {
        val v = if (x <= 0.0031308) 12.92 * x else 1.055 * x.pow(1 / 2.4) - 0.055
        return v.coerceIn(0.0, 1.0).toFloat()
    }
    return Color(
        gamma(4.0767416621 * l1 - 3.3077115913 * m1 + 0.2309699292 * s1),
        gamma(-1.2684380046 * l1 + 2.6097574011 * m1 - 0.3413193965 * s1),
        gamma(-0.0041960863 * l1 - 0.7034186147 * m1 + 1.7076147010 * s1),
    )
}

private fun hues(turn: Float, dark: Boolean): List<Color> =
    (0..12).map { rainbow(((it / 12f - turn) * 360f).mod(360f), dark) }

/** The turning rainbow ring around a box with [corner] corners, and a soft glow under it. */
fun Modifier.ultraRing(on: Boolean, dark: Boolean, corner: Dp): Modifier = if (!on) this else composed {
    val turn by rememberInfiniteTransition(label = "ultra").animateFloat(
        0f, 1f, infiniteRepeatable(tween(3000, easing = LinearEasing), RepeatMode.Restart), label = "ultra",
    )
    drawWithContent {
        drawContent()
        val brush = Brush.sweepGradient(hues(turn, dark), center = Offset(size.width / 2, size.height / 2))
        val r = corner.toPx()
        // the glow: wider and fainter strokes under the ring, since a blur needs Android 12
        for ((width, alpha) in listOf(10.dp to 0.06f, 6.dp to 0.1f, 3.5.dp to 0.18f)) {
            val w = width.toPx()
            drawRoundRect(
                brush,
                topLeft = Offset(-w / 2 + 1.dp.toPx(), -w / 2 + 1.dp.toPx()),
                size = Size(size.width + w - 2.dp.toPx(), size.height + w - 2.dp.toPx()),
                cornerRadius = CornerRadius(r + w / 2),
                alpha = alpha,
                style = Stroke(w),
            )
        }
        val w = 1.5.dp.toPx()
        drawRoundRect(
            brush,
            topLeft = Offset(w / 2, w / 2),
            size = Size(size.width - w, size.height - w),
            cornerRadius = CornerRadius(r - w / 2),
            style = Stroke(w),
        )
    }
}

/** Draws each whole-word "ultrathink" in the rainbow, leaving the text itself as typed. */
data class UltraWords(private val dark: Boolean) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val words = ultraWords(text.text)
        if (words.isEmpty()) return TransformedText(text, OffsetMapping.Identity)
        // a word's width or so per sweep, so each one shows the whole rainbow
        val brush = Brush.linearGradient(
            (0..6).map { rainbow(it * 50f, dark) },
            start = Offset.Zero,
            end = Offset(220f, 0f),
            tileMode = TileMode.Mirror,
        )
        val out = buildAnnotatedString {
            append(text)
            for (w in words) addStyle(SpanStyle(brush = brush), w.first, w.last + 1)
        }
        return TransformedText(out, OffsetMapping.Identity)
    }
}
