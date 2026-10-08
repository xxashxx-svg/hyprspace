package com.hyprspace.android.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.Desktop
import com.hyprspace.android.service.LinkService
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.ago
import com.hyprspace.android.ui.clickableQuiet

@Composable
fun ComputerRow(
    d: Desktop,
    current: Boolean,
    conn: Conn,
    onClick: (() -> Unit)?,
    trailing: @Composable () -> Unit = {},
) {
    val h = LocalHues.current
    val (dot, status) = when {
        !current -> null to (d.last?.let { "Last at $it" } ?: "Paired ${ago(d.paired, System.currentTimeMillis())} ago")
        conn is Conn.Online -> h.ok to ("Connected" + (d.last?.let { " at $it" } ?: ""))
        conn is Conn.Denied -> h.error to conn.message
        conn is Conn.Offline -> h.error to "Can't reach it right now"
        else -> h.busy to "Connecting"
    }
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (onClick != null) Modifier.clickableQuiet(onClick) else Modifier)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .size(38.dp)
                .clip(RoundedCornerShape(11.dp))
                .background(if (current) h.accent.copy(alpha = 0.14f) else h.ink(0.05f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(painterResource(R.drawable.ic_monitor), null, Modifier.size(18.dp), tint = if (current) h.accent else h.text2)
        }
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                d.name,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = if (current) FontWeight.SemiBold else FontWeight.Normal,
                color = h.text1,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                dot?.let { Box(Modifier.size(6.dp).clip(RoundedCornerShape(50)).background(it)) }
                Text(status, style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        trailing()
    }
}

fun switchTo(app: App, d: Desktop) {
    if (app.store.saved.value.current()?.id == d.id) return
    app.store.update { it.copy(active = d.id) }
    app.link.restart()
    LinkService.sync(app)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ComputersSheet(app: App, onPair: () -> Unit, onDismiss: () -> Unit) {
    val h = LocalHues.current
    val saved by app.store.saved.collectAsStateWithLifecycle()
    val conn by app.link.conn.collectAsStateWithLifecycle()
    val current = saved.current()
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Computers", style = MaterialTheme.typography.titleLarge, color = h.text1)
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f))) {
                saved.desktops.forEachIndexed { i, d ->
                    if (i > 0) HorizontalDivider(Modifier.padding(start = 64.dp), color = h.border0)
                    val on = d.id == current?.id
                    ComputerRow(d, on, conn, onClick = { switchTo(app, d); onDismiss() }) {
                        if (on) Icon(painterResource(R.drawable.ic_check), "Current", Modifier.size(18.dp), tint = h.accent)
                    }
                }
                HorizontalDivider(color = h.border0)
                Row(
                    Modifier.fillMaxWidth().clickableQuiet { onDismiss(); onPair() }.padding(horizontal = 14.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(38.dp), contentAlignment = Alignment.Center) {
                        Icon(painterResource(R.drawable.ic_plus), null, Modifier.size(18.dp), tint = h.text2)
                    }
                    Spacer(Modifier.width(12.dp))
                    Text("Pair a computer", style = MaterialTheme.typography.bodyLarge, color = h.text1)
                }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}
