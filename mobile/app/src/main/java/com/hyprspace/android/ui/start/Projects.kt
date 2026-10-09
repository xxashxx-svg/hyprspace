package com.hyprspace.android.ui.start

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.BoardSpace
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.SpaceTag
import com.hyprspace.android.ui.clickableQuiet

@Composable
internal fun Projects(spaces: List<BoardSpace>, current: Long?, onPick: (Long) -> Unit, onBrowse: (() -> Unit)?, onBack: () -> Unit) {
    val h = LocalHues.current
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    val shown = spaces.filter { query.isBlank() || it.name.contains(query.trim(), true) || it.path.contains(query.trim(), true) }
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { if (searching) { searching = false; query = "" } else onBack() }) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
            }
            if (searching) {
                val focus = remember { FocusRequester() }
                LaunchedEffect(Unit) { focus.requestFocus() }
                Box(Modifier.weight(1f).padding(end = 12.dp)) {
                    if (query.isEmpty()) Text("Search projects", style = MaterialTheme.typography.bodyLarge, color = h.text3)
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth().focusRequester(focus),
                        textStyle = MaterialTheme.typography.bodyLarge.copy(color = h.text1),
                        cursorBrush = SolidColor(h.accent),
                    )
                }
            } else {
                Text("Choose project", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium, color = h.text1)
                IconButton(onClick = { searching = true }) {
                    Icon(painterResource(R.drawable.ic_search), "Search", Modifier.size(20.dp), tint = h.text2)
                }
                if (onBrowse != null) {
                    IconButton(onClick = onBrowse) {
                        Icon(painterResource(R.drawable.ic_plus), "Another folder", Modifier.size(20.dp), tint = h.text2)
                    }
                }
            }
        }
        LazyColumn(Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
            if (shown.isEmpty()) {
                item {
                    Text(
                        if (spaces.isEmpty()) "No projects on this computer yet." else "No project matches.",
                        Modifier.padding(horizontal = 4.dp, vertical = 16.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = h.text3,
                    )
                }
            }
            itemsIndexed(shown, key = { _, s -> s.id }) { i, s ->
                val shape = when {
                    shown.size == 1 -> RoundedCornerShape(14.dp)
                    i == 0 -> RoundedCornerShape(topStart = 14.dp, topEnd = 14.dp)
                    i == shown.lastIndex -> RoundedCornerShape(bottomStart = 14.dp, bottomEnd = 14.dp)
                    else -> RoundedCornerShape(0.dp)
                }
                Column(Modifier.fillMaxWidth().clip(shape).background(h.ink(0.04f))) {
                    if (i > 0) HorizontalDivider(Modifier.padding(start = 52.dp), color = h.border0)
                    Row(
                        Modifier.fillMaxWidth().clickableQuiet { onPick(s.id) }.padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        SpaceTag(s, 24.dp)
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(s.name, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(s.path, style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
                        }
                        Spacer(Modifier.width(10.dp))
                        if (s.id == current) {
                            Icon(painterResource(R.drawable.ic_check), "Current", Modifier.size(18.dp), tint = h.accent)
                        } else {
                            Icon(painterResource(R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
                        }
                    }
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
}

@Composable
internal fun FolderBrowser(app: App, onPick: (String) -> Unit, onBack: () -> Unit) {
    val h = LocalHues.current
    val here by app.link.folders.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
            }
            Text("Choose a folder", style = MaterialTheme.typography.titleMedium, color = h.text1)
        }
        val f = here
        if (f == null) {
            Text("Loading", Modifier.padding(horizontal = 20.dp), style = MaterialTheme.typography.bodyMedium, color = h.text3)
            return@Column
        }
        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(f.path, Modifier.padding(horizontal = 4.dp), fontFamily = Mono, style = MaterialTheme.typography.bodySmall, color = h.text2)
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f))) {
                f.parent?.let { up ->
                    FolderRow(R.drawable.ic_arrow_up, "Up a level") { app.link.browse(up) }
                }
                for (d in f.dirs) {
                    FolderRow(R.drawable.ic_folder, leaf(d)) { app.link.browse(d) }
                }
                if (f.dirs.isEmpty()) {
                    Text(
                        "No folders in here.",
                        Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = h.text3,
                    )
                }
            }
        }
        Button(
            onClick = { onPick(f.path) },
            modifier = Modifier.fillMaxWidth().padding(16.dp).height(48.dp),
            shape = RoundedCornerShape(12.dp),
            colors = ButtonDefaults.buttonColors(containerColor = h.accent, contentColor = h.onAccent),
        ) {
            Text("Use ${leaf(f.path)}", fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
private fun FolderRow(icon: Int, label: String, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onClick).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(painterResource(icon), null, Modifier.size(16.dp), tint = h.text3)
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Icon(painterResource(R.drawable.ic_chevron_right), null, Modifier.size(14.dp), tint = h.text3)
    }
}
