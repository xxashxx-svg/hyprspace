package com.hyprspace.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.hyprspace.android.net.PairLink
import com.hyprspace.android.ui.HyprApp
import com.hyprspace.android.ui.Screen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) handle(intent)
        setContent { HyprApp(application as App) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    /** A tapped notification opens its thread; a pairing link opens pairing with it filled in. */
    private fun handle(intent: Intent?) {
        val app = application as App
        val thread = intent?.getLongExtra(THREAD, -1L) ?: -1L
        if (thread >= 0) app.nav.go(Screen.Thread(thread))
        intent?.data?.toString()?.let(PairLink::parse)?.let {
            app.nav.link = it
            app.nav.go(Screen.Pair)
        }
    }

    companion object {
        const val THREAD = "thread"
    }
}
