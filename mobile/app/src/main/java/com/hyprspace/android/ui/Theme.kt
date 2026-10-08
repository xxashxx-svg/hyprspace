// The desktop's look on the phone: the theme the computer uses, light or dark as the phone is
// set (unless the desktop pins one side), with the desktop's fonts. Before the first connect it
// uses the default theme, the same colors crates/theme builds.

package com.hyprspace.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.net.BoardTheme
import com.hyprspace.android.net.Palette
import com.hyprspace.android.net.Scheme

/** 0xRRGGBBAA, as the desktop sends colors. */
fun rgba(c: Long): Color = Color(
    red = ((c shr 24) and 0xff).toInt(),
    green = ((c shr 16) and 0xff).toInt(),
    blue = ((c shr 8) and 0xff).toInt(),
    alpha = (c and 0xff).toInt(),
)

@Immutable
data class Hues(
    val dark: Boolean,
    val bg: Color,
    val surface1: Color,
    val surface2: Color,
    val surface3: Color,
    val accent: Color,
    val onAccent: Color,
    val link: Color,
    val text1: Color,
    val text2: Color,
    val text3: Color,
    val border0: Color,
    val border1: Color,
    val border2: Color,
    val ink: Color,
    val busy: Color,
    val waiting: Color,
    val ok: Color,
    val error: Color,
    val diffAdd: Color,
    val diffDel: Color,
    val termBg: Color,
    val termFg: Color,
    val cursor: Color,
    val ansi: List<Color>,
) {
    /** The theme's ink at an alpha: lines and washes, white on dark and black on light. */
    fun ink(a: Float) = ink.copy(alpha = a)
}

fun hues(p: Palette, dark: Boolean) = Hues(
    dark = dark,
    bg = rgba(p.bg), surface1 = rgba(p.surface1), surface2 = rgba(p.surface2), surface3 = rgba(p.surface3),
    accent = rgba(p.accent), onAccent = rgba(p.onAccent), link = rgba(p.link),
    text1 = rgba(p.text1), text2 = rgba(p.text2), text3 = rgba(p.text3),
    border0 = rgba(p.border0), border1 = rgba(p.border1), border2 = rgba(p.border2), ink = rgba(p.ink),
    busy = rgba(p.busy), waiting = rgba(p.waiting), ok = rgba(p.ok), error = rgba(p.error),
    diffAdd = rgba(p.diffAdd), diffDel = rgba(p.diffDel),
    termBg = rgba(p.termBg), termFg = rgba(p.termFg), cursor = rgba(p.cursor),
    ansi = p.ansi.map(::rgba),
)

/** The default theme, from crates/theme's build("t3", ...). */
val DefaultTheme = BoardTheme(
    scheme = Scheme.System,
    light = Palette(
        bg = 0xF7F7F7FF, surface1 = 0xF7F7F7FF, surface2 = 0xFDFDFDFF, surface3 = 0xEBEBEBFF,
        accent = 0x1745C2FF, onAccent = 0xFFFFFFFF, link = 0x1745C2FF,
        text1 = 0x1B1B1BFF, text2 = 0x585858FF, text3 = 0x808080FF,
        border0 = 0x0000000D, border1 = 0x00000014, border2 = 0x00000021, ink = 0x000000FF,
        busy = 0xF59E0BFF, waiting = 0x3B82F6FF, ok = 0x10B981FF, error = 0xEF4444FF,
        diffAdd = 0x059669FF, diffDel = 0xDC2626FF,
        termBg = 0xF7F7F7FF, termFg = 0x222222FF, cursor = 0x1745C2FF,
        ansi = listOf(
            0x282C34FF, 0xC4283CFF, 0x248444FF, 0xAA7400FF, 0x1E64D6FF, 0x923CBAFF, 0x00869AFF, 0x78808CFF,
            0x6E7682FF, 0xD83C50FF, 0x2E9854FF, 0xBA840AFF, 0x3278ECFF, 0xA650CEFF, 0x0A9AB0FF, 0x1E2228FF,
        ),
    ),
    dark = Palette(
        bg = 0x161616FF, surface1 = 0x161616FF, surface2 = 0x1F1F1FFF, surface3 = 0x2B2B2BFF,
        accent = 0x1B4ED8FF, onAccent = 0xFFFFFFFF, link = 0x77A2FCFF,
        text1 = 0xF5F5F5FF, text2 = 0xA4A4A4FF, text3 = 0x747474FF,
        border0 = 0xFFFFFF09, border1 = 0xFFFFFF0F, border2 = 0xFFFFFF1A, ink = 0xFFFFFFFF,
        busy = 0xF59E0BFF, waiting = 0x3B82F6FF, ok = 0x10B981FF, error = 0xEF4444FF,
        diffAdd = 0x10B981FF, diffDel = 0xEF4444FF,
        termBg = 0x161616FF, termFg = 0xEEEEEEFF, cursor = 0xA3C4FFFF,
        ansi = listOf(
            0x181E26FF, 0xFF7A8EFF, 0x86E795FF, 0xF4CD72FF, 0x89BEFFFF, 0xD0B0FFFF, 0x7CE8EDFF, 0xD2DAE6FF,
            0x6E7888FF, 0xFFA8B4FF, 0xB0F5BAFF, 0xFFE095FF, 0xAED2FFFF, 0xE5CBFFFF, 0xA7F4F7FF, 0xF4F7FCFF,
        ),
    ),
)

