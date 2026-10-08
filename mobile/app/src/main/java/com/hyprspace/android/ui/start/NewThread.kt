// Starting a thread from the phone: which space, which agent (or a plain shell), its model and
// effort, how it runs, how much it may do alone, and what to tell it. The picks start where the
// desktop's composer left them.

package com.hyprspace.android.ui.start

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Agent
import com.hyprspace.android.net.Ask
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.NewThread
import com.hyprspace.android.net.Permission
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.SpaceTag
import com.hyprspace.android.ui.clickableQuiet

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewThreadSheet(app: App, board: Board, space: Long, onDismiss: () -> Unit, onStarted: (Long) -> Unit = {}) {
    val h = LocalHues.current
    val installed = board.agents.filter { it.status.installed }.map { it.agent }
    var spaceId by remember { mutableStateOf(space) }
    val order = remember { board.spaces.sortedByDescending { it.id == space } }
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
    var terminal by remember { mutableStateOf(true) }
    var prompt by remember { mutableStateOf("") }
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet, containerColor = h.surface2) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .navigationBarsPadding()
                .imePadding(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text("New thread", style = MaterialTheme.typography.titleLarge, color = h.text1)

            Field("Space") {
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (s in order) {
                        Chip(on = s.id == spaceId, onClick = { spaceId = s.id }) {
                            SpaceTag(s, 16.dp)
                            Text(s.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                    }
                }
            }

            Field("Agent") {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (a in installed) {
                        Chip(on = agent == a, onClick = { agent = a }) {
                            AgentMark(a, 14.dp)
                            Text(a.label)
                        }
                    }
                    Chip(on = agent == null, onClick = { agent = null }) {
                        AgentMark(null, 14.dp)
                        Text("Shell")
                    }
                }
            }

            if (agent != null && catalog != null) {
                Field("Model") {
                    val options = listOf("" to "Default") + catalog.models.filter { it.id.isNotEmpty() }.map { it.id to it.label }
                    Picker(options.firstOrNull { it.first == model.substringBefore('[') }?.second ?: model.ifEmpty { "Default" }, options.map { it.second }) { i ->
                        model = options[i].first
                    }
                }
                val efforts = catalog.effortsFor(model)
                if (efforts.isNotEmpty()) {
                    Field("Effort") {
                        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Chip(on = effort.isEmpty(), onClick = { effort = "" }) { Text("Default") }
                            for (e in efforts) {
                                Chip(on = effort == e, onClick = { effort = e }) { Text(e.replaceFirstChar { it.uppercase() }) }
                            }
                        }
                    }
                }
                Field("Runs as") {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Chip(on = terminal, onClick = { terminal = true }) {
                            AgentMark(null, 13.dp)
                            Text("Terminal")
                        }
                        Chip(on = !terminal, onClick = { terminal = false }) {
                            Icon(painterResource(R.drawable.ic_sparkles), null, Modifier.size(14.dp))
                            Text("Structured")
                        }
                    }
                    Text(
                        if (terminal) "The agent's own CLI, as on the computer."
                        else "Messages, tool calls and approvals, made for a phone screen. Still being built.",
                        style = MaterialTheme.typography.bodySmall,
                        color = h.text3,
                    )
                }
                Field("Permission") {
                    Picker(permission.label, Permission.entries.map { it.label }) { permission = Permission.entries[it] }
                    Text(
                        permission.about,
                        style = MaterialTheme.typography.bodySmall,
                        color = if (permission == Permission.Bypass) h.error else h.text3,
                    )
                }
                OutlinedTextField(
                    value = prompt,
                    onValueChange = { prompt = it },
                    modifier = Modifier.fillMaxWidth().heightIn(min = 110.dp),
                    placeholder = { Text("What should it do? You can leave this empty.") },
                    shape = RoundedCornerShape(12.dp),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedBorderColor = h.border2,
                        unfocusedBorderColor = h.border1,
                        focusedContainerColor = h.surface1,
                        unfocusedContainerColor = h.surface1,
                    ),
                )
            }

            Button(
                onClick = {
                    val start = NewThread(agent, model, effort, permission, terminal || agent == null, prompt.trim())
                    if (app.link.ask(Ask.New(spaceId, start))) {
                        onStarted(spaceId)
                        onDismiss()
                    }
                },
                modifier = Modifier.fillMaxWidth().height(48.dp),
                shape = RoundedCornerShape(12.dp),
                colors = ButtonDefaults.buttonColors(containerColor = h.accent, contentColor = h.onAccent),
            ) {
                Text(if (agent == null) "Open a shell" else "Start ${agent?.label}", fontWeight = FontWeight.SemiBold)
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun Field(label: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, style = MaterialTheme.typography.labelMedium, color = LocalHues.current.text3)
        content()
    }
}

@Composable
fun Chip(on: Boolean, onClick: () -> Unit, content: @Composable () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier
            .clip(RoundedCornerShape(10.dp))
            .background(if (on) h.surface3 else h.surface1)
            .border(1.dp, if (on) h.border2 else h.border1, RoundedCornerShape(10.dp))
            .clickableQuiet(onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        androidx.compose.runtime.CompositionLocalProvider(
            androidx.compose.material3.LocalContentColor provides if (on) h.text1 else h.text2,
            androidx.compose.material3.LocalTextStyle provides MaterialTheme.typography.labelLarge,
        ) { content() }
    }
}

/** A field that opens a list to pick from. */
@Composable
fun Picker(current: String, options: List<String>, onPick: (Int) -> Unit) {
    val h = LocalHues.current
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(10.dp))
                .background(h.surface1)
                .border(1.dp, h.border1, RoundedCornerShape(10.dp))
                .clickableQuiet { open = true }
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(current, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = h.text1)
            Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(16.dp), tint = h.text3)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEachIndexed { i, o ->
                DropdownMenuItem(text = { Text(o) }, onClick = { open = false; onPick(i) })
            }
        }
    }
}
