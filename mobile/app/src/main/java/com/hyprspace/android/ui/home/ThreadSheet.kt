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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.hyprspace.android.R
import com.hyprspace.android.net.BoardSpace
import com.hyprspace.android.net.BoardThread
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.SpaceTag
import com.hyprspace.android.ui.clickableQuiet

data class Action(val label: String, val icon: Int, val run: () -> Unit)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ThreadSheet(t: BoardThread, space: BoardSpace?, actions: List<Action>, onDismiss: () -> Unit) {
    val h = LocalHues.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (space != null) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        SpaceTag(space, 16.dp)
                        Spacer(Modifier.width(7.dp))
                        Text(space.name, style = MaterialTheme.typography.bodySmall, color = h.text3, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                Text(
                    t.title.ifBlank { "New thread" },
                    style = MaterialTheme.typography.titleLarge,
                    color = h.text1,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f))) {
                actions.forEachIndexed { i, a ->
                    if (i > 0) HorizontalDivider(Modifier.padding(start = 64.dp), color = h.border0)
                    Row(
                        Modifier.fillMaxWidth().clickableQuiet(a.run).padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(Modifier.size(38.dp), contentAlignment = Alignment.Center) {
                            Icon(painterResource(a.icon), null, Modifier.size(18.dp), tint = h.text2)
                        }
                        Spacer(Modifier.width(12.dp))
                        Text(a.label, style = MaterialTheme.typography.bodyLarge, color = h.text1)
                    }
                }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}
