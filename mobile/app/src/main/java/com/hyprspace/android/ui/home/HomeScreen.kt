// The phone's sidebar: each space and its threads with what they are doing, the shelves of
// snoozed and settled threads, and a button to start one.

package com.hyprspace.android.ui.home

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.BoardKind
import com.hyprspace.android.net.BoardSpace
import com.hyprspace.android.net.BoardStatus
import com.hyprspace.android.net.BoardThread
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.Shelf
import com.hyprspace.android.net.SnoozeUntil
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.ConnStrip
import com.hyprspace.android.ui.UpdateStrip
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.Pill
import com.hyprspace.android.ui.SpaceTag
import com.hyprspace.android.ui.StatusMark
import com.hyprspace.android.ui.ago
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.elapsed
import com.hyprspace.android.ui.start.NewThreadSheet
import com.hyprspace.android.ui.ticking
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@Composable
fun HomeScreen(app: App, onOpen: (Long) -> Unit, onSettings: () -> Unit, onPair: () -> Unit) {
    val h = LocalHues.current
    val conn by app.link.conn.collectAsStateWithLifecycle()
    val board by app.link.board.collectAsStateWithLifecycle()
    val saved by app.store.saved.collectAsStateWithLifecycle()
    var starting by remember { mutableStateOf<Long?>(null) }
    var computers by remember { mutableStateOf(false) }
    val snack = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val undo: (String, () -> Unit) -> Unit = { text, back ->
        scope.launch {
            snack.currentSnackbarData?.dismiss()
            if (snack.showSnackbar(text, "Undo", duration = SnackbarDuration.Long) == SnackbarResult.ActionPerformed) back()
        }
    }
    // a thread just asked for: the threads there were before, to spot the new one
    var before by remember { mutableStateOf<Set<Long>?>(null) }
    LaunchedEffect(board) {
        val known = before ?: return@LaunchedEffect
        val made = board?.threads?.firstOrNull { it.id !in known } ?: return@LaunchedEffect
        before = null
        onOpen(made.id)
    }
    val name = (conn as? Conn.Online)?.desktop ?: saved.current()?.name ?: "HyprSpace"

    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Row(
            Modifier.fillMaxWidth().padding(start = 18.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).clip(RoundedCornerShape(10.dp)).clickableQuiet { computers = true }) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Crossfade(name, Modifier.weight(1f, fill = false), label = "computer") { n ->
                        Text(n, style = MaterialTheme.typography.titleLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    Icon(painterResource(R.drawable.ic_chevron_down), "Switch computer", Modifier.padding(start = 6.dp).size(18.dp), tint = h.text3)
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    val (dot, label) = when (conn) {
                        is Conn.Online -> h.ok to "Connected"
                        is Conn.Denied -> h.error to "Not connected"
                        else -> h.busy to "Connecting"
                    }
                    Box(Modifier.size(7.dp).clip(RoundedCornerShape(50)).background(dot))
                    Text(label, style = MaterialTheme.typography.bodySmall, color = h.text3)
                }
            }
            IconButton(onClick = onSettings) {
                Icon(painterResource(R.drawable.ic_settings), "Settings", Modifier.size(20.dp), tint = h.text2)
            }
        }
        ConnStrip(conn) { app.link.start(); app.link.nudge() }
        val update by app.updater.state.collectAsStateWithLifecycle()
        UpdateStrip(update) { app.updater.install() }
        Crossfade(board == null, Modifier.weight(1f), label = "board") { waiting ->
            val b = board
            if (waiting || b == null) {
                Waiting(conn)
            } else {
                Threads(
                    b,
                    onOpen = onOpen,
                    onSettle = { t, on -> app.link.ask(Ask.Settle(t, on)) },
                    onSnooze = if (app.link.desktopAtLeast(SNOOZE)) { t, until -> app.link.ask(Ask.Snooze(t, until)) } else null,
                    undo = undo,
                )
            }
        }
    }

    SnackbarHost(
        snack,
        Modifier.fillMaxSize().navigationBarsPadding().padding(bottom = 84.dp),
    ) { data ->
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
            Snackbar(
                data,
                Modifier.padding(horizontal = 16.dp),
                shape = RoundedCornerShape(12.dp),
                containerColor = h.surface3,
                contentColor = h.text1,
                actionColor = h.accent,
            )
        }
    }

    if (computers) {
        ComputersSheet(app, onPair = onPair, onDismiss = { computers = false })
    }

    if (board?.spaces?.isNotEmpty() == true) {
        Box(Modifier.fillMaxSize().navigationBarsPadding().padding(20.dp), contentAlignment = Alignment.BottomEnd) {
            ExtendedFloatingActionButton(
                // the space of the thread on top, where work last started
                onClick = {
                    val b = board
                    starting = b?.threads?.filter { it.shelf == Shelf.Active }?.maxByOrNull { it.rank }?.space
                        ?: b?.spaces?.firstOrNull()?.id
                },
                containerColor = h.accent,
                contentColor = h.onAccent,
                shape = RoundedCornerShape(14.dp),
                icon = { Icon(painterResource(R.drawable.ic_plus), null, Modifier.size(18.dp)) },
                text = { Text("New thread") },
            )
        }
    }

    starting?.let { space ->
        board?.let { b ->
            NewThreadSheet(
                app,
                b,
                space,
                onDismiss = { starting = null },
                onStarted = { before = b.threads.map { it.id }.toSet() },
            )
        }
    }
}

