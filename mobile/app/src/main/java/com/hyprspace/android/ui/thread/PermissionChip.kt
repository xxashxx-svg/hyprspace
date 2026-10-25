// How much a structured thread's agent may do without asking, picked for its next message.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.hyprspace.android.R
import com.hyprspace.android.net.Permission
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet

@Composable
fun PermissionChip(current: Permission, enabled: Boolean, onPick: (Permission) -> Unit) {
    val h = LocalHues.current
    var open by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .clip(RoundedCornerShape(10.dp))
                .then(if (enabled) Modifier.clickableQuiet { open = true } else Modifier)
                .alpha(if (enabled) 1f else 0.5f)
                .padding(horizontal = 8.dp, vertical = 7.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(current.label, style = MaterialTheme.typography.labelLarge, color = h.text2, maxLines = 1)
            Icon(painterResource(R.drawable.ic_chevron_down), null, Modifier.size(14.dp), tint = h.text3)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (p in Permission.entries) {
                DropdownMenuItem(
                    text = {
                        Column(Modifier.widthIn(max = 260.dp).padding(vertical = 2.dp)) {
                            Text(p.label, fontWeight = if (p == current) FontWeight.SemiBold else FontWeight.Normal, color = h.text1)
                            Text(p.about, style = MaterialTheme.typography.bodySmall, color = h.text3)
                        }
                    },
                    trailingIcon = if (p == current) {
                        { Icon(painterResource(R.drawable.ic_check), null, Modifier.size(15.dp), tint = h.accent) }
                    } else {
                        null
                    },
                    onClick = {
                        open = false
                        if (p != current) onPick(p)
                    },
                )
            }
        }
    }
}
