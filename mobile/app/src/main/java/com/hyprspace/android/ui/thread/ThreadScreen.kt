// One thread: its bar (back, agent, title, what it's doing) over the transcript or the terminal.
// Opening it tells the computer to stream it; leaving stops the stream and gives a terminal its
// desktop size back.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.BoardKind
import com.hyprspace.android.net.BoardStatus
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.Shelf
import com.hyprspace.android.net.Up
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.ConnStrip
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.StatusMark

@Composable
fun ThreadScreen(app: App, id: Long, onBack: () -> Unit) {
    val h = LocalHues.current
    val board by app.link.board.collectAsStateWithLifecycle()
    val conn by app.link.conn.collectAsStateWithLifecycle()
    val saved by app.store.saved.collectAsStateWithLifecycle()
    val t = board?.threads?.firstOrNull { it.id == id }
    val online = conn is Conn.Online

    // streamed only while it's on screen: a phone in a pocket shouldn't keep a terminal its size
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    // each return to the screen asks for the terminal's fit again, since leaving gave it back
    var visit by remember { mutableIntStateOf(0) }
    DisposableEffect(id, lifecycle) {
        fun show() {
            visit++
            app.link.watch(id)
            app.notifier.viewing = id
            app.notifier.cancel(id)
        }
        fun hide() {
            app.link.unwatch(id)
            if (app.notifier.viewing == id) app.notifier.viewing = null
        }
        val watcher = LifecycleEventObserver { _, e ->
            when (e) {
                Lifecycle.Event.ON_START -> show()
                Lifecycle.Event.ON_STOP -> hide()
                else -> {}
            }
        }
        lifecycle.addObserver(watcher)
        onDispose {
            lifecycle.removeObserver(watcher)
            hide()
        }
    }
    // the thread went away on the computer
    LaunchedEffect(board, t) {
        if (board != null && t == null) onBack()
    }

    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().imePadding()) {
        Row(
            Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
            }
            AgentMark(t?.agent, 16.dp)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    t?.title?.ifBlank { "New thread" } ?: "",
                    style = MaterialTheme.typography.titleMedium,
                    color = h.text1,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                val sub = when {
                    t == null -> ""
                    t.status == BoardStatus.Waiting -> t.doing ?: "Waiting for you"
                    t.status == BoardStatus.Working -> t.doing ?: "Working"
                    else -> listOfNotNull(t.model, t.place.takeIf { it.isNotEmpty() }).joinToString("  ·  ")
                }
                if (sub.isNotEmpty()) {
                    Text(
                        sub,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (t?.status == BoardStatus.Waiting) h.waiting else h.text3,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (t != null) StatusMark(t.status, false)
            var menu by remember { mutableStateOf(false) }
            IconButton(onClick = { menu = true }) {
                Icon(painterResource(R.drawable.ic_ellipsis), "More", Modifier.size(20.dp), tint = h.text2)
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    if (t?.shelf == Shelf.Active) {
                        DropdownMenuItem(text = { Text("Settle") }, onClick = {
                            menu = false
                            app.link.ask(Ask.Settle(id, true))
                            onBack()
                        })
                    } else {
                        DropdownMenuItem(text = { Text("Bring back") }, onClick = {
                            menu = false
                            app.link.ask(Ask.Settle(id, false))
                        })
                    }
                }
            }
        }
        HorizontalDivider(color = h.border1)
        ConnStrip(conn) { app.link.start(); app.link.nudge() }
        when (t?.kind) {
            BoardKind.Structured -> {
                val view by remember(id) { app.link.transcript(id) }.collectAsStateWithLifecycle()
                Chat(
                    view = view,
                    working = t.status == BoardStatus.Working || t.status == BoardStatus.Waiting,
                    busy = t.status == BoardStatus.Working,
                    doing = t.doing,
                    since = t.since,
                    canAnswer = online && t.live,
                    onSend = { text, images -> app.link.ask(Ask.Send(id, text, images)) },
                    onStop = { app.link.ask(Ask.Interrupt(id)) },
                    onAnswer = { request, answer -> app.link.ask(Ask.Approve(id, request, answer)) },
                    attach = if (app.link.desktopAtLeast(PHOTOS)) { onPhoto -> rememberPhotoPicker(app, true, onPhoto) } else null,
                    modifier = Modifier.weight(1f),
                )
            }
            BoardKind.Terminal -> {
                val view by remember(id) { app.link.term(id) }.collectAsStateWithLifecycle()
                key(visit) {
                    Terminal(
                        view = view,
                        live = online && t.live,
                        autoFit = saved.fit,
                        onFit = { cols, rows -> app.link.fit(id, cols, rows) },
                        onKeys = { app.link.send(Up.Keys(id, it)) },
                        onPaste = { app.link.send(Up.Paste(id, it)) },
                        onImage = if (app.link.desktopAtLeast(PHOTOS)) {
                            rememberPhotoPicker(app, false) { p ->
                                p.path?.let { path -> app.link.send(Up.Paste(id, (if (' ' in path) "\"$path\"" else path) + " ")) }
                            }
                        } else {
                            null
                        },
                        modifier = Modifier.weight(1f),
                    )
                }
            }
            null -> Spacer(Modifier.weight(1f))
        }
    }
}

private const val PHOTOS = "0.24.12"
