// Small pieces the screens share: an agent's mark, a thread's state, a space's tag, the
// connection strip, and times written the way the desktop's sidebar writes them.

package com.hyprspace.android.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.net.Agent
import com.hyprspace.android.net.BoardSpace
import com.hyprspace.android.net.BoardStatus
import com.hyprspace.android.net.Conn
import kotlinx.coroutines.delay

/** An agent's signature color: the same pair crates/theme gives the desktop. */
fun brand(agent: Agent?): Color = when (agent) {
    Agent.Claude -> Color(0xFFD97757)
    Agent.Codex -> Color(0xFF10A37F)
    null -> Color(0xFF8F8F8F)
}

@Composable
fun AgentMark(agent: Agent?, size: Dp = 16.dp, tint: Color = brand(agent)) {
    when (agent) {
        Agent.Claude -> Icon(painterResource(R.drawable.mark_claude), "Claude", Modifier.size(size), tint = tint)
        Agent.Codex -> Icon(painterResource(R.drawable.mark_codex), "Codex", Modifier.size(size), tint = tint)
        null -> Text(
            ">_",
            fontFamily = Mono,
            fontSize = (size.value * 0.62f).sp,
            fontWeight = FontWeight.SemiBold,
            color = LocalHues.current.text2,
        )
    }
}

/** A space's tag: its initials on its own color, the desktop's square. */
@Composable
fun SpaceTag(space: BoardSpace, size: Dp = 18.dp) {
    val h = LocalHues.current
    val (fill, ink) = space.tag.let { t ->
        if (t.size >= 4) {
            if (h.dark) rgba(t[2]) to rgba(t[3]) else rgba(t[0]) to rgba(t[1])
        } else {
            h.surface3 to h.text2
        }
    }
    Box(
        Modifier.size(size).clip(RoundedCornerShape(size / 4)).background(fill),
        contentAlignment = Alignment.Center,
    ) {
        Text(initials(space.name), color = ink, fontFamily = Mono, fontSize = (size.value * 0.5f).sp, fontWeight = FontWeight.Bold)
    }
}

/**
 * Two letters from a name, the desktop's rule: the first letters of its first two words, or for
 * one word its first letter and its first digit (else its last letter).
 */
fun initials(name: String): String {
    val words = name.split(Regex("""[^\p{L}\p{N}]+""")).filter { it.isNotEmpty() }
    val pair = when {
        words.isEmpty() -> ""
        words.size == 1 -> {
            val w = words[0]
            val rest = w.drop(1)
            w.take(1) + (rest.firstOrNull { it in '0'..'9' }?.toString() ?: rest.takeLast(1))
        }
        else -> "${words[0].first()}${words[1].first()}"
    }
    return pair.uppercase()
}

/** Where a thread stands: a pulse when it waits on you, a spinner while it works, a check when done. */
@Composable
fun StatusMark(status: BoardStatus, unseen: Boolean) {
    val h = LocalHues.current
    when (status) {
        BoardStatus.Working -> CircularProgressIndicator(
            modifier = Modifier.size(12.dp),
            strokeWidth = 1.6.dp,
            color = h.busy,
            trackColor = h.ink(0.08f),
        )
        BoardStatus.Waiting -> {
            val pulse by rememberInfiniteTransition(label = "wait").animateFloat(
                0.35f, 1f, infiniteRepeatable(tween(1000), RepeatMode.Reverse), label = "wait",
            )
            Box(Modifier.size(9.dp).alpha(pulse).clip(CircleShape).background(h.waiting))
        }
        BoardStatus.Done -> Box(
            Modifier.size(9.dp).clip(CircleShape).background(if (unseen) h.ok else h.ok.copy(alpha = 0.45f)),
        )
        BoardStatus.Failed -> Box(Modifier.size(9.dp).clip(CircleShape).background(h.error))
        BoardStatus.Idle -> {}
    }
}

/** "now", "5m", "3h", "2d", "1w", as the desktop's sidebar writes ages. */
fun ago(then: Long, now: Long): String {
    val s = (now - then).coerceAtLeast(0) / 1000
    return when {
        s < 60 -> "now"
        s < 3600 -> "${s / 60}m"
        s < 86_400 -> "${s / 3600}h"
        s < 1_209_600 -> "${s / 86_400}d"
        else -> "${s / 604_800}w"
    }
}

/** A running count, "0:42" or "1:02:03", like a stopwatch. */
fun elapsed(secs: Long): String {
    val h = secs / 3600
    val m = (secs % 3600) / 60
    val s = secs % 60
    return if (h > 0) "%d:%02d:%02d".format(h, m, s) else "%d:%02d".format(m, s)
}

/** The current time, moving once a second while something on screen counts. */
@Composable
fun ticking(on: Boolean): Long {
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(on) {
        while (on) {
            now = System.currentTimeMillis()
            delay(1000)
        }
    }
    return now
}

/** A strip under the top bar while the line to the computer is down. */
@Composable
fun ConnStrip(conn: Conn, onRetry: () -> Unit) {
    val h = LocalHues.current
    val text = when (conn) {
        Conn.Connecting -> "Connecting"
        is Conn.Offline -> conn.message
        is Conn.Denied -> conn.message
        else -> return
    }
    val bad = conn is Conn.Denied
    Row(
        Modifier
            .fillMaxWidth()
            .background(if (bad) h.error.copy(alpha = 0.12f) else h.ink(0.04f))
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (conn !is Conn.Denied) {
            CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.6.dp, color = h.text3)
        }
        Text(
            text,
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodySmall,
            color = if (bad) h.error else h.text2,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (conn is Conn.Offline) {
            Text(
                "Retry",
                Modifier.clip(RoundedCornerShape(6.dp)).clickableQuiet(onRetry).padding(horizontal = 8.dp, vertical = 4.dp),
                style = MaterialTheme.typography.labelMedium,
                color = h.text1,
            )
        }
    }
}

/** A label like the desktop's tags: small, rounded, on a faint wash. */
@Composable
fun Pill(text: String, color: Color = LocalHues.current.text2) {
    Text(
        text,
        Modifier
            .clip(RoundedCornerShape(50))
            .background(LocalHues.current.ink(0.06f))
            .padding(horizontal = 7.dp, vertical = 2.dp),
        fontSize = 11.sp,
        fontWeight = FontWeight.Medium,
        color = color,
        maxLines = 1,
    )
}

/** A click with no ripple, for text-like buttons. */
fun Modifier.clickableQuiet(onClick: () -> Unit): Modifier = this.then(
    Modifier.clickable(
        interactionSource = androidx.compose.foundation.interaction.MutableInteractionSource(),
        indication = null,
        onClick = onClick,
    ),
)
