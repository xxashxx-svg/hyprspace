// A structured thread on the phone: the conversation with its tool calls folded into one line
// per run of calls, subagents, approvals, questions and plans to answer, the task list, what runs
// in the background and what's queued, and a box to reply, queue or stop.

package com.hyprspace.android.ui.thread

import android.os.SystemClock
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.R
import com.hyprspace.android.model.AgentState
import com.hyprspace.android.model.Item
import com.hyprspace.android.model.PLAN
import com.hyprspace.android.model.QUESTION
import com.hyprspace.android.model.TODO
import com.hyprspace.android.model.counts
import com.hyprspace.android.model.doing as doingNow
import com.hyprspace.android.model.folds
import com.hyprspace.android.model.input
import com.hyprspace.android.model.label
import com.hyprspace.android.model.special
import com.hyprspace.android.model.summary
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.AsideView
import com.hyprspace.android.net.Slash
import com.hyprspace.android.net.BgTask
import com.hyprspace.android.net.Prompt
import com.hyprspace.android.net.RunStatus
import com.hyprspace.android.net.Tool
import com.hyprspace.android.net.TranscriptView
import com.hyprspace.android.ui.Eclipse
import com.hyprspace.android.ui.Feel
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.elapsed
import com.hyprspace.android.ui.feel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.max
import kotlin.math.roundToInt

/** What the list shows: the transcript's items, with runs of calls in a row folded together. */
private sealed interface Shown {
    val key: String
    data class One(val item: Item) : Shown { override val key = item.key }
    data class Calls(val calls: List<Item.Call>) : Shown { override val key = "calls" + calls.first().key }
    data class Todo(val call: Item.Call) : Shown { override val key = call.key }
    data class Folded(val prompt: String, val ms: Long?, val open: Boolean) : Shown { override val key = "fold$prompt" }
}

/** A question or plan call whose approval in the same exchange is its card already. */
private fun carded(items: List<Item>, ix: Int, name: String): Boolean {
    val from = items.subList(0, ix).indexOfLast { it is Item.User } + 1
    val next = items.subList(ix, items.size).indexOfFirst { it is Item.User }
    val to = if (next < 0) items.size else ix + next
    return items.subList(from, to).any { it is Item.Approval && special(it.tool) == name }
}

private fun fold(items: List<Item>, unfolded: Set<String>): List<Shown> {
    val out = ArrayList<Shown>()
    val folds = folds(items)
    val heads = folds.associateBy { it.first }
    val hidden = folds.flatMap { f -> if (f.prompt in unfolded) listOf(f.finish) else f.hide + f.finish }.toSet()
    val lastTodo = items.indexOfLast { it is Item.Call && special(it.tool) == TODO }
    var run = ArrayList<Item.Call>()
    fun flush() {
        when (run.size) {
            0 -> {}
            1 -> out += Shown.One(run[0])
            else -> out += Shown.Calls(run)
        }
        run = ArrayList()
    }
    for ((ix, i) in items.withIndex()) {
        val name = (i as? Item.Call)?.let { special(it.tool) }
        heads[i.key]?.let {
            flush()
            out += Shown.Folded(it.prompt, it.ms, it.prompt in unfolded)
        }
        when {
            i.key in hidden -> {}
            name == TODO -> {
                flush()
                if (ix == lastTodo) out += Shown.Todo(i)
            }
            name != null && carded(items, ix, name) -> {}
            i is Item.Call -> run += i
            else -> {
                flush()
                out += Shown.One(i)
            }
        }
    }
    flush()
    return out
}

/** The replies that end a run, and how each run ended: they get a footer with copy and its time. */
private fun ends(items: List<Item>): Map<String, Item.Finished> = items.indices
    .filter { items[it] is Item.Reply && items.getOrNull(it + 1) is Item.Finished }
    .associate { items[it].key to items[it + 1] as Item.Finished }