@Composable
private fun Waiting(conn: Conn) {
    val h = LocalHues.current
    Column(
        Modifier.fillMaxSize().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        if (conn !is Conn.Denied) CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp, color = h.text3)
        Spacer(Modifier.height(16.dp))
        Text(
            when (conn) {
                is Conn.Denied -> "Pair this phone again from Settings."
                is Conn.Offline -> "Open HyprSpace on your computer and switch on Settings, Phone. The app keeps trying."
                else -> "Getting your threads."
            },
            style = MaterialTheme.typography.bodyMedium,
            color = h.text2,
        )
    }
}

private const val SNOOZE = "0.24.12"

private sealed interface Row {
    val key: String
    data class Thread(val thread: BoardThread, val space: BoardSpace?, val shelved: Boolean) : Row { override val key = "t${thread.id}" }
    data class Shelf(val name: String, val count: Int, val open: Boolean) : Row { override val key = "shelf$name" }
    data object Empty : Row { override val key = "empty" }
}

/**
 * The desktop sidebar's rows: every thread at work in its place (new ones on top, none moved by
 * activity), then the Snoozed and Settled shelves, most recently active first.
 */
@Composable
private fun Threads(
    b: Board,
    onOpen: (Long) -> Unit,
    onSettle: (Long, Boolean) -> Boolean,
    onSnooze: ((Long, SnoozeUntil) -> Boolean)?,
    undo: (String, () -> Unit) -> Unit,
) {
    val h = LocalHues.current
    var snoozedOpen by rememberSaveable { mutableStateOf(false) }
    var settledOpen by rememberSaveable { mutableStateOf(false) }
    // swiped away, until the computer's board catches up
    val gone = remember { mutableStateMapOf<Long, Shelf>() }
    val resets = remember { mutableStateMapOf<Long, Int>() }
    var snoozing by remember { mutableStateOf<BoardThread?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(b) {
        val now = b.threads.associate { it.id to it.shelf }
        gone.keys.filter { now[it] != gone[it] }.forEach { gone.remove(it) }
    }
    fun hide(t: BoardThread) {
        gone[t.id] = t.shelf
        scope.launch {
            delay(4_000)
            gone.remove(t.id)
        }
    }
    val spaces = b.spaces.associateBy { it.id }
    val shown = b.threads.filter { it.id !in gone }
    val rows = buildList {
        val active = shown.filter { it.shelf == Shelf.Active }.sortedByDescending { it.rank }
        if (active.isEmpty()) add(Row.Empty)
        active.forEach { add(Row.Thread(it, spaces[it.space], false)) }
        val snoozed = shown.filter { it.shelf == Shelf.Snoozed }.sortedByDescending { it.touched }
        if (snoozed.isNotEmpty()) {
            add(Row.Shelf("Snoozed", snoozed.size, snoozedOpen))
            if (snoozedOpen) snoozed.forEach { add(Row.Thread(it, spaces[it.space], true)) }
        }
        val settled = shown.filter { it.shelf == Shelf.Settled }.sortedByDescending { it.touched }
        if (settled.isNotEmpty()) {
            add(Row.Shelf("Settled", settled.size, settledOpen))
            if (settledOpen) settled.forEach { add(Row.Thread(it, spaces[it.space], true)) }
        }
    }
    val now = ticking(b.threads.any { it.since != null })
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = 4.dp, bottom = 110.dp)) {
        items(rows, key = { it.key }) { r ->
            when (r) {
                is Row.Thread -> {
                    val t = r.thread
                    val right = if (r.shelved) {
                        Swipe("Bring back", R.drawable.ic_archive_restore, h.accent) {
                            if (onSettle(t.id, false)) hide(t) else resets[t.id] = (resets[t.id] ?: 0) + 1
                        }
                    } else {
                        Swipe("Settle", R.drawable.ic_archive, h.ok) {
                            if (onSettle(t.id, true)) {
                                hide(t)
                                undo("Settled") { onSettle(t.id, false) }
                            } else {
                                resets[t.id] = (resets[t.id] ?: 0) + 1
                            }
                        }
                    }
                    val left = if (!r.shelved && onSnooze != null) {
                        Swipe("Snooze", R.drawable.ic_clock, h.busy) { snoozing = t }
                    } else {
                        null
                    }
                    SwipeRow(left, right, resets[t.id] ?: 0, Modifier.animateItem()) {
                        ThreadRow(t, r.space, r.shelved, now, onOpen, { id, on -> onSettle(id, on) })
                    }
                }
                is Row.Shelf -> Box(Modifier.animateItem()) {
                    ShelfHeader(r) {
                        if (r.name == "Snoozed") snoozedOpen = !snoozedOpen else settledOpen = !settledOpen
                    }
                }
                Row.Empty -> Text(
                    "No threads at work. Start one with New thread.",
                    Modifier.animateItem().padding(horizontal = 20.dp, vertical = 16.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = LocalHues.current.text3,
                )
            }
        }
    }

    snoozing?.let { t ->
        val working = t.status == BoardStatus.Working || t.status == BoardStatus.Waiting
        SnoozeSheet(
            working,
            onPick = { until, label ->
                snoozing = null
                if (onSnooze?.invoke(t.id, until) == true) {
                    hide(t)
                    undo("Snoozed until $label") { onSettle(t.id, false) }
                } else {
                    resets[t.id] = (resets[t.id] ?: 0) + 1
                }
            },
            onDismiss = {
                snoozing = null
                resets[t.id] = (resets[t.id] ?: 0) + 1
            },
        )
    }
}

