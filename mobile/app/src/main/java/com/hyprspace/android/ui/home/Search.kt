// The home screen's header while searching: a field that narrows the threads by title or space.

package com.hyprspace.android.ui.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import com.hyprspace.android.R
import com.hyprspace.android.ui.LocalHues

@Composable
fun Search(query: String, onQuery: (String) -> Unit, onClose: () -> Unit) {
    val h = LocalHues.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    Row(
        Modifier.fillMaxWidth().padding(start = 4.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onClose) {
            Icon(painterResource(R.drawable.ic_arrow_left), "Close search", Modifier.size(22.dp), tint = h.text1)
        }
        Box(Modifier.weight(1f).padding(horizontal = 4.dp)) {
            if (query.isEmpty()) Text("Search threads", style = MaterialTheme.typography.titleMedium, color = h.text3)
            BasicTextField(
                value = query,
                onValueChange = onQuery,
                singleLine = true,
                textStyle = MaterialTheme.typography.titleMedium.copy(color = h.text1),
                cursorBrush = SolidColor(h.accent),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
        }
        if (query.isNotEmpty()) {
            IconButton(onClick = { onQuery("") }) {
                Icon(painterResource(R.drawable.ic_x), "Clear", Modifier.size(18.dp), tint = h.text3)
            }
        }
    }
}
