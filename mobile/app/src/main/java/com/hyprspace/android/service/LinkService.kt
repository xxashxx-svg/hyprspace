// Keeps the app's process, and with it the connection, alive in the background, so the user
// hears when an agent needs them. Android only lets a foreground service do that, so it shows a
// quiet notification that says where it's connected. Switched off in Settings, the connection
// closes a minute after the app leaves the screen.

package com.hyprspace.android.service

import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.hyprspace.android.App
import com.hyprspace.android.MainActivity
import com.hyprspace.android.R
import com.hyprspace.android.net.Conn
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

class LinkService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val app = application as App
        show(note(app.link.conn.value))
        scope.launch { app.link.conn.collect { show(note(it)) } }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        (application as App).link.start()
        return START_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun note(c: Conn): String = when (c) {
        is Conn.Online -> "Connected to ${c.desktop}"
        Conn.Connecting -> "Connecting"
        is Conn.Offline -> "Reconnecting"
        is Conn.Denied -> c.message
        Conn.None -> "Not paired"
    }

    private fun show(text: String) {
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val n = NotificationCompat.Builder(this, Notifier.LINK)
            .setSmallIcon(R.drawable.ic_notify)
            .setContentTitle(text)
            .setOngoing(true)
            .setSilent(true)
            .setContentIntent(open)
            .build()
        // the special-use type only exists from Android 14; before that the manifest's type is enough
        val type = if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE else 0
        ServiceCompat.startForeground(this, ID, n, type)
    }

    companion object {
        private const val ID = 0x4859

        /** Runs the service when the settings and pairing call for it, and stops it otherwise. */
        fun sync(ctx: Context) {
            val app = ctx.applicationContext as App
            val s = app.store.saved.value
            val want = s.stay && s.current() != null
            val intent = Intent(ctx, LinkService::class.java)
            if (want) {
                runCatching { ContextCompat.startForegroundService(ctx, intent) }
            } else {
                ctx.stopService(intent)
            }
        }
    }
}
