package com.hyprspace.android.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.hyprspace.android.net.PairLink

sealed interface Screen {
    data object Home : Screen
    data class Thread(val id: Long) : Screen
    data object Settings : Screen
    /** A thread's folder: its files, and what git sees changed. */
    data class Files(val thread: Long) : Screen
    /** Pairing another computer, or the first one. */
    data object Pair : Screen
}

/** Where the app is. Kept by the process, so turning the phone doesn't send it home. */
class Nav {
    var screen by mutableStateOf<Screen>(Screen.Home)
        private set
    /** A pairing link that arrived from outside: a scan with the camera app, or a tap. */
    var link by mutableStateOf<PairLink?>(null)

    fun go(s: Screen) {
        screen = s
    }

    /** One step back. Returns false when there is nowhere left to go. */
    fun back(): Boolean {
        val s = screen
        if (s == Screen.Home) return false
        screen = if (s is Screen.Files) Screen.Thread(s.thread) else Screen.Home
        return true
    }
}
