// A structured thread on the phone: the conversation with its tool calls folded into one line
// per run of calls, subagents, approvals to answer, and a box to reply, steer or stop.

package com.hyprspace.android.ui.thread

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
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
import com.hyprspace.android.ui.Eclipse
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
    // the agent is at it now, not waiting on an answer
    busy: Boolean,
    doing: String?,
    since: Long?,
    canAnswer: Boolean,
    onSend: (String, List<String>) -> Unit,
    onStop: () -> Unit,
    onAnswer: (String, Answer) -> Unit,
    attach: (@Composable ((Photo) -> Unit) -> (() -> Unit))?,
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
    // what was sent or answered here shows at once, until the journal brings it back
    var pending by remember { mutableStateOf(listOf<String>()) }
    var answered by remember { mutableStateOf(mapOf<String, Answer>()) }
    LaunchedEffect(view.items) {
        val said = view.items.takeLast(8).filterIsInstance<Item.User>().map { it.text }
        pending = pending.filter { it !in said }
    }
    val rows = shown.size + pending.size + (if (busy) 1 else 0)
    LaunchedEffect(rows, view.items.lastOrNull(), follow) {
        if (follow && rows > 0) list.scrollToItem(rows - 1, Int.MAX_VALUE / 2)
    }
    val send: (String, List<String>) -> Unit = { text, images ->
        if (text.isNotBlank()) pending = pending + text
        follow = true
        onSend(text, images)
    }
    val answer: (String, Answer) -> Unit = { request, a ->
        answered = answered + (request to a)
        onAnswer(request, a)
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
                if (shown.isEmpty() && pending.isEmpty()) {
                    item { Text("Nothing here yet. Say what to do below.", color = h.text3, style = MaterialTheme.typography.bodyMedium) }
                }
                items(shown, key = { it.key }) { s ->
                    Box(Modifier.animateItem()) {
                        when (s) {
                            is Shown.Calls -> CallRun(s.calls)
                            is Shown.One -> One(s.item, canAnswer, answered, answer)
                        }
                    }
                }
                items(pending, key = { "pending$it" }) { text ->
                    Box(Modifier.animateItem()) {
                        One(Item.User("pending", text, 0, steer = working), false, answered, answer)
                    }
                }
                if (busy) {
                    item(key = "working") {
                        Box(Modifier.animateItem()) { Working(doing, since) }
                    }
                }
            }
        }
        Composer(working, send, onStop, attach)
    }
}

