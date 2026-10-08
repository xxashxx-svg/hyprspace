// The phone's settings: the computers it paired with (switch between them, forget one),
// notifications, how terminals size, and the app's version.

package com.hyprspace.android.ui.settings

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.net.APP
import com.hyprspace.android.R
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.Desktop
import com.hyprspace.android.service.LinkService
import com.hyprspace.android.update.Update
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.ago
import com.hyprspace.android.ui.clickableQuiet
import kotlinx.coroutines.launch

@Composable
fun SettingsScreen(app: App, onBack: () -> Unit, onPair: () -> Unit) {
    val h = LocalHues.current
    val saved by app.store.saved.collectAsStateWithLifecycle()
    val conn by app.link.conn.collectAsStateWithLifecycle()
    var forgetting by remember { mutableStateOf<Desktop?>(null) }
    val ctx = LocalContext.current
    // notifications go on once Android lets the app post them
    val allow = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) {
            app.store.update { it.copy(alerts = true) }
            LinkService.sync(app)
        }
    }
    val current = saved.current()

    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
            }
            Text("Settings", style = MaterialTheme.typography.titleLarge, color = h.text1)
        }
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            Group("Computers") {
                for (d in saved.desktops) {
                    val on = d.id == current?.id
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickableQuiet {
                                if (!on) {
                                    app.store.update { it.copy(active = d.id) }
                                    app.link.restart()
                                }
                            }
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(painterResource(R.drawable.ic_monitor), null, Modifier.size(18.dp), tint = if (on) h.accent else h.text3)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(d.name, style = MaterialTheme.typography.bodyLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            val state = when {
                                !on -> "Paired ${ago(d.paired, System.currentTimeMillis())} ago. Tap to use it."
                                conn is Conn.Online -> "Connected" + (d.last?.let { " at $it" } ?: "")
                                conn is Conn.Denied -> (conn as Conn.Denied).message
                                else -> "Connecting"
                            }
                            Text(state, style = MaterialTheme.typography.bodySmall, color = h.text3)
                        }
                        TextButton(onClick = { forgetting = d }) { Text("Forget", color = h.error) }
                    }
                    HorizontalDivider(color = h.border0)
                }
                Row(
                    Modifier.fillMaxWidth().clickableQuiet(onPair).padding(horizontal = 14.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(painterResource(R.drawable.ic_plus), null, Modifier.size(18.dp), tint = h.text2)
                    Spacer(Modifier.width(12.dp))
                    Text("Pair another computer", style = MaterialTheme.typography.bodyLarge, color = h.text1)
                }
            }

            Group("Notifications") {
                Toggle(
                    "When an agent needs you",
                    "Not while you're in the app or at your computer. Stays connected in the background, with a quiet notification.",
                    saved.alerts,
                ) { on ->
                    if (on && Build.VERSION.SDK_INT >= 33 &&
                        ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) !=
                        PackageManager.PERMISSION_GRANTED
                    ) {
                        allow.launch(Manifest.permission.POST_NOTIFICATIONS)
                    } else {
                        app.store.update { it.copy(alerts = on) }
                        LinkService.sync(app)
                    }
                }
                if (saved.alerts) {
                    HorizontalDivider(color = h.border0)
                    Toggle(
                        "When a run finishes",
                        "Errors too.",
                        saved.finished,
                    ) { on -> app.store.update { it.copy(finished = on) } }
                }
            }

            Group("Terminals") {
                Toggle(
                    "Fit to this phone",
                    "Terminals take this phone's width until you type on the computer.",
                    saved.fit,
                ) { on -> app.store.update { it.copy(fit = on) } }
            }

            Group("About") {
                Row(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                    Text("This app", Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = h.text1)
                    Text(APP, fontFamily = Mono, style = MaterialTheme.typography.bodyMedium, color = h.text2)
                }
                HorizontalDivider(color = h.border0)
                Updates(app)
                (conn as? Conn.Online)?.version?.takeIf { it.isNotEmpty() }?.let { desk ->
                    HorizontalDivider(color = h.border0)
                    Row(Modifier.padding(horizontal = 14.dp, vertical = 12.dp)) {
                        Text("HyprSpace on ${(conn as Conn.Online).desktop}", Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge, color = h.text1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(desk, fontFamily = Mono, style = MaterialTheme.typography.bodyMedium, color = h.text2)
                    }
                }
                HorizontalDivider(color = h.border0)
                Text(
                    "Talks only to your paired computers, and to GitHub for updates. The QR reader comes from Google Play services.",
                    Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
                    style = MaterialTheme.typography.bodySmall,
                    color = h.text3,
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    forgetting?.let { d ->
        AlertDialog(
            onDismissRequest = { forgetting = null },
            title = { Text("Forget ${d.name}?") },
            text = { Text("Both sides drop the pairing. To connect again, pair again.") },
            confirmButton = {
                TextButton(onClick = {
                    val was = d.id == current?.id
                    forgetting = null
                    app.scope.launch {
                        app.link.leave(d)
                        if (was) app.link.restart()
                        LinkService.sync(app)
                    }
                }) { Text("Forget", color = h.error) }
            },
            dismissButton = { TextButton(onClick = { forgetting = null }) { Text("Cancel") } },
            containerColor = h.surface2,
        )
    }
}

@Composable
private fun Updates(app: App) {
    val h = LocalHues.current
    val u by app.updater.state.collectAsStateWithLifecycle()
    val (line, action) = if (!app.updater.enabled) "Dev builds don't update" to null else when (val s = u) {
        Update.Idle -> "Checks every few hours" to "Check"
        Update.Checking -> "Checking" to null
        Update.Current -> "Up to date" to "Check"
        is Update.Downloading -> "Downloading ${s.version}" to null
        is Update.Ready -> "${s.version} is ready" to "Install"
        is Update.Installing -> "Installing ${s.version}" to null
        is Update.Failed -> s.message to "Check"
    }
    Row(
        Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text("Updates", style = MaterialTheme.typography.bodyLarge, color = h.text1)
            Text(line, style = MaterialTheme.typography.bodySmall, color = if (u is Update.Failed) h.error else h.text3)
        }
        action?.let { label ->
            TextButton(onClick = { if (u is Update.Ready) app.updater.install() else app.updater.check(force = true) }) {
                Text(label, color = h.accent)
            }
        }
    }
}

@Composable
private fun Group(title: String, content: @Composable () -> Unit) {
    val h = LocalHues.current
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, Modifier.padding(start = 4.dp), style = MaterialTheme.typography.labelLarge, color = h.text3)
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.surface2)) { content() }
    }
}

@Composable
private fun Toggle(title: String, about: String, on: Boolean, onChange: (Boolean) -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet { onChange(!on) }.padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = h.text1)
            Text(about, style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = on,
            onCheckedChange = onChange,
            colors = SwitchDefaults.colors(
                checkedTrackColor = h.accent,
                checkedThumbColor = h.onAccent,
                uncheckedTrackColor = h.surface3,
                uncheckedThumbColor = h.text3,
                uncheckedBorderColor = h.border2,
            ),
        )
    }
}
