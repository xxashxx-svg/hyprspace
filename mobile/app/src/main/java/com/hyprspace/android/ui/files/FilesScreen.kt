// A thread's folder on the computer: what git sees changed, with each file's diff and a commit
// box, and the files themselves to browse and read. Reading and git only; the phone never
// writes a file.

package com.hyprspace.android.ui.files

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.DirEntry
import com.hyprspace.android.net.FolderCmd
import com.hyprspace.android.net.FolderEvt
import com.hyprspace.android.net.GitChange
import com.hyprspace.android.net.GitState
import com.hyprspace.android.ui.Feel
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.feel
import com.hyprspace.android.ui.thread.Diff

private enum class Tab { Changes, Files }

/** What the screen shows over its tab: one file's diff, or one file's text. */
private sealed interface Open {
    data class Diff(val path: String) : Open
    data class Read(val path: String) : Open
}

@Composable
fun FilesScreen(app: App, thread: Long, onBack: () -> Unit) {
    val h = LocalHues.current
    val board by app.link.board.collectAsStateWithLifecycle()
    val t = board?.threads?.firstOrNull { it.id == thread }
    val cwd = t?.cwd.orEmpty()
    var tab by rememberSaveable { mutableStateOf(Tab.Changes) }
    var open by remember { mutableStateOf<Open?>(null) }
    var git by remember { mutableStateOf<GitState?>(null) }
    var busy by remember { mutableStateOf(false) }
    var dir by rememberSaveable { mutableStateOf(cwd) }
    var entries by remember { mutableStateOf<Result<List<DirEntry>>?>(null) }
    var text by remember { mutableStateOf<Result<String>?>(null) }
    val host = LocalView.current

    LaunchedEffect(cwd) {
        app.link.folder.collect { e ->
            when (e) {
                is FolderEvt.Git -> if (e.cwd == cwd) git = e.status
                is FolderEvt.GitDone -> if (e.cwd == cwd) {
                    busy = false
                    e.result.ok?.let {
                        host.feel(Feel.Done)
                        if (it.isNotBlank()) app.link.say(it)
                    }
                    e.result.err?.let { host.feel(Feel.Fail); app.link.fail(it) }
                }
                is FolderEvt.Dir -> if (e.path == dir) entries = e.entries.ok?.let { Result.success(it) } ?: Result.failure(Exception(e.entries.err))
                is FolderEvt.File, is FolderEvt.Diff -> {
                    val got = when (e) {
                        is FolderEvt.File -> e.text.takeIf { open == Open.Read(e.path) }
                        is FolderEvt.Diff -> e.text.takeIf { open == Open.Diff(e.path) }
                        else -> null
                    }
                    got?.let { text = it.ok?.let { s -> Result.success(s) } ?: Result.failure(Exception(it.err)) }
                }
            }
        }
    }
    // the board may land after the screen, and the folder with it
    LaunchedEffect(cwd) { if (dir.isEmpty()) dir = cwd }
    LaunchedEffect(cwd, tab) { if (cwd.isNotEmpty() && tab == Tab.Changes) app.link.folder(FolderCmd.GitStatus(cwd)) }
    LaunchedEffect(dir, tab) {
        if (dir.isNotEmpty() && tab == Tab.Files) {
            entries = null
            app.link.folder(FolderCmd.ListDir(dir))
        }
    }
    LaunchedEffect(open) {
        text = null
        when (val o = open) {
            is Open.Diff -> app.link.folder(FolderCmd.Diff(cwd, o.path))
            is Open.Read -> app.link.folder(FolderCmd.ReadFile(o.path))
            null -> {}
        }
    }

    val up = parent(dir)?.takeIf { dir != cwd && dir.startsWith(cwd) }
    BackHandler(enabled = open != null || (tab == Tab.Files && up != null)) {
        if (open != null) open = null else dir = up ?: cwd
    }

    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().imePadding()) {
        Row(
            Modifier.fillMaxWidth().padding(start = 4.dp, end = 12.dp, top = 6.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { if (open != null) open = null else onBack() }) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(22.dp), tint = h.text1)
            }
            Column(Modifier.weight(1f)) {
                val title = when (val o = open) {
                    is Open.Diff -> o.path.substringAfterLast('/')
                    is Open.Read -> name(o.path)
                    null -> "Files"
                }
                Text(title, style = MaterialTheme.typography.titleLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    (open as? Open.Diff)?.path ?: t?.title.orEmpty(),
                    style = MaterialTheme.typography.bodyMedium,
                    color = h.text3,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (open == null) {
            Tabs(tab) { tab = it; host.feel(Feel.Tick) }
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when {
                cwd.isEmpty() -> Quiet("This thread has no folder.")
                open != null -> Shown(open!!, text)
                tab == Tab.Changes -> Changes(
                    git,
                    busy,
                    onOpen = { open = Open.Diff(it.path) },
                    onCommit = { message, push ->
                        busy = true
                        if (!app.link.folder(FolderCmd.Commit(cwd, message, push, stageAll = true))) {
                            busy = false
                            host.feel(Feel.Fail)
                        }
                    },
                    onPush = {
                        busy = true
                        if (!app.link.folder(FolderCmd.Push(cwd))) {
                            busy = false
                            host.feel(Feel.Fail)
                        }
                    },
                )
                else -> Browse(
                    entries,
                    at = dir.removePrefix(cwd).trimStart('/', '\\').ifEmpty { name(cwd) },
                    onOpen = { e ->
                        host.feel(Feel.Tick)
                        val path = join(dir, e.name)
                        if (e.dir) dir = path else open = Open.Read(path)
                    },
                )
            }
        }
    }
}