/** What the desktop does with a structured thread's queue and background, when it's new enough to. */
class Extras(
    val queue: List<Prompt>,
    val tasks: List<BgTask>,
    val onSteer: (Int, String) -> Unit,
    val onUnqueue: (Int, String) -> Unit,
    val onStopTask: (String) -> Unit,
    val redirects: Boolean = false,
)

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
    onAnswer: Answers,
    attach: (@Composable ((Attachment) -> Unit) -> (() -> Unit))?,
    modifier: Modifier = Modifier,
    model: (@Composable () -> Unit)? = null,
    extras: Extras? = null,
    ultra: Boolean = false,
    lines: Int = 0,
    onEarlier: (() -> Unit)? = null,
    aside: AsideView? = null,
    onDropAside: () -> Unit = {},
    commands: List<Slash> = emptyList(),
    onFork: ((Int, String) -> Unit)? = null,
) {
    val h = LocalHues.current
    val host = LocalView.current
    var unfolded by remember { mutableStateOf(setOf<String>()) }
    val shown = remember(view.items, unfolded) { fold(view.items, unfolded) }
    val ending = remember(view.items) { ends(view.items) }
    // a run that ended well shows its time under the reply instead of on a line of its own
    val footed = remember(ending) { ending.values.filter { it.status == RunStatus.Done }.map { it.key }.toSet() }
    // the prompt each ending reply answers, which a fork keeps along with everything before it
    val owner = remember(view.items) {
        var at: Item.User? = null
        view.items.mapNotNull { i ->
            if (i is Item.User) at = i
            at?.takeIf { i is Item.Reply && it.at > 0 }?.let { i.key to it }
        }.toMap()
    }
    var stopping by remember { mutableStateOf(false) }
    LaunchedEffect(working) { if (!working) stopping = false }
    var draft by rememberSaveable { mutableStateOf("") }
    // laid out from the bottom, so a growing reply stays pinned to the box without scrolling
    val list = rememberLazyListState()
    var stuck by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(list) {
        snapshotFlow { list.isScrollInProgress }.collect { moving ->
            if (!moving) stuck = list.firstVisibleItemIndex == 0 && list.firstVisibleItemScrollOffset < 24
        }
    }
    // let go of the bottom on touch, or a message landing as the finger lifts pulls the list back
    LaunchedEffect(list) {
        list.interactionSource.interactions.collect { if (it is DragInteraction.Start) stuck = false }
    }
    // what was sent or answered here shows at once, until the journal or the queue brings it back
    var pending by remember { mutableStateOf(listOf<Pending>()) }
    var answered by remember { mutableStateOf(mapOf<String, Answer>()) }
    LaunchedEffect(view.items, extras?.queue) {
        val users = view.items.filterIsInstance<Item.User>()
        val used = mutableSetOf<String>()
        pending = pending.filter { p ->
            val hit = users.firstOrNull { it.at > p.after && it.key !in used && it.text == p.text }
            if (hit != null) used += hit.key
            hit == null && !(p.queued && extras?.queue?.any { it.text == p.text } == true)
        }
    }
    // a line that dropped may have taken a message with it: once the new line has brought the
    // transcript and the queue back, what still isn't there goes again
    LaunchedEffect(lines) {
        delay(3000)
        val stale = pending.filter { it.line < lines }
        if (stale.isEmpty()) return@LaunchedEffect
        pending = pending.map { if (it.line < lines) it.copy(id = System.nanoTime(), line = lines) else it }
        for (p in stale) onSend(p.text, p.images)
    }
    // a message the computer never put in its queue comes back to the box rather than vanish
    LaunchedEffect(pending) {
        while (pending.any { it.queued }) {
            delay(1000)
            val now = System.nanoTime()
            val lost = pending.filter { it.queued && now - it.id > 30_000_000_000L }
            if (lost.isNotEmpty()) {
                pending = pending - lost.toSet()
                draft = (lost.map { it.text } + draft).filter { it.isNotBlank() }.joinToString("\n\n")
            }
        }
    }
    val bubbles = pending.filter { !it.queued }
    val newest = if (busy) "working" else bubbles.lastOrNull()?.let { "pending${it.id}" } ?: shown.lastOrNull()?.key
    LaunchedEffect(newest) {
        if (stuck && newest != null) list.animateScrollToItem(0)
    }
    val live = if (working) (shown.lastOrNull() as? Shown.One)?.item?.takeIf { it is Item.Reply }?.key else null
    Felt(view, busy, canAnswer)
    val denies = extras?.redirects == true && view.items.any {
        it is Item.Approval && it.answer == null && !it.expired && it.request !in answered &&
            special(it.tool).let { s -> s != QUESTION && s != PLAN }
    }
    val send: (String, List<String>) -> Unit = { text, images ->
        // while it works, the computer queues the message for when the run ends
        val queued = working && extras != null && !denies
        // a command may never become a message, so it shows once the computer says what it did
        if (text.isNotBlank() && !text.trimStart().startsWith("/")) {
            val after = view.prompts
            pending = pending + Pending(System.nanoTime(), text, after, queued, images, lines)
        }
        if (!queued) {
            stuck = true
            scope.launch { list.animateScrollToItem(0) }
        }
        onSend(text, images)
    }
    val answer: Answers = { request, a, given ->
        answered = answered + (request to a)
        onAnswer(request, a, given)
    }
    Column(modifier) {
        if (!view.loaded) {
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = h.text3)
            }
        } else {
            Box(Modifier.weight(1f).fillMaxWidth()) {
            LazyColumn(
                Modifier.fillMaxSize().clipToBounds(),
                state = list,
                reverseLayout = true,
                contentPadding = PaddingValues(horizontal = 20.dp, vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
                overscrollEffect = null,
            ) {
                if (busy) {
                    item(key = "working") {
                        val label = doingNow(waiting = false, stopping = stopping, compacting = doing == "Compacting")
                        Box(rise()) { Working(label, since) }
                    }
                }
                items(bubbles.asReversed(), key = { "pending${it.id}" }) { p ->
                    Box(rise()) {
                        One(Item.User("pending", p.text, 0, steer = working), false, answered, answer, live = false, end = null)
                    }
                }
                items(shown.filterNot { it is Shown.One && it.item.key in footed }.asReversed(), key = { it.key }) { s ->
                    Box(rise()) {
                        when (s) {
                            is Shown.Calls -> CallRun(s.calls)
                            is Shown.Todo -> TodoCard(s.call.tool)
                            is Shown.Folded -> FoldHead(s.ms?.let { "Worked for ${took(it)}" } ?: "Continued", s.open) {
                                unfolded = if (s.open) unfolded - s.prompt else unfolded + s.prompt
                            }
                            is Shown.One -> One(
                                s.item,
                                canAnswer,
                                answered,
                                answer,
                                live = s.key == live,
                                end = ending[s.key],
                                fork = onFork?.let { f -> owner[s.key]?.takeIf { !working }?.let { u -> { f(u.at, u.text) } } },
                            )
                        }
                    }
                }
                if (view.earlier && onEarlier != null) {
                    item(key = "earlier") { Earlier(shown.lastOrNull()?.key, onEarlier) }
                }
                if (shown.isEmpty() && bubbles.isEmpty()) {
                    item(key = "empty") { Text("Nothing here yet. Say what to do below.", color = h.text3, style = MaterialTheme.typography.bodyMedium) }
                }
            }
            val away by remember { derivedStateOf { list.firstVisibleItemIndex > 0 || list.firstVisibleItemScrollOffset > 48 } }
            androidx.compose.animation.AnimatedVisibility(
                away,
                Modifier.align(Alignment.BottomCenter).padding(bottom = 10.dp),
                enter = fadeIn() + scaleIn(initialScale = 0.8f),
                exit = fadeOut() + scaleOut(targetScale = 0.8f),
            ) {
                Box(
                    Modifier
                        .size(38.dp)
                        .clip(CircleShape)
                        .background(h.surface2)
                        .border(1.dp, h.border1, CircleShape)
                        .clickableQuiet {
                            host.feel(Feel.Tick)
                            stuck = true
                            scope.launch { list.animateScrollToItem(0) }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(painterResource(R.drawable.ic_arrow_down), "Jump to the newest", Modifier.size(16.dp), tint = h.text2)
                }
            }
            }
        }
        if (aside != null) AsideCard(aside, onDropAside)
        if (extras != null) {
            val waiting = extras.queue.map { Waiting(it.text, it.images.size, sent = false) } +
                pending.filter { it.queued }.map { Waiting(it.text, 0, sent = true) }
            Tray(
                tasks = extras.tasks,
                items = view.items,
                queue = waiting,
                canAct = canAnswer,
                onStopTask = extras.onStopTask,
                onSendNow = extras.onSteer,
                onEdit = { i, text ->
                    extras.onUnqueue(i, text)
                    draft = if (draft.isBlank()) text else "$text\n\n$draft"
                },
                onRemove = extras.onUnqueue,
            )
        }
        Composer(
            draft = draft,
            onDraft = { draft = it },
            working = working,
            queues = extras != null,
            denies = denies,
            ultra = ultra,
            onSend = send,
            onStop = {
                stopping = true
                onStop()
            },
            attach = attach,
            model = model,
            commands = commands,
        )
    }
}

