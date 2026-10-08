// Tells the user when an agent on the computer needs them or finishes, from the board's changes.
// Quiet about the thread they are looking at.

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
        val shown = viewing.takeIf { foreground }
        shown?.let { cancel(it) }
        if (before == null || !store.saved.value.notify) return
        for (t in b.threads) {
            val was = before[t.id] ?: continue
            if (t.id == shown || t.status == was) continue
            when {
                t.status == BoardStatus.Waiting -> post(t, t.doing ?: "Waiting for your answer.")
                t.status == BoardStatus.Done && (was == BoardStatus.Working || was == BoardStatus.Waiting) ->
                    post(t, "Finished.")
                t.status == BoardStatus.Failed && was == BoardStatus.Working -> post(t, "Stopped with an error.")
                else -> {}
            }
        }
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