@Composable
private fun Tabs(tab: Tab, onPick: (Tab) -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.padding(horizontal = 16.dp).padding(bottom = 10.dp).clip(RoundedCornerShape(10.dp)).background(h.ink(0.05f)).padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        for (t in Tab.entries) {
            val on = t == tab
            Text(
                t.name,
                Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (on) h.surface3 else h.ink(0f))
                    .clickableQuiet { if (!on) onPick(t) }
                    .padding(horizontal = 16.dp, vertical = 7.dp),
                style = MaterialTheme.typography.labelLarge,
                color = if (on) h.text1 else h.text2,
            )
        }
    }
}

@Composable
private fun Changes(
    git: GitState?,
    busy: Boolean,
    onOpen: (GitChange) -> Unit,
    onCommit: (String, Boolean) -> Unit,
    onPush: () -> Unit,
) {
    val h = LocalHues.current
    when {
        git == null -> Loading()
        !git.branch.isRepo -> Quiet("This folder isn't a git repository.")
        else -> Column(Modifier.fillMaxSize()) {
            val b = git.branch
            Row(Modifier.padding(horizontal = 20.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(painterResource(R.drawable.ic_git_branch), null, Modifier.size(15.dp), tint = h.text2)
                Text(b.branch.ifEmpty { "detached" }, Modifier.padding(start = 8.dp), style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, color = h.text1)
                if (b.ahead > 0) Text("  ↑${b.ahead}", style = MaterialTheme.typography.bodyMedium, color = h.text2)
                if (b.behind > 0) Text("  ↓${b.behind}", style = MaterialTheme.typography.bodyMedium, color = h.text2)
            }
            if (git.changes.isEmpty()) {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    Quiet(if (b.ahead > 0) "Nothing changed. ${b.ahead} commit${if (b.ahead == 1) "" else "s"} to push." else "Nothing changed.")
                }
                if (b.ahead > 0 && b.upstream) {
                    Action(if (busy) "Pushing" else "Push", busy, Modifier.fillMaxWidth().padding(16.dp), onClick = onPush)
                }
            } else {
                LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp)) {
                    items(git.changes, key = { it.path }) { c -> ChangeRow(c) { onOpen(c) } }
                }
                CommitBox(busy, canPush = b.upstream, onCommit)
            }
        }
    }
}

