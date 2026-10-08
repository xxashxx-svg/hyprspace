// Tells the user when an agent on the computer needs them, and if they asked, when a run ends,
// from the board's changes. Only when it's news: never while the app is open, and never while
// someone is at the computer, where the desktop app already shows it.

package com.hyprspace.android.service

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.hyprspace.android.MainActivity
import com.hyprspace.android.R
import com.hyprspace.android.data.Store
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.BoardStatus
import com.hyprspace.android.net.BoardThread

class Notifier(private val ctx: Context, private val store: Store) {
    @Volatile var foreground = false
    /** The thread on screen, if one is. */
    @Volatile var viewing: Long? = null
    private var last: Map<Long, BoardStatus>? = null

    init {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(AGENTS, "Agents", NotificationManager.IMPORTANCE_HIGH).apply {
                description = "When an agent needs you or finishes."
            },
        )
        nm.createNotificationChannel(
            NotificationChannel(LINK, "Connection", NotificationManager.IMPORTANCE_MIN).apply {
                description = "Shows while the app stays connected to your computer."
                setShowBadge(false)
            },
        )
    }

    fun board(b: Board) {
        val before = last
        last = b.threads.associate { it.id to it.status }
        val s = store.saved.value
        // back at the computer: what the phone said is on the screen there now
        if (b.present) cancelAll()
        if (before == null || !s.alerts || foreground || b.present) return
        for (t in b.threads) {
            val was = before[t.id] ?: continue
            if (t.status == was) continue
            // a thread that stops waiting no longer needs the user
            if (was == BoardStatus.Waiting) cancel(t.id)
            when {
                t.status == BoardStatus.Waiting -> post(t, t.doing ?: "Waiting for your answer.")
                !s.finished -> {}
                t.status == BoardStatus.Done && (was == BoardStatus.Working || was == BoardStatus.Waiting) ->
                    post(t, "Finished.")
                t.status == BoardStatus.Failed && was == BoardStatus.Working -> post(t, "Stopped with an error.")
                else -> {}
            }
        }
    }

    /** The app came to the front: everything it would say is on its screen. */
    fun cancelAll() {
        val nm = NotificationManagerCompat.from(ctx)
        last?.keys?.forEach { nm.cancel(it.toInt()) }
    }

    fun cancel(thread: Long) = NotificationManagerCompat.from(ctx).cancel(thread.toInt())

    private fun post(t: BoardThread, text: String) {
        if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) return
        val open = PendingIntent.getActivity(
            ctx,
            t.id.toInt(),
            Intent(ctx, MainActivity::class.java)
                .putExtra(MainActivity.THREAD, t.id)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val n = NotificationCompat.Builder(ctx, AGENTS)
            .setSmallIcon(R.drawable.ic_notify)
            .setContentTitle(t.title.ifBlank { "Thread" })
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .setAutoCancel(true)
            .setContentIntent(open)
            .build()
        NotificationManagerCompat.from(ctx).notify(t.id.toInt(), n)
    }

    companion object {
        const val AGENTS = "agents"
        const val LINK = "link"
    }
}