val Sans = FontFamily(
    Font(R.font.geist_regular, FontWeight.Normal),
    Font(R.font.geist_medium, FontWeight.Medium),
    Font(R.font.geist_semibold, FontWeight.SemiBold),
    Font(R.font.geist_bold, FontWeight.Bold),
)

val Mono = FontFamily(
    Font(R.font.geist_mono, FontWeight.Normal),
    Font(R.font.geist_mono_medium, FontWeight.Medium),
)

/** The terminal's font: the Nerd Font build, so agent status lines draw their glyphs. */
val TermFont = FontFamily(
    Font(R.font.term_regular, FontWeight.Normal),
    Font(R.font.term_bold, FontWeight.Bold),
)

val LocalHues = staticCompositionLocalOf { hues(DefaultTheme.dark, true) }

@Composable
fun HyprTheme(theme: BoardTheme?, content: @Composable () -> Unit) {
    val t = theme?.takeIf { it.dark.bg != 0L } ?: DefaultTheme
    val dark = when (t.scheme) {
        Scheme.Dark -> true
        Scheme.Light -> false
        Scheme.System -> isSystemInDarkTheme()
    }
    val h = hues(if (dark) t.dark else t.light, dark)
    val base = if (dark) darkColorScheme() else lightColorScheme()
    val colors = base.copy(
        primary = h.accent,
        onPrimary = h.onAccent,
        secondary = h.text2,
        onSecondary = h.bg,
        background = h.bg,
        onBackground = h.text1,
        surface = h.surface1,
        onSurface = h.text1,
        surfaceVariant = h.surface3,
        onSurfaceVariant = h.text2,
        surfaceContainerLowest = h.bg,
        surfaceContainerLow = h.surface1,
        surfaceContainer = h.surface2,
        surfaceContainerHigh = h.surface2,
        surfaceContainerHighest = h.surface3,
        outline = h.border2,
        outlineVariant = h.border1,
        error = h.error,
        onError = h.onAccent,
        surfaceTint = Color.Transparent,
        scrim = h.ink(0.32f).copy(red = 0f, green = 0f, blue = 0f),
    )
    val type = Typography().let { d ->
        fun TextStyle.sans() = copy(fontFamily = Sans)
        Typography(
            displayLarge = d.displayLarge.sans(), displayMedium = d.displayMedium.sans(), displaySmall = d.displaySmall.sans(),
            headlineLarge = d.headlineLarge.sans(), headlineMedium = d.headlineMedium.sans(),
            headlineSmall = d.headlineSmall.sans().copy(fontWeight = FontWeight.SemiBold),
            titleLarge = d.titleLarge.sans().copy(fontWeight = FontWeight.SemiBold, fontSize = 20.sp),
            titleMedium = d.titleMedium.sans().copy(fontWeight = FontWeight.SemiBold),
            titleSmall = d.titleSmall.sans().copy(fontWeight = FontWeight.Medium),
            bodyLarge = d.bodyLarge.sans().copy(fontSize = 15.sp, lineHeight = 22.sp),
            bodyMedium = d.bodyMedium.sans().copy(fontSize = 14.sp, lineHeight = 20.sp),
            bodySmall = d.bodySmall.sans().copy(fontSize = 12.5.sp, lineHeight = 17.sp),
            labelLarge = d.labelLarge.sans().copy(fontWeight = FontWeight.Medium),
            labelMedium = d.labelMedium.sans().copy(fontWeight = FontWeight.Medium),
            labelSmall = d.labelSmall.sans(),
        )
    }
    CompositionLocalProvider(LocalHues provides h) {
        MaterialTheme(colorScheme = colors, typography = type, content = content)
    }
}