@Composable
private fun One(item: Item, canAnswer: Boolean, answered: Map<String, Answer>, onAnswer: (String, Answer) -> Unit) {
    val h = LocalHues.current
    when (item) {
        is Item.User -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .clip(RoundedCornerShape(18.dp))
                    .background(h.ink(0.06f))
                    .padding(horizontal = 15.dp, vertical = 10.dp),
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
        is Item.Approval -> ApprovalCard(item.copy(answer = item.answer ?: answered[item.request]), canAnswer, onAnswer)
        is Item.Error -> Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(h.error.copy(alpha = 0.08f)).padding(horizontal = 14.dp, vertical = 11.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(painterResource(R.drawable.ic_circle_alert), null, Modifier.size(16.dp).padding(top = 2.dp), tint = h.error)
            Text(item.message, style = MaterialTheme.typography.bodyMedium, color = h.text1)
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

/** A muted line led by a chevron that opens to show more. */
@Composable
private fun FoldHead(label: String, open: Boolean, trailing: @Composable () -> Unit = {}, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onClick).padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(painterResource(if (open) R.drawable.ic_chevron_down else R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
        Text(label, Modifier.weight(1f, fill = false), style = MaterialTheme.typography.bodyMedium, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
        trailing()
    }
}

@Composable
private fun Fold(title: String, body: String, mono: Boolean) {
    val h = LocalHues.current
    var open by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.animateContentSize()) {
        FoldHead(title, open) { open = !open }
        if (open) {
            Text(
                body,
                Modifier.padding(start = 22.dp, top = 4.dp),
                style = MaterialTheme.typography.bodyMedium,
                fontFamily = if (mono) Mono else null,
                fontStyle = if (mono) null else FontStyle.Italic,
                color = h.text3,
            )
        }
    }
}

@Composable
private fun CallRun(calls: List<Item.Call>) {
    val h = LocalHues.current
    var open by rememberSaveable { mutableStateOf(false) }
    val live = calls.any { it.done == null }
    val failed = calls.count { it.done?.ok == false }
    Column(Modifier.fillMaxWidth().animateContentSize()) {
        FoldHead(summary(calls.map { it.tool }), open, trailing = {
            if (failed > 0) Text("$failed failed", style = MaterialTheme.typography.bodySmall, color = h.text3)
            if (live) Eclipse(h.text3)
        }) { open = !open }
        if (open) {
            Column(Modifier.padding(start = 22.dp, top = 2.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                for (c in calls) CallLine(c)
            }
        }
    }
}

@Composable
private fun CallLine(c: Item.Call) {
    val h = LocalHues.current
    var open by rememberSaveable(c.key) { mutableStateOf(false) }
    val tool = c.tool
    val failed = c.done?.ok == false
    Column(Modifier.fillMaxWidth().animateContentSize()) {
        FoldHead(label(tool), open, trailing = {
            if (tool is Tool.Edit) {
                val (add, del) = counts(tool.changes)
                Text("+$add", fontFamily = Mono, fontSize = 11.sp, color = h.diffAdd)
                Text("-$del", fontFamily = Mono, fontSize = 11.sp, color = h.diffDel)
            }
            if (failed) Text("Failed", style = MaterialTheme.typography.bodySmall, color = if (tool is Tool.Edit) h.error else h.text3)
            if (c.done == null) Eclipse(h.text3)
        }) { open = !open }
        if (open) {
            Column(Modifier.padding(start = 22.dp, top = 4.dp, bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                input(tool)?.let { Mono(it) }
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
            .clip(RoundedCornerShape(10.dp))
            .background(h.ink(0.035f))
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 9.dp),
        fontFamily = Mono,
        fontSize = 12.sp,
        lineHeight = 17.sp,
        color = h.text2,
        softWrap = false,
        maxLines = maxLines,
    )
}

/** One file's diff, added and removed lines marked as the desktop marks them. */
@Composable
fun Diff(path: String, diff: String) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(h.ink(0.035f))
            .padding(bottom = 6.dp),
    ) {
        Row(
            Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(painterResource(R.drawable.ic_file_diff), null, Modifier.size(12.dp), tint = h.text3)
            Text(path, style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.Medium, color = h.text2, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Column(Modifier.horizontalScroll(rememberScrollState())) {
            for (line in diff.lines().take(400)) {
                val (bg, fg) = when {
                    line.startsWith("+") && !line.startsWith("+++") -> h.diffAdd.copy(alpha = 0.14f) to h.text1
                    line.startsWith("-") && !line.startsWith("---") -> h.diffDel.copy(alpha = 0.14f) to h.text1
                    line.startsWith("@@") -> androidx.compose.ui.graphics.Color.Transparent to h.text3
                    else -> androidx.compose.ui.graphics.Color.Transparent to h.text2
                }
                Text(
                    line.ifEmpty { " " },
                    Modifier.background(bg).padding(horizontal = 12.dp, vertical = 1.dp).widthIn(min = 360.dp),
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
            .background(h.ink(0.035f))
            .animateContentSize(),
    ) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { open = !open }.padding(start = 12.dp, end = 14.dp, top = 11.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(painterResource(if (open) R.drawable.ic_chevron_down else R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
            Icon(painterResource(R.drawable.ic_bot), null, Modifier.size(15.dp), tint = h.text2)
            Text(
                a.description.ifBlank { a.agentType.ifBlank { "Subagent" } },
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = h.text1,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            when (a.state) {
                AgentState.Working -> Eclipse(h.text3)
                AgentState.Done -> Text("Done", style = MaterialTheme.typography.bodySmall, color = h.text3)
                AgentState.Failed -> Text("Failed", style = MaterialTheme.typography.bodySmall, color = h.error)
                AgentState.Stopped -> Text("Stopped", style = MaterialTheme.typography.bodySmall, color = h.text3)
            }
        }
        val did = if (a.calls.isEmpty()) a.agentType.ifBlank { null } else summary(a.calls.map { it.tool })
        did?.let {
            Text(it, Modifier.padding(start = 57.dp, end = 14.dp), style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Spacer(Modifier.height(11.dp))
        if (open) {
            Column(Modifier.padding(start = 34.dp, end = 14.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                if (a.prompt.isNotBlank()) Fold("Prompt", a.prompt, mono = false)
                for (c in a.calls) CallLine(c)
                if (a.answer.isNotBlank()) Box(Modifier.padding(top = 4.dp)) { Markdown(a.answer) }
            }
        }
    }
}

@Composable
private fun ApprovalCard(a: Item.Approval, canAnswer: Boolean, onAnswer: (String, Answer) -> Unit) {
    val h = LocalHues.current
    val pending = a.answer == null && !a.expired
    val (mark, tint) = when {
        pending -> R.drawable.ic_hand to h.waiting
        a.answer == Answer.Deny -> R.drawable.ic_x to h.error
        a.answer != null -> R.drawable.ic_check to h.ok
        else -> R.drawable.ic_clock to h.text3
    }
    val state = when (a.answer) {
        Answer.Allow -> "Allowed"
        Answer.AllowAlways -> "Allowed for this session"
        Answer.Deny -> "Denied"
        null -> if (a.expired) "Not answered" else null
    }
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(if (pending) h.waiting.copy(alpha = 0.08f) else h.ink(0.035f))
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(painterResource(mark), null, Modifier.size(15.dp), tint = tint)
            Text(
                label(a.tool),
                Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = h.text1,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            state?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = h.text3) }
        }
        a.reason?.takeIf { it.isNotBlank() }?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = h.text2) }
        input(a.tool)?.let { Mono(it, maxLines = 12) }
        val tool = a.tool
        if (tool is Tool.Edit) for (ch in tool.changes) Diff(ch.path, ch.diff)
        if (pending && canAnswer) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Choice("Allow", primary = true) { onAnswer(a.request, Answer.Allow) }
                if (a.always) Choice("Always allow") { onAnswer(a.request, Answer.AllowAlways) }
                Choice("Deny") { onAnswer(a.request, Answer.Deny) }
            }
        } else if (pending) {
            Text("Waiting for an answer.", style = MaterialTheme.typography.bodySmall, color = h.text3)
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
            .background(if (primary) h.accent else h.ink(0.08f))
            .clickableQuiet(onClick)
            .padding(horizontal = 16.dp, vertical = 9.dp),
        style = MaterialTheme.typography.labelLarge,
        color = if (primary) h.onAccent else h.text1,
    )
}

@Composable
private fun Composer(
    working: Boolean,
    onSend: (String, List<String>) -> Unit,
    onStop: () -> Unit,
    attach: (@Composable ((Photo) -> Unit) -> (() -> Unit))?,
) {
    val h = LocalHues.current
    var text by rememberSaveable { mutableStateOf("") }
    val photos = remember { mutableStateListOf<Photo>() }
    val pick = attach?.invoke { p ->
        val i = photos.indexOfFirst { it.key == p.key }
        if (i >= 0) photos[i] = p else photos.add(p)
    }
    val ready = photos.none { it.path == null && !it.failed }
    val sendable = (text.isNotBlank() || photos.any { it.path != null }) && ready
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .clip(RoundedCornerShape(18.dp))
            .background(h.ink(0.045f)),
    ) {
    if (photos.isNotEmpty()) {
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 12.dp, end = 12.dp, top = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            for (p in photos) Thumb(p) { photos.remove(p) }
        }
    }
    Row(
        Modifier.fillMaxWidth().padding(start = if (pick != null) 4.dp else 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (pick != null) {
            Box(
                Modifier.size(36.dp).clip(CircleShape).clickableQuiet(pick),
                contentAlignment = Alignment.Center,
            ) {
                Icon(painterResource(R.drawable.ic_image_plus), "Add photos", Modifier.size(18.dp), tint = h.text2)
            }
            Spacer(Modifier.width(4.dp))
        }
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
        val stop = working && text.isBlank() && photos.isEmpty()
        Box(
            Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(if (stop) h.surface3 else if (!sendable) h.ink(0.08f) else h.accent)
                .clickableQuiet {
                    if (stop) {
                        onStop()
                    } else if (sendable) {
                        onSend(text.trim(), photos.mapNotNull { it.path })
                        text = ""
                        photos.clear()
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            if (!ready) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = h.text3)
            } else {
                Icon(
                    painterResource(if (stop) R.drawable.ic_square else R.drawable.ic_arrow_up),
                    if (stop) "Stop" else "Send",
                    Modifier.size(16.dp),
                    tint = if (stop) h.text1 else if (!sendable) h.text3 else h.onAccent,
                )
            }
        }
    }
    }
}

@Composable
private fun Thumb(p: Photo, onRemove: () -> Unit) {
    val h = LocalHues.current
    Box(Modifier.size(60.dp)) {
        Image(
            p.thumb,
            null,
            Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).alpha(if (p.path == null) 0.5f else 1f),
            contentScale = ContentScale.Crop,
        )
        if (p.path == null && !p.failed) {
            CircularProgressIndicator(Modifier.align(Alignment.Center).size(18.dp), strokeWidth = 2.dp, color = h.text1)
        }
        if (p.failed) {
            Box(Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).background(h.error.copy(alpha = 0.35f)))
        }
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .padding(3.dp)
                .size(20.dp)
                .clip(CircleShape)
                .background(h.bg.copy(alpha = 0.8f))
                .clickableQuiet(onRemove),
            contentAlignment = Alignment.Center,
        ) {
            Icon(painterResource(R.drawable.ic_x), "Remove", Modifier.size(11.dp), tint = h.text1)
        }
    }
}

@Composable
private fun Working(doing: String?, since: Long?) {
    val h = LocalHues.current
    val now = com.hyprspace.android.ui.ticking(since != null)
    Row(
        Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Eclipse(h.text3)
        Text(
            doing ?: "Working",
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium,
            color = h.text2,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        since?.let { Text(elapsed((now - it).coerceAtLeast(0) / 1000), fontFamily = Mono, fontSize = 11.sp, color = h.text3) }
    }
}
