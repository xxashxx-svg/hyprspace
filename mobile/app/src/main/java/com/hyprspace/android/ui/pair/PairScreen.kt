// Pairing: scan the QR code from the computer's Settings, Phone, or type its address and code.
// A pairing link that came from outside (the camera app) is shown to confirm first.

package com.hyprspace.android.ui.pair

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.net.Conn
import com.hyprspace.android.net.PairLink
import com.hyprspace.android.service.LinkService
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import com.hyprspace.android.ui.Screen
import kotlinx.coroutines.launch

private enum class Mode { Intro, Scan, Type }

@Composable
fun PairScreen(app: App, onDone: () -> Unit, onCancel: (() -> Unit)?) {
    val h = LocalHues.current
    val scope = rememberCoroutineScope()
    var mode by remember { mutableStateOf(Mode.Intro) }
    var busy by remember { mutableStateOf<String?>(null) }
    // the computer removed this phone: say so where it pairs again
    var error by remember { mutableStateOf((app.link.conn.value as? Conn.Denied)?.message) }
    val incoming = app.nav.link

    fun pair(link: PairLink) {
        app.nav.go(Screen.Pair)
        busy = "Pairing with ${link.name}"
        error = null
        scope.launch {
            val r = app.link.pair(link)
            busy = null
            r.onSuccess { d ->
                app.link.restart()
                LinkService.sync(app)
                onDone()
            }.onFailure {
                error = it.message
                mode = Mode.Intro
            }
        }
    }

    Column(
        Modifier
            .fillMaxSize()
            .statusBarsPadding()
            .navigationBarsPadding()
            .imePadding(),
    ) {
        Row(Modifier.fillMaxWidth().padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
            if (onCancel != null || mode != Mode.Intro) {
                IconButton(onClick = { if (mode != Mode.Intro) mode = Mode.Intro else onCancel?.invoke() }) {
                    Icon(painterResource(R.drawable.ic_arrow_left), "Back", Modifier.size(20.dp), tint = h.text1)
                }
            }
        }
        when {
            busy != null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp, color = h.text2)
                    Text(busy.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = h.text2)
                }
            }
            mode == Mode.Scan -> Box(Modifier.fillMaxSize()) {
                Scanner(onFound = { pair(it) }, modifier = Modifier.fillMaxSize().background(Color.Black))
                Text(
                    "Point the camera at the code in Settings, Phone on your computer.",
                    Modifier.align(Alignment.BottomCenter).padding(28.dp).clip(RoundedCornerShape(10.dp))
                        .background(Color.Black.copy(alpha = 0.55f)).padding(horizontal = 14.dp, vertical = 10.dp),
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color.White,
                )
            }
            mode == Mode.Type -> Typed(onPair = ::pair)
            incoming != null -> Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Cube(56)
                Text("Pair with ${incoming.name}?", style = MaterialTheme.typography.headlineSmall, color = h.text1)
                Text(
                    "This phone will see that computer's threads and can work in them. You can unpair from either side at any time.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = h.text2,
                )
                error?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = h.error) }
                Primary("Pair") { pair(incoming) }
            }
            else -> Intro(error, onScan = { mode = Mode.Scan }, onType = { mode = Mode.Type })
        }
    }
}

@Composable
private fun Intro(error: String?, onScan: () -> Unit, onType: () -> Unit) {
    val h = LocalHues.current
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Spacer(Modifier.height(24.dp))
        Cube(64)
        Text("Your threads, on your phone", style = MaterialTheme.typography.headlineSmall, color = h.text1)
        Text(
            "Pair this phone with HyprSpace on your computer to follow your agents, answer them and start new work. It talks to your computer directly, over your network or Tailscale.",
            style = MaterialTheme.typography.bodyLarge,
            color = h.text2,
        )
        Steps()
        error?.let {
            Text(
                it,
                Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(h.error.copy(alpha = 0.1f)).padding(12.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = h.error,
            )
        }
        Primary("Scan the code", onClick = onScan)
        OutlinedButton(
            onClick = onType,
            modifier = Modifier.fillMaxWidth().height(48.dp),
            shape = RoundedCornerShape(12.dp),
        ) { Text("Type the code instead", color = h.text1) }
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun Steps() {
    val h = LocalHues.current
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.surface2).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        listOf(
            "On your computer, open HyprSpace and go to Settings, Phone.",
            "Switch on Let your phone connect, then press Show code.",
            "Scan the code with this phone.",
        ).forEachIndexed { i, s ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("${i + 1}", fontFamily = Mono, fontWeight = FontWeight.SemiBold, color = h.accent)
                Text(s, style = MaterialTheme.typography.bodyMedium, color = h.text1)
            }
        }
    }
}

@Composable
private fun Typed(onPair: (PairLink) -> Unit) {
    val h = LocalHues.current
    var address by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    val link = PairLink.typed(address, code)
    val colors = OutlinedTextFieldDefaults.colors(
        focusedBorderColor = h.border2,
        unfocusedBorderColor = h.border1,
        focusedContainerColor = h.surface1,
        unfocusedContainerColor = h.surface1,
    )
    Column(Modifier.padding(horizontal = 24.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Text("Type the code", style = MaterialTheme.typography.headlineSmall, color = h.text1)
        Text(
            "Settings, Phone on your computer shows the address and the code under the QR code.",
            style = MaterialTheme.typography.bodyMedium,
            color = h.text2,
        )
        OutlinedTextField(
            value = address,
            onValueChange = { address = it.trim() },
            label = { Text("Address") },
            placeholder = { Text("192.168.1.20:47821") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            colors = colors,
        )
        OutlinedTextField(
            value = code,
            onValueChange = { if (it.length <= 16) code = it },
            label = { Text("Code") },
            placeholder = { Text("K7MX-Q2RT-H9WP") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
            textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = Mono),
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            colors = colors,
        )
        Primary("Pair", enabled = link != null && code.count { it.isLetterOrDigit() } == 12) { link?.let(onPair) }
    }
}

@Composable
private fun Primary(text: String, enabled: Boolean = true, onClick: () -> Unit) {
    val h = LocalHues.current
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.fillMaxWidth().height(48.dp),
        shape = RoundedCornerShape(12.dp),
        colors = ButtonDefaults.buttonColors(containerColor = h.accent, contentColor = h.onAccent),
    ) { Text(text, fontWeight = FontWeight.SemiBold) }
}

/** HyprSpace's cube, the mark on the desktop's title bar. */
@Composable
fun Cube(size: Int) {
    val h = LocalHues.current
    Canvas(Modifier.size(size.dp)) {
        val s = this.size.width / 24f
        fun p(vararg pts: Float) = Path().apply {
            moveTo(pts[0] * s, pts[1] * s)
            for (i in 2 until pts.size step 2) lineTo(pts[i] * s, pts[i + 1] * s)
            close()
        }
        drawPath(p(12f, 3f, 20.5f, 8f, 12f, 13f, 3.5f, 8f), Color(0xFF3D54E8))
        drawPath(p(3.5f, 8f, 12f, 13f, 12f, 21f, 3.5f, 16f), h.text1)
        drawPath(p(20.5f, 8f, 20.5f, 16f, 12f, 21f, 12f, 13f), h.text3)
    }
}
