// Starting a thread from the phone: which space, which agent (or a plain shell), its model and
// effort, how it runs, how much it may do alone, and what to tell it. The picks start where the
// desktop's composer left them.

package com.hyprspace.android.ui.start

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import com.hyprspace.android.net.Agent
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.foundation.layout.offset
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.Spring
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.NewThread
import com.hyprspace.android.net.Permission
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.SpaceTag
import com.hyprspace.android.ui.clickableQuiet
import com.hyprspace.android.ui.thread.ModelChip
import com.hyprspace.android.ui.thread.ModelSheet

private enum class Page { Start, Projects, Browse }

@Composable
fun NewThreadScreen(app: App, board: Board, space: Long, computer: String, onDismiss: () -> Unit, onStarted: (Long) -> Unit = {}) {
    val h = LocalHues.current
    val installed = board.agents.filter { it.status.installed }.map { it.agent }
    var spaceId by remember { mutableStateOf(space) }
    // null is a plain shell
    var agent by remember {
        mutableStateOf(board.start.agent?.takeIf { it in installed } ?: installed.firstOrNull())
    }
    val catalog = board.agents.firstOrNull { it.agent == agent }?.catalog
    var model by remember(agent) { mutableStateOf(agent?.let { board.start.pick(it).first } ?: "") }
    var effort by remember(agent, model) {
        mutableStateOf(
            agent?.let { a ->
                board.start.pick(a).second.takeIf { catalog?.effortsFor(model)?.contains(it) == true }
            } ?: "",
        )
    }
    var permission by remember { mutableStateOf(board.start.permission) }
    var terminal by remember { mutableStateOf(!board.start.structured) }
    var prompt by remember { mutableStateOf("") }
    var folder by remember { mutableStateOf<String?>(null) }
    var page by remember { mutableStateOf(Page.Start) }
    var picking by remember { mutableStateOf(false) }
    var running by remember { mutableStateOf(false) }
    val canBrowse = app.link.desktopAtLeast(FOLDERS)

    BackHandler {
        when (page) {
            Page.Start -> onDismiss()
            Page.Projects -> page = Page.Start
            Page.Browse -> page = Page.Projects
        }
    }
    Box(
        Modifier
            .fillMaxSize()
            .background(h.bg)
            .clickable(interactionSource = null, indication = null) {},
    ) {
        when (page) {
            Page.Projects -> Projects(
                board.spaces,
                if (folder == null) spaceId else null,
                onPick = { spaceId = it; folder = null; page = Page.Start },
                onBrowse = if (canBrowse) ({ app.link.browse(folder ?: ""); page = Page.Browse }) else null,
                onBack = { page = Page.Start },
            )
            Page.Browse -> FolderBrowser(
                app,
                onPick = { folder = it; page = Page.Start },
                onBack = { page = Page.Projects },
            )
            Page.Start -> Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().imePadding()) {
                Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = onDismiss) {
                        Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
                    }
                    Text("New thread", style = MaterialTheme.typography.titleMedium, color = h.text1)
                }
                Column(
                    Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Spacer(Modifier.height(56.dp))
                    Text(
                        "What should we work on?",
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.SemiBold,
                        color = h.text1,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(18.dp))
                    val current = board.spaces.firstOrNull { it.id == spaceId }
                    Row(
                        Modifier
                            .clip(RoundedCornerShape(10.dp))
                            .clickableQuiet { page = Page.Projects }
                            .padding(horizontal = 10.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        if (folder == null && current != null) {
                            SpaceTag(current, 18.dp)
                        } else {
                            Icon(painterResource(R.drawable.ic_folder), null, Modifier.size(16.dp), tint = h.text2)
                        }
                        Text(
                            folder?.let(::leaf) ?: current?.name ?: "Choose a project",
                            Modifier.widthIn(max = 240.dp),
                            style = MaterialTheme.typography.bodyLarge,
                            color = h.text1,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Icon(painterResource(R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
                    }
                    Row(
                        Modifier.padding(top = 2.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(7.dp),
                    ) {
                        Icon(painterResource(R.drawable.ic_monitor), null, Modifier.size(14.dp), tint = h.text3)
                        Text("on $computer", style = MaterialTheme.typography.bodyMedium, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    Spacer(Modifier.height(24.dp))
                }
                Column(
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 8.dp)
                        .clip(RoundedCornerShape(18.dp))
                        .background(h.ink(0.045f)),
                ) {
                    Box(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 4.dp)) {
                        if (prompt.isEmpty()) {
                            Text(
                                if (agent == null) "Opens a shell in the folder" else "Ask anything",
                                style = MaterialTheme.typography.bodyLarge,
                                color = h.text3,
                            )
                        }
                        BasicTextField(
                            value = prompt,
                            onValueChange = { prompt = it },
                            enabled = agent != null,
                            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp, max = 180.dp),
                            textStyle = MaterialTheme.typography.bodyLarge.copy(color = h.text1),
                            cursorBrush = SolidColor(h.accent),
                        )
                    }
                    Row(
                        Modifier.fillMaxWidth().padding(start = 6.dp, end = 6.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        if (agent != null) {
                            RunChip(terminal, permission) { running = true }
                        }
                        Spacer(Modifier.weight(1f))
                        val label = when {
                            agent == null -> "Shell"
                            else -> catalog?.models?.firstOrNull { it.id == model.substringBefore('[') }?.label ?: model.ifEmpty { "Default" }
                        }
                        ModelChip(agent, label, effort, enabled = true) { picking = true }
                        Spacer(Modifier.width(6.dp))
                        Box(
                            Modifier
                                .size(36.dp)
                                .clip(CircleShape)
                                .background(h.accent)
                                .clickableQuiet {
                                    val start = NewThread(agent, model, effort, permission, terminal || agent == null, prompt.trim())
                                    if (app.link.ask(Ask.New(if (folder != null) 0 else spaceId, start, folder))) {
                                        onStarted(spaceId)
                                        onDismiss()
                                    }
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(painterResource(R.drawable.ic_arrow_up), if (agent == null) "Open a shell" else "Start", Modifier.size(16.dp), tint = h.onAccent)
                        }
                    }
                }
            }
        }
    }

    if (picking) {
        ModelSheet(
            catalog,
            model,
            effort,
            onPick = { m, e -> model = m; effort = e },
            onDismiss = { picking = false },
            note = "",
            top = { AgentTabs(installed + listOf(null), agent) { agent = it } },
        )
    }
    if (running) {
        RunSheet(terminal, permission, onTerminal = { terminal = it }, onPermission = { permission = it }, onDismiss = { running = false })
    }
}

@Composable
private fun RunChip(terminal: Boolean, permission: Permission, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier
            .widthIn(max = 170.dp)
            .clip(RoundedCornerShape(10.dp))
            .clickableQuiet(onClick)
            .padding(horizontal = 8.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (terminal) {
            AgentMark(null, 13.dp)
        } else {
            Icon(painterResource(R.drawable.ic_sparkles), null, Modifier.size(14.dp), tint = h.text2)
        }
        Text(
            permission.label,
            Modifier.weight(1f, fill = false),
            style = MaterialTheme.typography.labelLarge,
            color = h.text2,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(14.dp), tint = h.text3)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RunSheet(
    terminal: Boolean,
    permission: Permission,
    onTerminal: (Boolean) -> Unit,
    onPermission: (Permission) -> Unit,
    onDismiss: () -> Unit,
) {
    val h = LocalHues.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("How it runs", style = MaterialTheme.typography.titleLarge, color = h.text1)
            Group {
                Option("Terminal", "The agent's own CLI, as on the computer.", terminal) { onTerminal(true) }
                HorizontalDivider(Modifier.padding(horizontal = 14.dp), color = h.border0)
                Option("Structured", "Messages, tool calls and approvals, made for a phone.", !terminal) { onTerminal(false) }
            }
            Text("Permission", style = MaterialTheme.typography.labelMedium, color = h.text3)
            Group {
                Permission.entries.forEachIndexed { i, p ->
                    if (i > 0) HorizontalDivider(Modifier.padding(horizontal = 14.dp), color = h.border0)
                    Option(p.label, p.about, permission == p) { onPermission(p) }
                }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun Group(content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(LocalHues.current.ink(0.04f))) { content() }
}

@Composable
private fun Option(label: String, about: String, on: Boolean, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onClick).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge, color = h.text1)
            Text(about, style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
        Icon(painterResource(R.drawable.ic_check), null, Modifier.padding(start = 12.dp).size(18.dp).alpha(if (on) 1f else 0f), tint = h.accent)
    }
}

internal const val FOLDERS = "0.24.8"

internal fun leaf(path: String): String =
    path.trimEnd('\\', '/').substringAfterLast('\\').substringAfterLast('/').ifEmpty { path }

@Composable
private fun AgentTabs(agents: List<Agent?>, picked: Agent?, onPick: (Agent?) -> Unit) {
    val h = LocalHues.current
    val density = LocalDensity.current
    var width by remember { mutableIntStateOf(0) }
    val at = agents.indexOf(picked).coerceAtLeast(0)
    val slot = with(density) { (width / agents.size.coerceAtLeast(1)).toDp() }
    val x by animateDpAsState(slot * at, spring(dampingRatio = 0.8f, stiffness = Spring.StiffnessMediumLow), label = "tab")
    Box(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.05f))
            .padding(3.dp)
            .onSizeChanged { width = it.width },
    ) {
        if (width > 0) {
            Box(
                Modifier
                    .offset { IntOffset(x.roundToPx(), 0) }
                    .width(slot)
                    .height(40.dp)
                    .clip(RoundedCornerShape(9.dp))
                    .background(h.surface3)
                    .border(1.dp, h.border1, RoundedCornerShape(9.dp)),
            )
        }
        Row(Modifier.fillMaxWidth()) {
            for ((i, a) in agents.withIndex()) {
                val on = i == at
                Row(
                    Modifier.weight(1f).height(40.dp).clip(RoundedCornerShape(9.dp)).clickableQuiet { onPick(a) },
                    horizontalArrangement = Arrangement.spacedBy(7.dp, Alignment.CenterHorizontally),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    AgentMark(a, 15.dp)
                    Text(
                        a?.label ?: "Shell",
                        style = MaterialTheme.typography.labelLarge,
                        fontWeight = if (on) FontWeight.SemiBold else FontWeight.Medium,
                        color = if (on) h.text1 else h.text3,
                    )
                }
            }
        }
    }
}
