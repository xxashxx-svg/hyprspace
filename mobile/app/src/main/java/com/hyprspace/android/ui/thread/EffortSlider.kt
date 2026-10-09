package com.hyprspace.android.ui.thread

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.hyprspace.android.ui.LocalHues
import kotlin.math.roundToInt

fun effortLabel(e: String): String = when (e) {
    "" -> "Default"
    "xhigh" -> "Extra high"
    else -> e.replaceFirstChar { it.uppercase() }
}

private val TRACK = 30.dp
private val KNOB = 24.dp

@Composable
fun EffortSlider(levels: List<String>, value: String, fallback: String?, onPick: (String) -> Unit) {
    val h = LocalHues.current
    val haptic = LocalHapticFeedback.current
    val density = LocalDensity.current
    val resting = levels.indexOf(value.ifEmpty { fallback ?: "" }).takeIf { it >= 0 } ?: (levels.size / 2)
    var dragging by remember { mutableStateOf<Int?>(null) }
    var width by remember { mutableIntStateOf(1) }
    val at = dragging ?: resting
    val shown = levels.getOrElse(at) { "" }
    val end = with(density) { (TRACK / 2).toPx() }
    val span = (width - 2 * end).coerceAtLeast(1f)
    val stop = { x: Float -> ((x - end) / span * (levels.size - 1)).roundToInt().coerceIn(0, levels.lastIndex) }
    val center by animateFloatAsState(
        end + span * at / (levels.size - 1).coerceAtLeast(1),
        spring(dampingRatio = 0.8f, stiffness = Spring.StiffnessMedium),
        label = "knob",
    )
    fun move(i: Int) {
        if (i != at) haptic.performHapticFeedback(HapticFeedbackType.SegmentTick)
        dragging = i
    }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Effort", Modifier.weight(1f), style = MaterialTheme.typography.labelMedium, color = h.text3)
            Text(effortLabel(shown), style = MaterialTheme.typography.labelLarge, color = h.text1)
            if (value.isEmpty() && dragging == null) {
                Text("  · default", style = MaterialTheme.typography.labelLarge, color = h.text3)
            }
        }
        Box(
            Modifier
                .fillMaxWidth()
                .height(TRACK)
                .onSizeChanged { width = it.width }
                .clip(CircleShape)
                .background(h.ink(0.07f))
                .pointerInput(levels) {
                    detectTapGestures { o -> onPick(levels[stop(o.x)]) }
                }
                .pointerInput(levels) {
                    detectHorizontalDragGestures(
                        onDragStart = { o -> move(stop(o.x)) },
                        onDragEnd = { dragging?.let { onPick(levels[it]) }; dragging = null },
                        onDragCancel = { dragging = null },
                    ) { change, _ -> move(stop(change.position.x)) }
                },
        ) {
            Box(
                Modifier
                    .fillMaxHeight()
                    .width(with(density) { (center + end).toDp() })
                    .clip(CircleShape)
                    .background(h.accent),
            )
            for (i in levels.indices) {
                if (i <= at) continue
                val x = end + span * i / (levels.size - 1).coerceAtLeast(1)
                Box(
                    Modifier
                        .align(Alignment.CenterStart)
                        .offset { IntOffset((x - 2.dp.toPx()).roundToInt(), 0) }
                        .size(4.dp)
                        .clip(CircleShape)
                        .background(h.ink(0.28f)),
                )
            }
            Box(
                Modifier
                    .align(Alignment.CenterStart)
                    .offset { IntOffset((center - KNOB.toPx() / 2).roundToInt(), 0) }
                    .size(KNOB)
                    .clip(CircleShape)
                    .background(h.onAccent),
            )
        }
    }
}
