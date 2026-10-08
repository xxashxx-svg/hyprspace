// A structured thread on the phone: the conversation with its tool calls folded into one line
// per run of calls, subagents, approvals to answer, and a box to reply, steer or stop.

package com.hyprspace.android.ui.thread

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.model.AgentState
import com.hyprspace.android.model.Item
import com.hyprspace.android.model.counts
import com.hyprspace.android.model.input
import com.hyprspace.android.model.label
import com.hyprspace.android.model.summary
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.RunStatus
import com.hyprspace.android.net.Tool
import com.hyprspace.android.net.TranscriptView
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.elapsed
import kotlinx.coroutines.flow.drop

/** What the list shows: the transcript's items, with runs of calls in a row folded together. */
private sealed interface Shown {
    val key: String
    data class One(val item: Item) : Shown { override val key = item.key }
    data class Calls(val calls: List<Item.Call>) : Shown { override val key = "calls" + calls.first().key }
}

private fun fold(items: List<Item>): List<Shown> {
    val out = ArrayList<Shown>()
    var run = ArrayList<Item.Call>()
    fun flush() {
        when (run.size) {
            0 -> {}
            1 -> out += Shown.One(run[0])
            else -> out += Shown.Calls(run)
        }
        run = ArrayList()
    }
    for (i in items) {
        if (i is Item.Call) run += i else {
            flush()
            out += Shown.One(i)
        }
    }
    flush()
    return out
}

@Composable
fun Chat(
    view: TranscriptView,
    working: Boolean,
    canAnswer: Boolean,
    onSend: (String) -> Unit,
    onStop: () -> Unit,
    onAnswer: (String, Answer) -> Unit,
    modifier: Modifier = Modifier,
) {
    val h = LocalHues.current
    val shown = remember(view.items) { fold(view.items) }
    // opens at the newest message and keeps up with the reply, unless the reader scrolled up
    val list = rememberLazyListState(initialFirstVisibleItemIndex = (shown.size - 1).coerceAtLeast(0))
    var follow by remember { mutableStateOf(true) }
    LaunchedEffect(list) {
        snapshotFlow { list.isScrollInProgress }.drop(1).collect { moving ->
            if (!moving) follow = !list.canScrollForward
        }
    }
    LaunchedEffect(shown.size, view.items.lastOrNull(), follow) {
        if (follow && shown.isNotEmpty()) list.scrollToItem(shown.size - 1, Int.MAX_VALUE / 2)
    }
    Column(modifier) {
        if (!view.loaded) {
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = h.text3)
            }
        } else {
            LazyColumn(
                Modifier.weight(1f).fillMaxWidth(),
                state = list,
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (shown.isEmpty()) {
                    item { Text("Nothing here yet. Say what to do below.", color = h.text3, style = MaterialTheme.typography.bodyMedium) }
                }
                items(shown, key = { it.key }) { s ->
                    when (s) {
                        is Shown.Calls -> CallRun(s.calls)
                        is Shown.One -> One(s.item, canAnswer, onAnswer)
                    }
                }
            }
        }
        Composer(working, { follow = true; onSend(it) }, onStop)
    }
}

