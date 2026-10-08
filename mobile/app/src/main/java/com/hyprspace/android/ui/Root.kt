package com.hyprspace.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.hyprspace.android.App
import com.hyprspace.android.ui.home.HomeScreen
import com.hyprspace.android.ui.pair.PairScreen
import com.hyprspace.android.ui.settings.SettingsScreen
import com.hyprspace.android.ui.thread.ThreadScreen

@Composable
fun HyprApp(app: App) {
    val saved by app.store.saved.collectAsStateWithLifecycle()
    val board by app.link.board.collectAsStateWithLifecycle()
    HyprTheme(board?.theme ?: saved.theme) {
        val h = LocalHues.current
        val snack = remember { SnackbarHostState() }
        LaunchedEffect(Unit) { app.link.failures.collect { snack.showSnackbar(it) } }
        val paired = saved.desktops.isNotEmpty()
        Box(Modifier.fillMaxSize().background(h.bg)) {
            val nav = app.nav
            val screen = nav.screen
            if (!paired || screen == Screen.Pair) {
                BackHandler(enabled = paired) { nav.back() }
                PairScreen(app, onDone = { nav.link = null; nav.go(Screen.Home) }, onCancel = if (paired) ({ nav.back() }) else null)
            } else {
                BackHandler(enabled = screen != Screen.Home) { nav.back() }
                when (screen) {
                    is Screen.Thread -> ThreadScreen(app, screen.id, onBack = { nav.back() })
                    Screen.Settings -> SettingsScreen(app, onBack = { nav.back() }, onPair = { nav.go(Screen.Pair) })
                    else -> HomeScreen(
                        app,
                        onOpen = { nav.go(Screen.Thread(it)) },
                        onSettings = { nav.go(Screen.Settings) },
                        onPair = { nav.go(Screen.Pair) },
                    )
                }
            }
            SnackbarHost(
                snack,
                Modifier.align(Alignment.BottomCenter).navigationBarsPadding().imePadding().padding(bottom = 72.dp),
            ) { data ->
                Snackbar(containerColor = h.surface3, contentColor = h.text1) { Text(data.visuals.message) }
            }
        }
    }
}
