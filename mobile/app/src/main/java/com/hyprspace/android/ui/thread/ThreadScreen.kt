// One thread: its bar (back, agent, title, what it's doing) over the transcript or the terminal.
// Opening it tells the computer to stream it; leaving stops the stream and gives a terminal its
// desktop size back.

package com.hyprspace.android.ui.thread

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.model.Item
import com.hyprspace.android.model.exported
import com.hyprspace.android.net.Agent
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.BoardKind
import com.hyprspace.android.net.BoardStatus
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.Shelf
import com.hyprspace.android.net.Up
import com.hyprspace.android.ui.ConnStrip
import com.hyprspace.android.ui.Feel
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.StatusMark
import com.hyprspace.android.ui.feel
import com.hyprspace.android.ui.felt
import com.hyprspace.android.ui.home.SnoozeSheet
import com.hyprspace.android.ui.start.NewThreadScreen

@Composable
fun ThreadScreen(app: App, id: Long, onBack: () -> Unit, onOpen: (Long) -> Unit = {}, onFiles: () -> Unit = {}) {
    val h = LocalHues.current
    val host = LocalView.current
    val board by app.link.board.collectAsStateWithLifecycle()
    val conn by app.link.conn.collectAsStateWithLifecycle()
    val lines by app.link.lines.collectAsStateWithLifecycle()
    val saved by app.store.saved.collectAsStateWithLifecycle()
    val t = board?.threads?.firstOrNull { it.id == id }
    val online = conn is Conn.Online
    // /clear opens a new thread here; once it starts, the threads there were before spot it
    var fresh by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    var snoozing by remember { mutableStateOf(false) }
    var before by remember { mutableStateOf<Set<Long>?>(null) }
    LaunchedEffect(board) {
        val known = before ?: return@LaunchedEffect
        val made = board?.threads?.firstOrNull { it.id !in known } ?: return@LaunchedEffect
        before = null
        onOpen(made.id)
    }

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
            Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 6.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(22.dp), tint = h.text1)
            }
            Column(Modifier.weight(1f).padding(start = 2.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        t?.title?.ifBlank { "New thread" } ?: "",
                        Modifier.weight(1f, fill = false),
                        style = MaterialTheme.typography.titleLarge,
                        color = h.text1,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (t != null) {
                        Spacer(Modifier.width(8.dp))
                        StatusMark(t.status)
                    }
                }
                val computer = (conn as? Conn.Online)?.desktop ?: saved.current()?.name
                val sub = when {
                    t == null -> ""
                    t.status == BoardStatus.Waiting -> t.doing ?: "Waiting for you"
                    t.status == BoardStatus.Working -> t.doing ?: "Working"
                    else -> listOfNotNull(
                        board?.spaces?.firstOrNull { it.id == t.space }?.name,
                        t.place.takeIf { t.branch && it.isNotEmpty() },
                        computer,
                    ).joinToString(" · ")
                }
                if (sub.isNotEmpty()) {
                    Text(
                        sub,
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (t?.status == BoardStatus.Waiting) h.waiting else h.text3,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (t != null && t.cwd.isNotEmpty() && app.link.desktopAtLeast(NEXT)) {
                IconButton(onClick = onFiles) {
                    Icon(painterResource(R.drawable.ic_folder), "Files and changes", Modifier.size(21.dp), tint = h.text2)
                }
            }
            var menu by remember { mutableStateOf(false) }
            IconButton(onClick = { menu = true }) {
                Icon(painterResource(R.drawable.ic_ellipsis), "More", Modifier.size(22.dp), tint = h.text2)
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    val item = @Composable { label: String, icon: Int, run: () -> Unit ->
                        DropdownMenuItem(
                            text = { Text(label) },
                            leadingIcon = { Icon(painterResource(icon), null, Modifier.size(18.dp), tint = h.text2) },
                            onClick = {
                                menu = false
                                run()
                            },
                        )
                    }
                    if (t != null && app.link.desktopAtLeast(RENAME)) item("Rename", R.drawable.ic_pencil) { renaming = true }
                    if (t?.kind == BoardKind.Structured && t.live && app.link.desktopAtLeast(NEXT)) {
                        item("Regenerate title", R.drawable.ic_sparkles) { host.felt(app.link.ask(Ask.Retitle(id)), Feel.Tick) }
                    }
                    if (t?.shelf == Shelf.Active) {
                        if (app.link.desktopAtLeast(PINS)) {
                            if (t.pinned) {
                                item("Unpin", R.drawable.ic_pin_off) { host.felt(app.link.ask(Ask.Pin(id, false))) }
                            } else {
                                item("Pin to top", R.drawable.ic_pin) { host.felt(app.link.ask(Ask.Pin(id, true))) }
                            }
                        }
                        if (app.link.desktopAtLeast(SNOOZE)) item("Snooze", R.drawable.ic_clock) { snoozing = true }
                        item("Settle", R.drawable.ic_archive) {
                            if (host.felt(app.link.ask(Ask.Settle(id, true)))) onBack()
                        }
                    } else {
                        item("Bring back", R.drawable.ic_archive_restore) { host.felt(app.link.ask(Ask.Settle(id, false))) }
                    }
                }
            }
        }
        ConnStrip(conn) { app.link.start(); app.link.nudge() }
        // the thread sits on a sheet of its own under the header
        Column(
            Modifier
                .weight(1f)
                .clip(RoundedCornerShape(topStart = 26.dp, topEnd = 26.dp))
                .background(if (h.dark) h.ink(0.03f) else h.surface2),
        ) {
        when (t?.kind) {
            BoardKind.Structured -> {
                val view by remember(id) { app.link.transcript(id) }.collectAsStateWithLifecycle()
                val asked by remember(id) { app.link.aside(id) }.collectAsStateWithLifecycle()
                var picking by remember { mutableStateOf(false) }
                val catalog = board?.agents?.firstOrNull { it.agent == t.agent }?.catalog?.takeIf { app.link.desktopAtLeast(MODELS) }
                val queueing = app.link.desktopAtLeast(QUEUE)
                val slashes by app.link.commands.collectAsStateWithLifecycle()
                val context = LocalContext.current
                // a command whose result shows on the phone runs here; the rest goes to the computer
                val here: (String) -> Boolean = { text ->
                    val words = text.trim().removePrefix("/").split(Regex("\\s+"), limit = 2)
                    val (name, arg) = words[0] to words.getOrElse(1) { "" }
                    when {
                        !text.trim().startsWith("/") -> false
                        name == "copy" -> {
                            val reply = view.items.lastOrNull { it is Item.Reply } as Item.Reply?
                            if (reply == null) {
                                app.link.say("There's no reply to copy yet.")
                            } else {
                                clip(context, reply.text)
                                host.feel(Feel.Tap)
                                app.link.say("Copied the last reply.")
                            }
                            true
                        }
                        name == "export" -> {
                            clip(context, exported(view.items))
                            host.feel(Feel.Tap)
                            app.link.say("Copied the conversation as Markdown.")
                            true
                        }
                        (name == "model" || name == "effort") && arg.isBlank() && catalog != null -> {
                            picking = true
                            true
                        }
                        name == "clear" || name == "new" -> {
                            fresh = true
                            true
                        }
                        else -> false
                    }
                }
                val media = remember(t.cwd) { MediaSource(t.cwd) { path, into -> app.link.fetch(path, into) } }
                CompositionLocalProvider(LocalMedia provides media.takeIf { app.link.desktopAtLeast(NEXT) }) {
                Chat(
                    view = view,
                    working = t.status == BoardStatus.Working || t.status == BoardStatus.Waiting,
                    busy = t.status == BoardStatus.Working,
                    doing = t.doing,
                    since = t.since,
                    canAnswer = online && t.live,
                    onSend = { text, images -> if (images.isNotEmpty() || !here(text)) app.link.ask(Ask.Send(id, text, images)) },
                    onStop = { host.felt(app.link.ask(Ask.Interrupt(id))) },
                    onAnswer = { request, answer, given ->
                        val sent = app.link.ask(Ask.Approve(id, request, answer, given.map { listOf(it.first, it.second) }))
                        host.felt(sent, if (answer == Answer.Deny) Feel.Tap else Feel.Done)
                    },
                    attach = if (app.link.desktopAtLeast(PHOTOS)) { onPick -> rememberAttach(app, app.link.desktopAtLeast(FILES), onPick) } else null,
                    modifier = Modifier.weight(1f),
                    model = catalog?.let {
                        { ModelChip(t.agent, t.model ?: "Default", t.effort, enabled = t.status != BoardStatus.Working && t.status != BoardStatus.Waiting) { picking = true } }
                    },
                    extras = if (queueing) {
                        Extras(
                            queue = t.queue,
                            tasks = t.tasks,
                            onSteer = { i, text -> host.felt(app.link.ask(Ask.Steer(id, i, text)), Feel.Tick) },
                            onUnqueue = { i, text -> host.felt(app.link.ask(Ask.Unqueue(id, i, text)), Feel.Tick) },
                            onStopTask = { task -> host.felt(app.link.ask(Ask.StopTask(id, task))) },
                            redirects = app.link.desktopAtLeast(REDIRECT),
                        )
                    } else {
                        null
                    },
                    ultra = t.agent == Agent.Claude,
                    lines = lines,
                    onEarlier = { app.link.earlier(id) },
                    aside = asked,
                    onDropAside = { app.link.ask(Ask.DropAside(id)) },
                    commands = slashes.filter { it.agent == null || it.agent == t.agent },
                    onFork = if (app.link.desktopAtLeast(NEXT)) {
                        { keep, prompt ->
                            if (app.link.ask(Ask.Fork(id, keep, prompt))) {
                                before = board?.threads?.map { it.id }?.toSet()
                                app.link.say("Forked. It opens as soon as the computer makes it.")
                            } else {
                                host.feel(Feel.Fail)
                            }
                        }
                    } else {
                        null
                    },
                )
                }
                if (picking && catalog != null) {
                    ModelSheet(
                        catalog,
                        t.modelId,
                        t.effort,
                        onPick = { m, e -> if (!app.link.ask(Ask.Model(id, m, e))) host.feel(Feel.Fail) },
                        onDismiss = { picking = false },
                        bottom = if (queueing) {
                            { PermissionPicker(t.permission) { if (!app.link.ask(Ask.SetPermission(id, it))) host.feel(Feel.Fail) } }
                        } else {
                            null
                        },
                    )
                }
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
                        attach = if (app.link.desktopAtLeast(PHOTOS)) { onPick -> rememberAttach(app, app.link.desktopAtLeast(FILES), onPick) } else null,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
            null -> Spacer(Modifier.weight(1f))
        }
        }
    }
    if (renaming && t != null) {
        Rename(t.title, onDismiss = { renaming = false }) { title ->
            renaming = false
            host.felt(app.link.ask(Ask.Rename(id, title)))
        }
    }
    if (snoozing && t != null) {
        SnoozeSheet(
            t.status == BoardStatus.Working || t.status == BoardStatus.Waiting,
            onPick = { until, label ->
                snoozing = false
                if (host.felt(app.link.ask(Ask.Snooze(id, until)))) {
                    app.link.say("Snoozed until $label")
                    onBack()
                }
            },
            onDismiss = { snoozing = false },
        )
    }
    val b = board
    if (fresh && b != null && t != null) {
        val computer = (conn as? Conn.Online)?.desktop ?: saved.current()?.name ?: "HyprSpace"
        NewThreadScreen(
            app,
            b,
            t.space,
            computer,
            onDismiss = { fresh = false },
            onStarted = {
                before = b.threads.map { it.id }.toSet()
                fresh = false
            },
        )
    }
}

