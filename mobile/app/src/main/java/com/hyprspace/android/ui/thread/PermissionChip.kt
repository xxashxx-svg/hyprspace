// How much a structured thread's agent may do without asking, picked for its next message. It sits
// in the model sheet under the effort, so the composer keeps one control for how the agent runs.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import com.hyprspace.android.net.Permission
import com.hyprspace.android.ui.Feel
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.feel

@Composable
fun PermissionPicker(current: Permission, onPick: (Permission) -> Unit) {
    val h = LocalHues.current
    val host = LocalView.current
    var picked by remember(current) { mutableStateOf(current) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Permission", Modifier.padding(start = 4.dp), style = MaterialTheme.typography.labelLarge, color = h.text3)
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f)).padding(4.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            for (p in Permission.entries) {
                PickRow(p.label, p.about, p == picked) {
                    if (p != picked) {
                        host.feel(Feel.Tick)
                        picked = p
                        onPick(p)
                    }
                }
            }
        }
    }
}
