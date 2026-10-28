// What sits over a structured thread's box: the agent's background tasks, each with Stop and,
// for a subagent, its latest steps, and the messages queued for when the run ends, each with
// Edit, Steer and Remove, as the desktop shows them.

package com.hyprspace.android.ui.thread

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.model.Item
import com.hyprspace.android.model.label
import com.hyprspace.android.net.AsideView
import com.hyprspace.android.net.BgTask
import com.hyprspace.android.net.TaskKind
import com.hyprspace.android.ui.Eclipse
import com.hyprspace.android.ui.Feel
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.feel

/** A side question (`/btw`) and its answer so far, with a cross that closes it. */
@Composable
fun AsideCard(aside: AsideView, onDrop: () -> Unit) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp)
            .padding(top = 6.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.045f))
            .padding(start = 12.dp, end = 4.dp, bottom = 10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(painterResource(R.drawable.ic_circle_help), null, Modifier.size(14.dp), tint = h.text3)
            Text(
                aside.question,
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                color = h.text2,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (!aside.done) Eclipse(h.text3)
            Round(R.drawable.ic_x, "Close") { onDrop() }
        }
        Box(Modifier.heightIn(max = 220.dp).verticalScroll(rememberScrollState()).padding(end = 8.dp)) {
            when {
                aside.error != null -> Text(aside.error, style = MaterialTheme.typography.bodyMedium, color = h.text3)
                aside.text.isEmpty() -> Text("Answering", style = MaterialTheme.typography.bodyMedium, color = h.text3)
                else -> Markdown(aside.text)
            }
        }
    }
}

/** A message waiting for the run to end; [sent] while it's only on its way to the computer. */
data class Waiting(val text: String, val images: Int, val sent: Boolean)

@Composable
fun Tray(
    tasks: List<BgTask>,
    items: List<Item>,
    queue: List<Waiting>,
    canAct: Boolean,
    onStopTask: (String) -> Unit,
    onSendNow: (Int, String) -> Unit,
    onEdit: (Int, String) -> Unit,
    onRemove: (Int, String) -> Unit,
) {
    if (tasks.isEmpty() && queue.isEmpty()) return
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(max = 280.dp)
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 12.dp)
            .padding(top = 6.dp)
            .animateContentSize(),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        for (t in tasks) TaskRow(t, items, canAct, onStopTask)
        if (queue.isNotEmpty()) {
            Text(
                if (queue.size == 1) "Queued for when it finishes" else "${queue.size} queued for when it finishes",
                Modifier.padding(start = 4.dp, top = 2.dp),
                style = MaterialTheme.typography.labelMedium,
                color = LocalHues.current.text3,
            )
        }
        queue.forEachIndexed { i, q -> QueuedRow(i, q, canAct, onSendNow, onEdit, onRemove) }
    }
}

@Composable
private fun TaskRow(t: BgTask, items: List<Item>, canAct: Boolean, onStop: (String) -> Unit) {
    val h = LocalHues.current
    var open by rememberSaveable(t.id) { mutableStateOf(false) }
    val host = LocalView.current
    val (icon, kind) = when (t.kind) {
        TaskKind.Command -> R.drawable.ic_terminal to "Background command"
        TaskKind.Monitor -> R.drawable.ic_gauge to "Monitor"
        TaskKind.Subagent -> R.drawable.ic_bot to "Background subagent"
        TaskKind.Other -> R.drawable.ic_zap to "Background task"
    }
    val agent = t.tool?.let { id -> items.lastOrNull { it is Item.Agent && it.id == id } as Item.Agent? }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.045f)),
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .then(if (t.kind == TaskKind.Subagent) Modifier.clickableQuiet { host.feel(Feel.Tick); open = !open } else Modifier)
                .padding(start = 12.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(painterResource(icon), null, Modifier.size(14.dp), tint = h.text3)
            Column(Modifier.weight(1f)) {
                Text(
                    t.label.ifBlank { kind },
                    style = MaterialTheme.typography.bodyMedium,
                    color = h.text1,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(kind, fontSize = 11.sp, color = h.text3)
            }
            Eclipse(h.text3)
            if (canAct) Pill("Stop") { onStop(t.id) }
        }
        if (open) {
            val steps = agent?.calls?.takeLast(6)?.map { label(it.tool) }.orEmpty()
            Column(
                Modifier.padding(start = 34.dp, end = 12.dp, bottom = 10.dp),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text("Latest steps", fontSize = 11.sp, color = h.text3)
                if (steps.isEmpty()) {
                    Text("No steps yet", style = MaterialTheme.typography.bodySmall, color = h.text3)
                } else {
                    for (s in steps) {
                        Text(s, style = MaterialTheme.typography.bodySmall, fontFamily = com.hyprspace.android.ui.Mono, color = h.text2, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}

@Composable
private fun QueuedRow(
    i: Int,
    q: Waiting,
    canAct: Boolean,
    onSendNow: (Int, String) -> Unit,
    onEdit: (Int, String) -> Unit,
    onRemove: (Int, String) -> Unit,
) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .alpha(if (q.sent) 0.6f else 1f)
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.045f))
            .padding(start = 12.dp, end = 6.dp, top = 8.dp, bottom = 4.dp),
    ) {
        Text(q.text, style = MaterialTheme.typography.bodyMedium, color = h.text1, maxLines = 3, overflow = TextOverflow.Ellipsis)
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                when {
                    q.sent -> "Queuing"
                    q.images == 1 -> "1 image"
                    q.images > 1 -> "${q.images} images"
                    else -> ""
                },
                Modifier.weight(1f),
                fontSize = 11.sp,
                color = h.text3,
            )
            if (canAct && !q.sent) {
                Round(R.drawable.ic_pencil, "Edit") { onEdit(i, q.text) }
                Round(R.drawable.ic_x, "Remove") { onRemove(i, q.text) }
                Pill("Steer") { onSendNow(i, q.text) }
            }
        }
    }
}

@Composable
private fun Pill(text: String, onClick: () -> Unit) {
    val h = LocalHues.current
    Box(
        Modifier
            .padding(4.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(h.ink(0.08f))
            .clickableQuiet(onClick)
            .padding(horizontal = 10.dp, vertical = 5.dp),
    ) {
        Text(text, style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Medium, color = h.text1)
    }
}

@Composable
private fun Round(icon: Int, what: String, onClick: () -> Unit) {
    val h = LocalHues.current
    Box(
        Modifier.size(32.dp).clip(CircleShape).clickableQuiet(onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(painterResource(icon), what, Modifier.size(15.dp), tint = h.text2)
    }
}
