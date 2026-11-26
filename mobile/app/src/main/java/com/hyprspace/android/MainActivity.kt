package com.hyprspace.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.content.IntentCompat
import com.hyprspace.android.net.PairLink
import com.hyprspace.android.ui.HyprApp
import com.hyprspace.android.ui.Screen
import com.hyprspace.android.ui.Shared

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

    /**
     * A tapped notification or shortcut opens its thread, a pairing link opens pairing with it
     * filled in, and what another app shared waits on the home screen for a thread to take it.
     */
    private fun handle(intent: Intent?) {
        val app = application as App
        val thread = intent?.getLongExtra(THREAD, -1L) ?: -1L
        if (thread >= 0) app.nav.go(Screen.Thread(thread))
        if (intent?.getBooleanExtra(NEW, false) == true) {
            app.nav.go(Screen.Home)
            app.nav.starting = true
        }
        if (intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE) {
            val files = if (intent.action == Intent.ACTION_SEND) {
                listOfNotNull(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
            } else {
                IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
            }
            val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString().orEmpty()
            if (text.isNotBlank() || files.isNotEmpty()) {
                app.nav.shared = Shared(text.trim(), files.take(4))
                app.nav.go(Screen.Home)
            }
            return
        }
        intent?.data?.toString()?.let(PairLink::parse)?.let {
            app.nav.link = it
            app.nav.go(Screen.Pair)
        }
    }

    companion object {
        const val THREAD = "thread"
        const val NEW = "new"
    }
}