@Composable
private fun ShelfHeader(r: Row.Shelf, onToggle: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onToggle).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("${r.name}  ${r.count}", style = MaterialTheme.typography.labelLarge, color = h.text3)
        Spacer(Modifier.width(10.dp))
        HorizontalDivider(Modifier.weight(1f), color = h.border1)
        Spacer(Modifier.width(10.dp))
        Icon(
            painterResource(if (r.open) R.drawable.ic_chevron_up else R.drawable.ic_chevron_down),
            null,
            Modifier.size(16.dp),
            tint = h.text3,
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ThreadRow(
    t: BoardThread,
    space: BoardSpace?,
    shelved: Boolean,
    now: Long,
    onOpen: (Long) -> Unit,
    onSettle: (Long, Boolean) -> Unit,
) {
    val h = LocalHues.current
    var menu by remember { mutableStateOf(false) }
    val waiting = t.status == BoardStatus.Waiting
    // background work takes less attention than a thread that needs you
    val recede = t.status == BoardStatus.Working
    Box {
        Column(
            Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 2.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(h.bg)
                .background(if (waiting) h.waiting.copy(alpha = 0.07f) else h.bg)
                .combinedClickable(onClick = { onOpen(t.id) }, onLongClick = { menu = true })
                .padding(horizontal = 12.dp, vertical = 10.dp)
                .alpha(if (shelved) 0.6f else 1f),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (space != null) {
                    SpaceTag(space, 16.dp)
                    Spacer(Modifier.width(7.dp))
                    Text(space.name, Modifier.weight(1f), style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
                } else {
                    Spacer(Modifier.weight(1f))
                }
                val since = t.since
                if (since != null) {
                    Text(elapsed((now - since) / 1000), fontFamily = Mono, fontSize = 11.sp, fontWeight = FontWeight.Medium, color = h.busy)
                } else if (t.touched > 0) {
                    Text(ago(t.touched, now), style = MaterialTheme.typography.bodySmall, color = h.text3)
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    t.title.ifBlank { "New thread" },
                    Modifier.weight(1f),
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (recede) FontWeight.Normal else FontWeight.Medium,
                    color = h.text1,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.width(8.dp))
                StatusMark(t.status, t.unseen)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                AgentMark(t.agent, 12.dp)
                val doing = t.doing?.takeIf { t.status != BoardStatus.Idle }
                if (doing != null) {
                    Text(
                        doing,
                        Modifier.weight(1f),
                        style = MaterialTheme.typography.bodySmall,
                        color = if (waiting) h.waiting else h.text2,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                } else {
                    Icon(
                        painterResource(if (t.branch) R.drawable.ic_git_branch else R.drawable.ic_folder),
                        null,
                        Modifier.size(11.dp),
                        tint = h.text3,
                    )
                    Text(
                        t.place,
                        Modifier.weight(1f),
                        fontFamily = Mono,
                        fontSize = 11.sp,
                        color = h.text3,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    t.model?.takeIf { t.agent != null }?.let { Pill(it) }
                }
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            if (t.shelf == Shelf.Active) {
                DropdownMenuItem(
                    text = { Text("Settle") },
                    leadingIcon = { Icon(painterResource(R.drawable.ic_archive), null, Modifier.size(16.dp)) },
                    onClick = { menu = false; onSettle(t.id, true) },
                )
            } else {
                DropdownMenuItem(
                    text = { Text("Bring back") },
                    leadingIcon = { Icon(painterResource(R.drawable.ic_archive_restore), null, Modifier.size(16.dp)) },
                    onClick = { menu = false; onSettle(t.id, false) },
                )
            }
        }
    }
}