@Composable
private fun One(item: Item, canAnswer: Boolean, onAnswer: (String, Answer) -> Unit) {
    val h = LocalHues.current
    when (item) {
        is Item.User -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .clip(RoundedCornerShape(16.dp, 16.dp, 4.dp, 16.dp))
                    .background(h.surface3)
                    .padding(horizontal = 14.dp, vertical = 10.dp),
            ) {
                if (item.steer) Text("Sent while it worked", fontSize = 11.sp, color = h.text3)
                Text(item.text, style = MaterialTheme.typography.bodyLarge, color = h.text1)
                if (item.images > 0) {
                    Text(if (item.images == 1) "1 image" else "${item.images} images", fontSize = 11.sp, color = h.text3)
                }
            }
        }
        is Item.Reply -> Markdown(item.text)
        is Item.Thinking -> Fold("Thinking", item.text, mono = false)
        is Item.Call -> CallLine(item)
        is Item.Agent -> AgentCard(item)
        is Item.Approval -> ApprovalCard(item, canAnswer, onAnswer)
        is Item.Error -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(painterResource(R.drawable.ic_circle_alert), null, Modifier.size(16.dp).padding(top = 2.dp), tint = h.error)
            Text(item.message, style = MaterialTheme.typography.bodyMedium, color = h.error)
        }
        is Item.Finished -> {
            val text = when (item.status) {
                RunStatus.Done -> "Done in ${elapsed(item.ms / 1000)}"
                RunStatus.Interrupted -> "Stopped after ${elapsed(item.ms / 1000)}"
                RunStatus.Failed -> item.error?.let { "Failed: $it" } ?: "Failed"
            }
            Text(
                text,
                Modifier.fillMaxWidth(),
                fontSize = 12.sp,
                color = if (item.status == RunStatus.Failed) h.error else h.text3,
            )
        }
    }
}

/** A line that opens to show more. */
@Composable
private fun Fold(title: String, body: String, mono: Boolean) {
    val h = LocalHues.current
    var open by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.animateContentSize()) {
        Row(Modifier.clickableQuiet { open = !open }, verticalAlignment = Alignment.CenterVertically) {
            Text(title, style = MaterialTheme.typography.bodyMedium, color = h.text3)
            Icon(painterResource(if (open) R.drawable.ic_chevron_down else R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
        }
        if (open) {
            Text(
                body,
                Modifier.padding(top = 6.dp),
                style = MaterialTheme.typography.bodyMedium,
                fontFamily = if (mono) Mono else null,
                color = h.text2,
            )
        }
    }
}

private fun icon(tool: Tool): Int = when (tool) {
    is Tool.Command -> R.drawable.ic_terminal
    is Tool.Read -> R.drawable.ic_file_diff
    is Tool.Edit -> R.drawable.ic_file_diff
    is Tool.Search -> R.drawable.ic_search
    is Tool.Web -> R.drawable.ic_external_link
    is Tool.AgentCall -> R.drawable.ic_bot
    else -> R.drawable.ic_sparkles
}

@Composable
private fun CallRun(calls: List<Item.Call>) {
    val h = LocalHues.current
    var open by rememberSaveable { mutableStateOf(false) }
    val live = calls.any { it.done == null }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .border(1.dp, h.border1, RoundedCornerShape(10.dp))
            .animateContentSize(),
    ) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { open = !open }.padding(horizontal = 12.dp, vertical = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (live) CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = h.busy)
            else Icon(painterResource(R.drawable.ic_check), null, Modifier.size(13.dp), tint = h.text3)
            Text(summary(calls.map { it.tool }), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = h.text2, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Icon(painterResource(if (open) R.drawable.ic_chevron_down else R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
        }
        if (open) {
            Column(Modifier.padding(start = 8.dp, end = 8.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                for (c in calls) CallLine(c, boxed = false)
            }
        }
    }
}

@Composable
private fun CallLine(c: Item.Call, boxed: Boolean = true) {
    val h = LocalHues.current
    var open by rememberSaveable(c.key) { mutableStateOf(false) }
    val failed = c.done?.ok == false
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .then(if (boxed) Modifier.border(1.dp, h.border1, RoundedCornerShape(10.dp)) else Modifier)
            .animateContentSize(),
    ) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { open = !open }.padding(horizontal = if (boxed) 12.dp else 6.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (c.done == null) CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = h.busy)
            else Icon(painterResource(icon(c.tool)), null, Modifier.size(13.dp), tint = if (failed) h.error else h.text3)
            Text(label(c.tool), Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = if (failed) h.error else h.text2, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val tool = c.tool
            if (tool is Tool.Edit) {
                val (add, del) = counts(tool.changes)
                Text("+$add", fontFamily = Mono, fontSize = 11.sp, color = h.diffAdd)
                Text("-$del", fontFamily = Mono, fontSize = 11.sp, color = h.diffDel)
            }
        }
        if (open) {
            Column(Modifier.padding(start = 10.dp, end = 10.dp, bottom = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                input(c.tool)?.let { Mono(it) }
                val tool = c.tool
                if (tool is Tool.Edit) for (ch in tool.changes) Diff(ch.path, ch.diff)
                c.done?.output?.takeIf { it.isNotBlank() }?.let { Mono(it, maxLines = 40) }
            }
        }
    }
}

@Composable
private fun Mono(text: String, maxLines: Int = Int.MAX_VALUE) {
    val h = LocalHues.current
    Text(
        text,
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(h.ink(0.045f))
            .horizontalScroll(rememberScrollState())
            .padding(10.dp),
        fontFamily = Mono,
        fontSize = 12.sp,
        lineHeight = 17.sp,
        color = h.text2,
        softWrap = false,
        maxLines = maxLines,
        overflow = TextOverflow.Ellipsis,
    )
}

/** One file's diff, added and removed lines marked as the desktop marks them. */
@Composable
fun Diff(path: String, diff: String) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .border(1.dp, h.border1, RoundedCornerShape(8.dp)),
    ) {
        Text(path, Modifier.padding(horizontal = 10.dp, vertical = 6.dp), fontFamily = Mono, fontSize = 11.sp, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Column(Modifier.horizontalScroll(rememberScrollState())) {
            for (line in diff.lines().take(400)) {
                val (bg, fg) = when {
                    line.startsWith("+") && !line.startsWith("+++") -> h.diffAdd.copy(alpha = 0.13f) to h.text1
                    line.startsWith("-") && !line.startsWith("---") -> h.diffDel.copy(alpha = 0.13f) to h.text1
                    line.startsWith("@@") -> h.ink(0.04f) to h.text3
                    else -> androidx.compose.ui.graphics.Color.Transparent to h.text2
                }
                Text(
                    line.ifEmpty { " " },
                    Modifier.background(bg).padding(horizontal = 10.dp, vertical = 1.dp).widthIn(min = 360.dp),
                    fontFamily = Mono,
                    fontSize = 11.5.sp,
                    color = fg,
                    softWrap = false,
                )
            }
        }
    }
}