@Composable
private fun One(
    item: Item,
    canAnswer: Boolean,
    answered: Map<String, Answer>,
    onAnswer: Answers,
    live: Boolean,
    end: Item.Finished?,
    fork: (() -> Unit)? = null,
) {
    val h = LocalHues.current
    when (item) {
        is Item.User -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .clip(RoundedCornerShape(20.dp))
                    .background(h.ink(0.07f))
                    .padding(horizontal = 16.dp, vertical = 11.dp),
            ) {
                if (item.steer) Text("Sent while it worked", fontSize = 11.sp, color = h.text3)
                Text(item.text, style = MaterialTheme.typography.bodyLarge, color = h.text1)
                if (item.images > 0) {
                    Text(if (item.images == 1) "1 image" else "${item.images} images", fontSize = 11.sp, color = h.text3)
                }
            }
        }
        is Item.Reply -> Column {
            Markdown(revealed(item.key, item.text, live))
            if (end != null && !live) Foot(item.text, end, fork)
        }
        is Item.Thinking -> Fold("Thinking", item.text, mono = false)
        is Item.Call -> CallLine(item)
        is Item.Agent -> AgentCard(item)
        is Item.Approval -> {
            val a = item.copy(answer = item.answer ?: answered[item.request])
            when (special(a.tool)) {
                QUESTION -> QuestionCard(a, canAnswer, onAnswer)
                PLAN -> PlanCard(a, canAnswer, onAnswer)
                else -> ApprovalCard(a, canAnswer) { r, ans -> onAnswer(r, ans, emptyList()) }
            }
        }
        is Item.Error -> Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(h.error.copy(alpha = 0.08f)).padding(horizontal = 14.dp, vertical = 11.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(painterResource(R.drawable.ic_circle_alert), null, Modifier.size(16.dp).padding(top = 2.dp), tint = h.error)
            Text(item.message, style = MaterialTheme.typography.bodyMedium, color = h.text1)
        }
        is Item.Finished -> TurnEnd(item)
        is Item.Note -> Text(item.text, style = MaterialTheme.typography.bodySmall, color = h.text3)
    }
}