private const val PHOTOS = "0.24.12"
private const val SNOOZE = "0.24.12"
private const val PINS = "0.24.14"
private const val RENAME = "0.24.15"
private const val NEXT = "0.24.37"
private const val MODELS = "0.24.14"
private const val FILES = "0.24.14"
private const val QUEUE = "0.24.35"
private const val REDIRECT = "0.24.36"

@Composable
private fun Rename(was: String, onDismiss: () -> Unit, onRename: (String) -> Unit) {
    val h = LocalHues.current
    var title by remember { mutableStateOf(TextFieldValue(was, TextRange(0, was.length))) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val ok = title.text.isNotBlank() && title.text.trim() != was
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = h.surface2,
        title = { Text("Rename thread") },
        text = {
            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
                shape = RoundedCornerShape(12.dp),
                colors = OutlinedTextFieldDefaults.colors(
                    focusedBorderColor = h.border2,
                    unfocusedBorderColor = h.border1,
                    focusedContainerColor = h.surface1,
                    unfocusedContainerColor = h.surface1,
                ),
            )
        },
        confirmButton = {
            TextButton(onClick = { onRename(title.text.trim()) }, enabled = ok) { Text("Rename") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

private fun clip(context: Context, text: String) {
    context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("HyprSpace", text))
}