@Composable
private fun AgentCard(a: Item.Agent) {
    val h = LocalHues.current
    var open by rememberSaveable(a.key) { mutableStateOf(false) }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .border(1.dp, h.border1, RoundedCornerShape(12.dp))
            .animateContentSize(),
    ) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { open = !open }.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            when (a.state) {
                AgentState.Working -> CircularProgressIndicator(Modifier.size(13.dp), strokeWidth = 1.5.dp, color = h.busy)
                AgentState.Done -> Icon(painterResource(R.drawable.ic_circle_check), null, Modifier.size(14.dp), tint = h.ok)
                else -> Icon(painterResource(R.drawable.ic_circle_alert), null, Modifier.size(14.dp), tint = h.error)
            }
            Column(Modifier.weight(1f)) {
                Text(a.description.ifBlank { a.agentType.ifBlank { "Subagent" } }, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = h.text1, maxLines = 2, overflow = TextOverflow.Ellipsis)
                val sub = when (a.state) {
                    AgentState.Working -> if (a.calls.isEmpty()) "Starting" else summary(a.calls.map { it.tool })
                    AgentState.Done -> "Reported back"
                    AgentState.Failed -> "Failed"
                    AgentState.Stopped -> "Stopped before it reported"
                }
                Text(sub, fontSize = 12.sp, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            Icon(painterResource(if (open) R.drawable.ic_chevron_down else R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
        }
        if (open) {
            Column(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (a.prompt.isNotBlank()) Fold("What it was asked", a.prompt, mono = false)
                for (c in a.calls) CallLine(c, boxed = false)
                if (a.answer.isNotBlank()) Markdown(a.answer)
            }
        }
    }
}

@Composable
private fun ApprovalCard(a: Item.Approval, canAnswer: Boolean, onAnswer: (String, Answer) -> Unit) {
    val h = LocalHues.current
    val pending = a.answer == null && !a.expired
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(if (pending) h.waiting.copy(alpha = 0.08f) else h.bg)
            .border(1.dp, if (pending) h.waiting.copy(alpha = 0.45f) else h.border1, RoundedCornerShape(12.dp))
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(painterResource(R.drawable.ic_hand), null, Modifier.size(15.dp), tint = if (pending) h.waiting else h.text3)
            Text(
                if (pending) "Wants to ${label(a.tool).replaceFirstChar { it.lowercase() }}" else label(a.tool),
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = h.text1,
            )
        }
        a.reason?.takeIf { it.isNotBlank() }?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = h.text2) }
        input(a.tool)?.let { Mono(it, maxLines = 12) }
        val tool = a.tool
        if (tool is Tool.Edit) for (ch in tool.changes) Diff(ch.path, ch.diff)
        when {
            pending && canAnswer -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Choice("Allow", primary = true) { onAnswer(a.request, Answer.Allow) }
                if (a.always) Choice("Always") { onAnswer(a.request, Answer.AllowAlways) }
                Choice("Deny") { onAnswer(a.request, Answer.Deny) }
            }
            pending -> Text("Waiting for an answer.", fontSize = 12.sp, color = h.text3)
            a.answer == Answer.Allow -> Text("Allowed", fontSize = 12.sp, color = h.ok)
            a.answer == Answer.AllowAlways -> Text("Allowed for the rest of the session", fontSize = 12.sp, color = h.ok)
            a.answer == Answer.Deny -> Text("Denied", fontSize = 12.sp, color = h.error)
            else -> Text("The run ended before an answer.", fontSize = 12.sp, color = h.text3)
        }
    }
}

