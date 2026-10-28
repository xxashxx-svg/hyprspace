// Each agent's plan limits, as the computer's usage meter last read them: how much of each window
// is used and when it resets.

package com.hyprspace.android.ui.home

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.BoardBar
import com.hyprspace.android.ui.AgentMark
import com.hyprspace.android.ui.LocalHues
import kotlin.math.roundToInt

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UsageSheet(board: Board, onDismiss: () -> Unit) {
    val h = LocalHues.current
    val now = System.currentTimeMillis()
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            Text("Usage", style = MaterialTheme.typography.titleLarge, color = h.text1)
            for (u in board.usage) {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        AgentMark(u.agent, 16.dp)
                        Text(
                            u.plan ?: u.agent.name,
                            Modifier.padding(start = 8.dp),
                            style = MaterialTheme.typography.titleMedium,
                            color = h.text1,
                        )
                    }
                    for (b in u.bars) Bar(b, now)
                    u.note?.takeIf { it.isNotBlank() }?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = h.text3)
                    }
                }
            }
            Text(
                "From the computer's usage meter, which reads it every few minutes.",
                style = MaterialTheme.typography.bodySmall,
                color = h.text3,
            )
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun Bar(b: BoardBar, now: Long) {
    val h = LocalHues.current
    val used = (b.percent / 100).coerceIn(0.0, 1.0).toFloat()
    val fill = when {
        b.percent >= 90 -> h.error
        b.percent >= 70 -> h.busy
        else -> h.accent
    }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row {
            Text(b.label, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, color = h.text2)
            Text("${b.percent.roundToInt()}%", style = MaterialTheme.typography.bodyMedium, color = h.text1)
        }
        Box(Modifier.fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(h.ink(0.08f))) {
            Box(Modifier.fillMaxWidth(used).fillMaxHeight().clip(RoundedCornerShape(3.dp)).background(fill))
        }
        b.resetsAt?.let { Text("Resets in ${until(it - now)}", style = MaterialTheme.typography.bodySmall, color = h.text3) }
    }
}

private fun until(ms: Long): String {
    val m = (ms / 60_000).coerceAtLeast(1)
    return when {
        m < 60 -> "$m min"
        m < 48 * 60 -> "${m / 60} h ${m % 60} min"
        else -> "${m / (24 * 60)} days"
    }
}
