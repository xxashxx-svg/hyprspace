package com.hyprspace.android.ui.home

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.hyprspace.android.net.SnoozeUntil
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

data class Swipe(val label: String, val icon: Int, val color: Color, val onSwipe: () -> Unit)

@Composable
fun SwipeRow(
    left: Swipe?,
    right: Swipe?,
    reset: Int,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val offset = remember { Animatable(0f) }
    var width by remember { mutableFloatStateOf(1f) }
    val scope = rememberCoroutineScope()
    val haptic = LocalHapticFeedback.current
    val toRight = offset.value > 0
    val armed = abs(offset.value) > width * ARM
    LaunchedEffect(armed) {
        if (armed) haptic.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
    }
    LaunchedEffect(reset) {
        if (reset > 0) offset.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = Spring.StiffnessMediumLow))
    }
    val drag = rememberDraggableState { delta ->
        val next = offset.value + delta
        val allowed = (next > 0 && right != null) || (next < 0 && left != null)
        scope.launch { offset.snapTo(if (allowed) next else next * 0.15f) }
    }
    Box(
        modifier
            .onSizeChanged { width = it.width.toFloat().coerceAtLeast(1f) }
            .draggable(
                drag,
                Orientation.Horizontal,
                onDragStopped = { velocity ->
                    val dir = if (offset.value > 0) 1f else -1f
                    val s = if (dir > 0) right else left
                    val flung = velocity * dir > 1800f && abs(offset.value) > width * 0.12f
                    if (s != null && (abs(offset.value) > width * ARM || flung)) {
                        offset.animateTo(dir * width, tween(200))
                        s.onSwipe()
                    } else {
                        offset.animateTo(0f, spring(dampingRatio = 0.75f, stiffness = Spring.StiffnessMediumLow))
                    }
                },
            ),
    ) {
        val s = if (toRight) right else left
        if (offset.value != 0f) Box(Modifier.matchParentSize()) { Reveal(s, toRight, armed) }
        Box(Modifier.offset { IntOffset(offset.value.roundToInt(), 0) }) { content() }
    }
}

private const val ARM = 0.3f

@Composable
private fun Reveal(s: Swipe?, toRight: Boolean, armed: Boolean) {
    val h = LocalHues.current
    val fill by animateColorAsState(
        when {
            s == null -> Color.Transparent
            armed -> s.color
            else -> s.color.copy(alpha = 0.22f)
        },
        label = "fill",
    )
    val ink by animateColorAsState(if (armed) h.onAccent else s?.color ?: h.text2, label = "ink")
    val pop by animateFloatAsState(
        if (armed) 1.12f else 0.86f,
        spring(dampingRatio = 0.45f, stiffness = Spring.StiffnessMediumLow),
        label = "pop",
    )
    Box(
        Modifier
            .fillMaxSize()
            .padding(horizontal = 8.dp, vertical = 2.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(fill)
            .padding(horizontal = 22.dp),
        contentAlignment = if (toRight) Alignment.CenterStart else Alignment.CenterEnd,
    ) {
        if (s == null) return@Box
        Row(
            Modifier.scale(pop),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (!toRight) Text(s.label, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold, color = ink)
            Icon(painterResource(s.icon), null, Modifier.size(20.dp), tint = ink)
            if (toRight) Text(s.label, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold, color = ink)
        }
    }
}

data class Wake(val label: String, val at: LocalDateTime)

object Wakes {
    fun presets(now: LocalDateTime): List<Wake> = buildList {
        val today = now.toLocalDate()
        add(Wake("In 1 hour", now.plusHours(1)))
        add(Wake("In 3 hours", now.plusHours(3)))
        if (now.toLocalTime() < LocalTime.of(17, 0)) add(Wake("This evening", today.atTime(18, 0)))
        add(Wake("Tomorrow", today.plusDays(1).atTime(9, 0)))
        add(Wake("Next week", today.plusDays(7L - (today.dayOfWeek.value - 1)).atTime(9, 0)))
    }

    fun short(at: LocalDateTime, today: LocalDate, locale: Locale = Locale.getDefault()): String {
        val time = at.format(DateTimeFormatter.ofPattern("h:mm a", locale))
        return when (at.toLocalDate()) {
            today -> time
            today.plusDays(1) -> "Tomorrow $time"
            else -> at.format(DateTimeFormatter.ofPattern("EEE", locale)) + " $time"
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SnoozeSheet(working: Boolean, onPick: (SnoozeUntil, String) -> Unit, onDismiss: () -> Unit) {
    val h = LocalHues.current
    val now = remember { LocalDateTime.now() }
    val zone = remember { ZoneId.systemDefault() }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Snooze until", style = MaterialTheme.typography.titleLarge, color = h.text1)
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(h.ink(0.04f))) {
                val wakes = Wakes.presets(now)
                wakes.forEachIndexed { i, w ->
                    if (i > 0) HorizontalDivider(Modifier.padding(horizontal = 14.dp), color = h.border0)
                    val label = Wakes.short(w.at, now.toLocalDate())
                    val detail = if (w.label == "Tomorrow") label.removePrefix("Tomorrow ") else label
                    Choice(w.label, detail) { onPick(SnoozeUntil.at(w.at.atZone(zone).toInstant().toEpochMilli()), label) }
                }
                if (working) {
                    HorizontalDivider(Modifier.padding(horizontal = 14.dp), color = h.border0)
                    Choice("Until it finishes", "When the turn ends") { onPick(SnoozeUntil.Done, "it finishes") }
                }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun Choice(label: String, detail: String, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onClick).padding(horizontal = 14.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = h.text1)
        Text(detail, style = MaterialTheme.typography.bodyMedium, color = h.text3)
    }
}
