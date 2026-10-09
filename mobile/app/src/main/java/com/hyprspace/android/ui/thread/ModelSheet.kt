package com.hyprspace.android.ui.thread

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.hyprspace.android.R
import com.hyprspace.android.net.Agent
import com.hyprspace.android.net.AgentCatalog
import com.hyprspace.android.net.ModelInfo
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet

@Composable
fun ModelChip(agent: Agent?, label: String, effort: String, enabled: Boolean, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier
            .widthIn(max = 220.dp)
            .clip(RoundedCornerShape(10.dp))
            .then(if (enabled) Modifier.clickableQuiet(onClick) else Modifier)
            .alpha(if (enabled) 1f else 0.5f)
            .padding(horizontal = 8.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        AgentMark(agent, 13.dp)
        Text(label, Modifier.weight(1f, fill = false), style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.Medium, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
        if (effort.isNotEmpty()) Text(effortLabel(effort), style = MaterialTheme.typography.labelLarge, color = h.text3, maxLines = 1)
        Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(14.dp), tint = h.text3)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ModelSheet(
    catalog: AgentCatalog?,
    model: String,
    effort: String,
    onPick: (String, String) -> Unit,
    onDismiss: () -> Unit,
    note: String = "Applies from your next message.",
    top: (@Composable () -> Unit)? = null,
) {
    val h = LocalHues.current
    var picked by remember(catalog?.agent) { mutableStateOf(model.substringBefore('[')) }
    var level by remember(catalog?.agent) { mutableStateOf(effort) }
    fun pick(m: String, e: String) {
        picked = m
        level = e
        onPick(m, e)
    }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Model", style = MaterialTheme.typography.titleLarge, color = h.text1)
            top?.invoke()
            if (catalog != null) Column(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f)).padding(4.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                val options = listOf(ModelInfo(id = "", label = "Default", note = "The agent's own pick")) +
                    catalog.models.filter { it.id.isNotEmpty() }
                for (m in options) {
                    ModelRow(m, m.id == picked) { pick(m.id, level.takeIf { it in catalog.effortsFor(m.id) } ?: "") }
                }
            }
            val efforts = catalog?.effortsFor(picked).orEmpty()
            if (efforts.isNotEmpty()) {
                val fallback = catalog?.models?.firstOrNull { it.id == picked }?.defaultEffort
                EffortSlider(efforts, level, fallback) { pick(picked, it) }
            }
            if (note.isNotEmpty()) Text(note, style = MaterialTheme.typography.bodySmall, color = h.text3)
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun ModelRow(m: ModelInfo, on: Boolean, onClick: () -> Unit) {
    val h = LocalHues.current
    val fill by animateColorAsState(if (on) h.accent.copy(alpha = 0.13f) else Color.Transparent, label = "pick")
    val note = m.note?.trim().orEmpty()
    val badge = note.isNotEmpty() && note.length <= 14 && m.id.isNotEmpty()
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(fill)
            .clickableQuiet(onClick)
            .padding(horizontal = 12.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    m.label,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (on) FontWeight.SemiBold else FontWeight.Medium,
                    color = h.text1,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (badge) {
                    Text(
                        note,
                        Modifier.clip(RoundedCornerShape(50)).background(h.ink(0.07f)).padding(horizontal = 7.dp, vertical = 1.dp),
                        style = MaterialTheme.typography.labelSmall,
                        color = h.text2,
                        maxLines = 1,
                    )
                }
            }
            if (note.isNotEmpty() && !badge) {
                Text(note, style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Icon(
            painterResource(R.drawable.ic_check),
            if (on) "Current" else null,
            Modifier.padding(start = 10.dp).size(18.dp).alpha(if (on) 1f else 0f),
            tint = h.accent,
        )
    }
}