@Composable
private fun ChangeRow(c: GitChange, onClick: () -> Unit) {
    val h = LocalHues.current
    val (mark, tint) = when {
        c.status.contains('?') -> "A" to h.ok
        c.status.contains('D') -> "D" to h.error
        c.status.contains('A') -> "A" to h.ok
        c.status.contains('R') -> "R" to h.waiting
        else -> "M" to h.busy
    }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickableQuiet(onClick).padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(mark, Modifier.width(18.dp), fontFamily = Mono, fontSize = 13.sp, fontWeight = FontWeight.Medium, color = tint)
        Column(Modifier.weight(1f)) {
            Text(c.path.substringAfterLast('/'), style = MaterialTheme.typography.bodyLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if ('/' in c.path) {
                Text(c.path.substringBeforeLast('/'), style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (c.added > 0) Text("+${c.added}", Modifier.padding(start = 8.dp), fontFamily = Mono, fontSize = 12.sp, color = h.diffAdd)
        if (c.removed > 0) Text("-${c.removed}", Modifier.padding(start = 6.dp), fontFamily = Mono, fontSize = 12.sp, color = h.diffDel)
    }
}

@Composable
private fun CommitBox(busy: Boolean, canPush: Boolean, onCommit: (String, Boolean) -> Unit) {
    val h = LocalHues.current
    var message by rememberSaveable { mutableStateOf("") }
    Column(
        Modifier.fillMaxWidth().padding(12.dp).clip(RoundedCornerShape(20.dp)).background(h.ink(0.06f)).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.fillMaxWidth().padding(horizontal = 4.dp)) {
            if (message.isEmpty()) Text("Commit message", style = MaterialTheme.typography.bodyLarge, color = h.text3)
            BasicTextField(
                value = message,
                onValueChange = { message = it },
                textStyle = MaterialTheme.typography.bodyLarge.copy(color = h.text1),
                cursorBrush = SolidColor(h.accent),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        val ok = message.isNotBlank() && !busy
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Action("Commit all", busy, Modifier.weight(1f), enabled = ok, primary = !canPush) {
                onCommit(message.trim(), false)
                message = ""
            }
            if (canPush) {
                Action("Commit and push", busy, Modifier.weight(1f), enabled = ok, primary = true) {
                    onCommit(message.trim(), true)
                    message = ""
                }
            }
        }
    }
}

@Composable
private fun Action(
    label: String,
    busy: Boolean,
    modifier: Modifier = Modifier,
    enabled: Boolean = !busy,
    primary: Boolean = true,
    onClick: () -> Unit,
) {
    val h = LocalHues.current
    Box(
        modifier
            .height(44.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(if (primary && enabled) h.accent else h.ink(0.08f))
            .then(if (enabled) Modifier.clickableQuiet(onClick) else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        if (busy) {
            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = h.text3)
        } else {
            Text(label, style = MaterialTheme.typography.labelLarge, color = if (primary && enabled) h.onAccent else h.text2)
        }
    }
}

@Composable
private fun Browse(entries: Result<List<DirEntry>>?, at: String, onOpen: (DirEntry) -> Unit) {
    val h = LocalHues.current
    when {
        entries == null -> Loading()
        entries.isFailure -> Quiet(entries.exceptionOrNull()?.message ?: "Couldn't list this folder.")
        else -> {
            val list = entries.getOrThrow().sortedWith(compareBy({ !it.dir }, { it.name.lowercase() }))
            LazyColumn(contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp)) {
                item { Text(at, Modifier.padding(horizontal = 12.dp, vertical = 6.dp), style = MaterialTheme.typography.labelLarge, color = h.text3) }
                if (list.isEmpty()) item { Quiet("This folder is empty.") }
                items(list, key = { it.name }) { e ->
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).clickableQuiet { onOpen(e) }.padding(horizontal = 12.dp, vertical = 11.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            painterResource(if (e.dir) R.drawable.ic_folder else R.drawable.ic_file_text),
                            null,
                            Modifier.size(17.dp),
                            tint = if (e.dir) h.text2 else h.text3,
                        )
                        Spacer(Modifier.width(12.dp))
                        Text(e.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (e.dir) Icon(painterResource(R.drawable.ic_chevron_right), null, Modifier.size(15.dp), tint = h.text3)
                    }
                }
            }
        }
    }
}

@Composable
private fun Shown(open: Open, text: Result<String>?) {
    val h = LocalHues.current
    when {
        text == null -> Loading()
        text.isFailure -> Quiet(text.exceptionOrNull()?.message ?: "Couldn't read it.")
        open is Open.Diff -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp)) {
            val d = text.getOrThrow()
            if (d.isBlank()) Quiet("No changes to show.") else Diff(open.path, d)
        }
        else -> {
            val lines = text.getOrThrow().lines()
            val gutter = "${lines.size}".length
            Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).horizontalScroll(rememberScrollState())) {
                Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
                    lines.forEachIndexed { i, line ->
                        Row {
                            Text("${i + 1}".padStart(gutter), Modifier.padding(end = 14.dp), fontFamily = Mono, fontSize = 12.sp, lineHeight = 18.sp, color = h.text3)
                            Text(line, fontFamily = Mono, fontSize = 12.sp, lineHeight = 18.sp, color = h.text1, softWrap = false)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Loading() {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp, color = LocalHues.current.text3)
    }
}

@Composable
private fun Quiet(text: String) {
    Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Text(text, style = MaterialTheme.typography.bodyMedium, color = LocalHues.current.text3)
    }
}

// the computer's own separator, so a Windows path stays one
private fun sep(path: String) = if ('\\' in path && '/' !in path.substringAfter(':')) '\\' else '/'

private fun join(dir: String, name: String) = dir.trimEnd('/', '\\') + sep(dir) + name

private fun name(path: String) = path.trimEnd('/', '\\').substringAfterLast(sep(path))

private fun parent(path: String): String? {
    val trimmed = path.trimEnd('/', '\\')
    val at = trimmed.lastIndexOf(sep(path))
    return if (at > 0) trimmed.substring(0, at) else null
}