/** Asks the computer for the prompts before these, until the list starting at [first] changes. */
@Composable
private fun Earlier(first: String?, onEarlier: () -> Unit) {
    val h = LocalHues.current
    var asked by remember(first) { mutableStateOf(false) }
    val host = LocalView.current
    Box(Modifier.fillMaxWidth().padding(vertical = 4.dp), contentAlignment = Alignment.Center) {
        if (asked) {
            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = h.text3)
        } else {
            Text(
                "Show earlier messages",
                Modifier
                    .clip(RoundedCornerShape(10.dp))
                    .clickableQuiet {
                        host.feel(Feel.Tick)
                        asked = true
                        onEarlier()
                    }
                    .padding(horizontal = 12.dp, vertical = 6.dp),
                style = MaterialTheme.typography.labelLarge,
                color = h.text2,
            )
        }
    }
}

/** The commands that start with what's typed after `/`, above the box. */
@Composable
private fun Slashes(offered: List<Slash>, onPick: (Slash) -> Unit) {
    val h = LocalHues.current
    val host = LocalView.current
    Column(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp)
            .padding(top = 6.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(h.ink(0.045f))
            .heightIn(max = 260.dp)
            .verticalScroll(rememberScrollState())
            .padding(vertical = 4.dp),
    ) {
        for (s in offered) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .clickableQuiet { host.feel(Feel.Tick); onPick(s) }
                    .padding(horizontal = 14.dp, vertical = 8.dp),
            ) {
                Text("/${s.name}", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = h.text1)
                if (s.detail.isNotBlank()) {
                    Text(s.detail, style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/**
 * Ticks as a reply streams in, at most every [STREAM] ms, and buzzes when the open thread starts
 * waiting on you or a run ends, since a thread on screen gets no notification. The first look at
 * a thread only notes where it is, so opening one mid-stream is quiet.
 */
@Composable
private fun Felt(view: TranscriptView, busy: Boolean, canAnswer: Boolean) {
    val host = LocalView.current
    val seen = remember { Seen() }
    val items = view.items
    val reply = (items.lastOrNull() as? Item.Reply)?.takeIf { busy }
    val ask = items.lastOrNull { it is Item.Approval && it.answer == null && !it.expired }?.key
    val end = items.lastOrNull { it is Item.Finished } as Item.Finished?
    SideEffect {
        if (!view.loaded) return@SideEffect
        val now = SystemClock.uptimeMillis()
        if (seen.seeded) {
            when {
                ask != null && ask != seen.ask && canAnswer -> host.feel(Feel.Hold)
                end != null && end.key != seen.end && end.status != RunStatus.Interrupted ->
                    host.feel(if (end.status == RunStatus.Done) Feel.Done else Feel.Fail)
                reply != null && (reply.key != seen.reply || (reply.text.length > seen.length && now - seen.at >= STREAM)) -> {
                    seen.at = now
                    host.feel(Feel.Stream)
                }
            }
        }
        seen.seeded = true
        seen.reply = reply?.key
        seen.length = reply?.text?.length ?: 0
        seen.ask = ask
        seen.end = end?.key
    }
}

private class Seen {
    var seeded = false
    var reply: String? = null
    var length = 0
    var at = 0L
    var ask: String? = null
    var end: String? = null
}

private const val STREAM = 320L

/** Sent here and not yet back from the computer, on line [line] of [Link.lines]. */
private data class Pending(
    val id: Long,
    val text: String,
    /** How many prompts the transcript had when it was sent, which a fresh copy keeps. */
    val after: Int,
    val queued: Boolean = false,
    val images: List<String> = emptyList(),
    val line: Int = 0,
)

/** Under the reply that ends a run: copy it, and how long the run took when it finished well. */
@Composable
private fun Foot(text: String, end: Item.Finished, fork: (() -> Unit)?) {
    val h = LocalHues.current
    val host = LocalView.current
    Row(Modifier.padding(top = 6.dp).offset(x = (-8).dp), verticalAlignment = Alignment.CenterVertically) {
        Copy(text)
        if (fork != null) {
            Box(
                Modifier.size(36.dp).clip(CircleShape).clickableQuiet {
                    host.feel(Feel.Tap)
                    fork()
                },
                contentAlignment = Alignment.Center,
            ) {
                Icon(painterResource(R.drawable.ic_git_branch), "Fork from here", Modifier.size(16.dp), tint = h.text3)
            }
        }
        if (end.status == RunStatus.Done) {
            Text(clock(end.ms), Modifier.padding(start = 6.dp), style = MaterialTheme.typography.labelLarge, color = h.text3)
        }
    }
}

private fun clock(ms: Long): String {
    val s = ms / 1000
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s % 3600 / 60, s % 60) else "%02d:%02d".format(s / 60, s % 60)
}

@Composable
private fun Copy(text: String) {
    val h = LocalHues.current
    val clipboard = LocalContext.current.getSystemService(android.content.ClipboardManager::class.java)
    val host = LocalView.current
    var done by remember { mutableStateOf(false) }
    LaunchedEffect(done) {
        if (done) {
            delay(1500)
            done = false
        }
    }
    Box(
        Modifier
            .size(36.dp)
            .clip(CircleShape)
            .clickableQuiet {
                clipboard?.setPrimaryClip(android.content.ClipData.newPlainText("Reply", text))
                host.feel(Feel.Tap)
                done = true
            },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painterResource(if (done) R.drawable.ic_check else R.drawable.ic_copy),
            if (done) "Copied" else "Copy",
            Modifier.size(16.dp),
            tint = h.text3,
        )
    }
}

// no placement spring: a reply growing at the bottom would leave the rows above trailing over it
private fun LazyItemScope.rise(): Modifier = Modifier.animateItem(
    fadeInSpec = tween(220),
    fadeOutSpec = null,
    placementSpec = null,
)

@Composable
private fun revealed(key: String, text: String, live: Boolean): String {
    var shown by rememberSaveable(key) { mutableIntStateOf(if (live) 0 else text.length) }
    val target by rememberUpdatedState(text)
    LaunchedEffect(key) {
        snapshotFlow { target.length }.collect {
            var prev = 0L
            while (shown < target.length) {
                withFrameNanos { now ->
                    val dt = if (prev == 0L) 0.016f else ((now - prev) / 1e9f).coerceAtMost(0.05f)
                    prev = now
                    val left = target.length - shown
                    // text that piles up runs out faster, so the reveal never trails for long
                    val step = max(1, (max(120f, left / 0.2f) * dt).roundToInt())
                    var end = (shown + step).coerceAtMost(target.length)
                    if (end < target.length && Character.isHighSurrogate(target[end - 1])) end++
                    shown = end
                }
            }
        }
    }
    return if (shown >= text.length) text else text.substring(0, shown)
}

@Composable
private fun Opens(open: Boolean, content: @Composable () -> Unit) {
    AnimatedVisibility(
        open,
        enter = fadeIn(tween(180)) + expandVertically(spring(stiffness = Spring.StiffnessMediumLow)),
        exit = fadeOut(tween(120)) + shrinkVertically(spring(stiffness = Spring.StiffnessMedium)),
    ) { content() }
}

@Composable
private fun Twist(open: Boolean, size: Int = 14) {
    val h = LocalHues.current
    val turn by animateFloatAsState(if (open) 90f else 0f, spring(stiffness = Spring.StiffnessMedium), label = "twist")
    Icon(painterResource(R.drawable.ic_chevron_right), null, Modifier.size(size.dp).rotate(turn), tint = h.text3)
}

private fun took(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(1)
    return when {
        s < 60 -> "${s}s"
        s < 3600 -> if (s % 60 == 0L) "${s / 60}m" else "${s / 60}m ${s % 60}s"
        else -> if (s % 3600 / 60 == 0L) "${s / 3600}h" else "${s / 3600}h ${s % 3600 / 60}m"
    }
}

@Composable
private fun TurnEnd(item: Item.Finished) {
    val h = LocalHues.current
    val failed = item.status == RunStatus.Failed
    val (icon, text) = when (item.status) {
        RunStatus.Done -> R.drawable.ic_check to "Worked for ${took(item.ms)}"
        RunStatus.Interrupted -> R.drawable.ic_square to "Stopped after ${took(item.ms)}"
        RunStatus.Failed -> R.drawable.ic_circle_alert to "Failed"
    }
    val tint = if (failed) h.error else h.text3
    Column(Modifier.fillMaxWidth().padding(vertical = 2.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            HorizontalDivider(Modifier.weight(1f), color = h.border0)
            Row(
                Modifier.padding(horizontal = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Icon(painterResource(icon), null, Modifier.size(if (icon == R.drawable.ic_square) 9.dp else 12.dp), tint = tint)
                Text(text, style = MaterialTheme.typography.labelMedium, color = tint)
            }
            HorizontalDivider(Modifier.weight(1f), color = h.border0)
        }
        if (failed) {
            item.error?.takeIf { it.isNotBlank() }?.let {
                Text(it, Modifier.padding(top = 6.dp), style = MaterialTheme.typography.bodySmall, color = h.text2, textAlign = TextAlign.Center)
            }
        }
    }
}

/** A muted line led by a chevron that opens to show more. */
@Composable
private fun FoldHead(label: String, open: Boolean, trailing: @Composable () -> Unit = {}, onClick: () -> Unit) {
    val h = LocalHues.current
    val host = LocalView.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet { host.feel(Feel.Tick); onClick() }.padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Twist(open)
        Text(label, Modifier.weight(1f, fill = false), style = MaterialTheme.typography.bodyMedium, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
        trailing()
    }
}

@Composable
private fun Fold(title: String, body: String, mono: Boolean) {
    val h = LocalHues.current
    var open by rememberSaveable { mutableStateOf(false) }
    Column {
        FoldHead(title, open) { open = !open }
        Opens(open) {
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
    Column(Modifier.fillMaxWidth()) {
        FoldHead(summary(calls.map { it.tool }), open, trailing = {
            if (failed > 0) Text("$failed failed", style = MaterialTheme.typography.bodySmall, color = h.text3)
            if (live) Eclipse(h.text3)
        }) { open = !open }
        Opens(open) {
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
    Column(Modifier.fillMaxWidth()) {
        FoldHead(label(tool), open, trailing = {
            if (tool is Tool.Edit) {
                val (add, del) = counts(tool.changes)
                Text("+$add", fontFamily = Mono, fontSize = 11.sp, color = h.diffAdd)
                Text("-$del", fontFamily = Mono, fontSize = 11.sp, color = h.diffDel)
            }
            if (failed) Text("Failed", style = MaterialTheme.typography.bodySmall, color = if (tool is Tool.Edit) h.error else h.text3)
            if (c.done == null) Eclipse(h.text3)
        }) { open = !open }
        Opens(open) {
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
    val host = LocalView.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.035f)),
    ) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { host.feel(Feel.Tick); open = !open }.padding(start = 12.dp, end = 14.dp, top = 11.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Twist(open)
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
        Opens(open) {
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
    if (a.answer == null && !a.expired) PendingApproval(a, canAnswer, onAnswer) else SettledApproval(a)
}

@Composable
private fun PendingApproval(a: Item.Approval, canAnswer: Boolean, onAnswer: (String, Answer) -> Unit) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(h.ink(0.035f))
            .border(1.dp, h.border1, RoundedCornerShape(14.dp))
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(painterResource(R.drawable.ic_hand), null, Modifier.size(13.dp), tint = h.waiting)
                Text("Needs your OK", style = MaterialTheme.typography.labelMedium, color = h.waiting)
            }
            Text(label(a.tool), style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, color = h.text1, maxLines = 3, overflow = TextOverflow.Ellipsis)
            a.reason?.takeIf { it.isNotBlank() }?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = h.text2) }
        }
        ApprovalDetail(a.tool)
        if (canAnswer) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Choice("Deny", Modifier.weight(1f)) { onAnswer(a.request, Answer.Deny) }
                Choice("Allow", Modifier.weight(1f), primary = true) { onAnswer(a.request, Answer.Allow) }
            }
            if (a.always) {
                Text(
                    "Always allow this session",
                    Modifier
                        .align(Alignment.CenterHorizontally)
                        .clip(RoundedCornerShape(8.dp))
                        .clickableQuiet { onAnswer(a.request, Answer.AllowAlways) }
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                    style = MaterialTheme.typography.labelLarge,
                    color = h.text2,
                )
            }
        } else {
            Text("Answer it on the computer.", style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
    }
}

@Composable
private fun SettledApproval(a: Item.Approval) {
    val h = LocalHues.current
    var open by rememberSaveable(a.request) { mutableStateOf(false) }
    val host = LocalView.current
    val (mark, tint, state) = when (a.answer) {
        Answer.Allow -> Triple(R.drawable.ic_check, h.ok, "Allowed")
        Answer.AllowAlways -> Triple(R.drawable.ic_check, h.ok, "Allowed for this session")
        Answer.Deny -> Triple(R.drawable.ic_x, h.error, "Denied")
        null -> Triple(R.drawable.ic_clock, h.text3, "Not answered")
    }
    Column(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth().clickableQuiet { host.feel(Feel.Tick); open = !open }.padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(painterResource(mark), null, Modifier.size(14.dp), tint = tint)
            Text(label(a.tool), Modifier.weight(1f, fill = false), style = MaterialTheme.typography.bodyMedium, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(state, style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
        Opens(open) { Box(Modifier.padding(start = 22.dp, top = 4.dp, bottom = 4.dp)) { ApprovalDetail(a.tool) } }
    }
}

@Composable
private fun ApprovalDetail(tool: Tool) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        input(tool)?.let { Mono(it, maxLines = 12) }
        if (tool is Tool.Edit) for (ch in tool.changes) Diff(ch.path, ch.diff)
    }
}

@Composable
internal fun Choice(text: String, modifier: Modifier = Modifier, primary: Boolean = false, enabled: Boolean = true, onClick: () -> Unit) {
    val h = LocalHues.current
    Box(
        modifier
            .height(44.dp)
            .alpha(if (enabled) 1f else 0.5f)
            .clip(RoundedCornerShape(12.dp))
            .background(if (primary) h.accent else h.ink(0.08f))
            .then(if (enabled) Modifier.clickableQuiet(onClick) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = MaterialTheme.typography.labelLarge, color = if (primary) h.onAccent else h.text1)
    }
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun Composer(
    draft: String,
    onDraft: (String) -> Unit,
    working: Boolean,
    queues: Boolean,
    denies: Boolean,
    ultra: Boolean,
    onSend: (String, List<String>) -> Unit,
    onStop: () -> Unit,
    attach: (@Composable ((Attachment) -> Unit) -> (() -> Unit))?,
    model: (@Composable () -> Unit)?,
    commands: List<Slash>,
) {
    val h = LocalHues.current
    val text = draft
    // text put in from outside (a picked command, an edited queue item) leaves the cursor at its end
    var field by remember { mutableStateOf(TextFieldValue(text, TextRange(text.length))) }
    val shown = if (field.text == text) field else TextFieldValue(text, TextRange(text.length))
    val typed = text.takeIf { it.startsWith("/") && it.none(Char::isWhitespace) }?.drop(1)?.lowercase()
    val offered = typed?.let { t -> commands.filter { it.name.lowercase().startsWith(t) }.take(6) }.orEmpty()
    if (offered.isNotEmpty()) Slashes(offered) { onDraft("/${it.name} ") }
    val glow = ultra && saysUltrathink(text)
    val photos = remember { mutableStateListOf<Attachment>() }
    val pick = attach?.invoke { p ->
        val i = photos.indexOfFirst { it.key == p.key }
        if (i >= 0) photos[i] = p else photos.add(p)
    }
    val ready = photos.none { it.path == null && !it.failed }
    val sendable = (text.isNotBlank() || photos.any { it.path != null }) && ready
    var focused by remember { mutableStateOf(false) }
    // closing the keyboard keeps the field focused, which would hold the box open
    val keyboard = WindowInsets.isImeVisible
    val focusManager = LocalFocusManager.current
    LaunchedEffect(keyboard) { if (!keyboard && focused) focusManager.clearFocus() }
    // a pill with the box, the clip and send while idle; typing opens it up with its toolbar
    val open = focused || text.isNotEmpty() || photos.isNotEmpty()
    val stop = working && text.isBlank() && photos.isEmpty()
    val send = @Composable {
        Box(
            Modifier
                .size(40.dp)
                .clip(CircleShape)
                .background(if (stop) h.surface3 else if (!sendable) h.ink(0.08f) else h.accent)
                .clickableQuiet {
                    if (stop) {
                        onStop()
                    } else if (sendable) {
                        onSend(text.trim(), photos.mapNotNull { it.path })
                        onDraft("")
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
                    Modifier.size(if (stop) 14.dp else 18.dp),
                    tint = if (stop) h.text1 else if (!sendable) h.text3 else h.onAccent,
                )
            }
        }
    }
    val plus = @Composable {
        if (pick != null) {
            Box(Modifier.size(40.dp).clip(CircleShape).clickableQuiet(pick), contentAlignment = Alignment.Center) {
                Icon(painterResource(R.drawable.ic_plus), "Attach", Modifier.size(22.dp), tint = h.text2)
            }
        }
    }
    Column(
        Modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, top = 6.dp, bottom = 10.dp)
            .ultraRing(glow, h.dark, 26.dp)
            .clip(RoundedCornerShape(26.dp))
            .background(h.ink(if (h.dark) 0.075f else 0.05f))
            .animateContentSize(spring(stiffness = Spring.StiffnessMediumLow))
            .padding(6.dp),
    ) {
        if (photos.isNotEmpty()) {
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(start = 8.dp, end = 8.dp, top = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                for (p in photos) AttachmentThumb(p) { photos.remove(p) }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (!open) plus()
            Box(Modifier.weight(1f).padding(horizontal = if (open) 12.dp else 6.dp, vertical = if (open) 10.dp else 8.dp)) {
                if (text.isEmpty()) {
                    val hint = when {
                        !working -> "Ask anything, or / for commands"
                        denies -> "Send a message to deny and redirect the agent"
                        queues -> "Queue a message for when it finishes"
                        else -> "Steer it, or stop it"
                    }
                    Text(hint, style = MaterialTheme.typography.bodyLarge, color = h.text3, maxLines = if (open) 2 else 1, overflow = TextOverflow.Ellipsis)
                }
                BasicTextField(
                    value = shown,
                    onValueChange = {
                        field = it
                        onDraft(it.text)
                    },
                    modifier = Modifier.fillMaxWidth().heightIn(max = 180.dp).onFocusChanged { focused = it.isFocused },
                    textStyle = MaterialTheme.typography.bodyLarge.copy(color = h.text1),
                    cursorBrush = SolidColor(h.accent),
                    visualTransformation = if (ultra) UltraWords(h.dark) else androidx.compose.ui.text.input.VisualTransformation.None,
                )
            }
            if (!open) send()
        }
        if (open) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                plus()
                Spacer(Modifier.weight(1f))
                if (model != null) Box(Modifier.padding(end = 6.dp)) { model() }
                send()
            }
        }
    }
}

/** [target] once what shows has been up long enough to read, so a burst of calls doesn't flicker. */
@Composable
private fun held(target: String): String {
    var shown by remember { mutableStateOf(target) }
    var at by remember { mutableLongStateOf(0L) }
    LaunchedEffect(target) {
        val wait = 850 - (System.currentTimeMillis() - at)
        if (wait > 0 && target != "Stopping") delay(wait)
        shown = target
        at = System.currentTimeMillis()
    }
    return shown
}

@Composable
private fun Working(doing: String, since: Long?) {
    val h = LocalHues.current
    val now = com.hyprspace.android.ui.ticking(since != null)
    val pulse by rememberInfiniteTransition(label = "working").animateFloat(
        0.55f, 1f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "working",
    )
    Row(
        Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Eclipse(h.text2)
        AnimatedContent(
            held(doing),
            Modifier.weight(1f).clipToBounds(),
            transitionSpec = {
                (slideInVertically { it } + fadeIn(tween(200))) togetherWith (slideOutVertically { -it } + fadeOut(tween(150)))
            },
            label = "doing",
        ) { label ->
            Text(
                label,
                Modifier.graphicsLayer { alpha = pulse },
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                color = h.text2,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        since?.let { Text(elapsed((now - it).coerceAtLeast(0) / 1000), fontFamily = Mono, fontSize = 11.sp, color = h.text3) }
    }
}