@Composable
private fun Choice(text: String, primary: Boolean = false, onClick: () -> Unit) {
    val h = LocalHues.current
    Text(
        text,
        Modifier
            .clip(RoundedCornerShape(9.dp))
            .background(if (primary) h.accent else h.surface3)
            .clickableQuiet(onClick)
            .padding(horizontal = 16.dp, vertical = 9.dp),
        style = MaterialTheme.typography.labelLarge,
        color = if (primary) h.onAccent else h.text1,
    )
}

@Composable
private fun Composer(working: Boolean, onSend: (String) -> Unit, onStop: () -> Unit) {
    val h = LocalHues.current
    var text by rememberSaveable { mutableStateOf("") }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(18.dp))
            .background(h.surface2)
            .border(1.dp, h.border1, RoundedCornerShape(18.dp))
            .padding(start = 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.weight(1f).padding(vertical = 8.dp)) {
            if (text.isEmpty()) {
                Text(if (working) "Steer it, or stop it" else "Reply", style = MaterialTheme.typography.bodyLarge, color = h.text3)
            }
            BasicTextField(
                value = text,
                onValueChange = { text = it },
                modifier = Modifier.fillMaxWidth().heightIn(max = 160.dp),
                textStyle = MaterialTheme.typography.bodyLarge.copy(color = h.text1),
                cursorBrush = SolidColor(h.accent),
            )
        }
        Spacer(Modifier.width(6.dp))
        val stop = working && text.isBlank()
        Box(
            Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(if (stop) h.surface3 else if (text.isBlank()) h.ink(0.08f) else h.accent)
                .clickableQuiet {
                    if (stop) onStop() else if (text.isNotBlank()) {
                        onSend(text.trim())
                        text = ""
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painterResource(if (stop) R.drawable.ic_square else R.drawable.ic_arrow_up),
                if (stop) "Stop" else "Send",
                Modifier.size(16.dp),
                tint = if (stop) h.text1 else if (text.isBlank()) h.text3 else h.onAccent,
            )
        }
    }
}
