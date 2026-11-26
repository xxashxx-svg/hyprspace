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
import android.os.Build
import android.os.SystemClock
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
    private val asked = java.util.concurrent.ConcurrentHashMap<Long, Long>()
    private var last: Map<Long, BoardThread>? = null
    private var present = false

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
        val left = present && !b.present
        present = b.present
        last = b.threads.associateBy { it.id }
        val s = store.saved.value
        // back at the computer: what the phone said is on the screen there now
        if (b.present) cancelAll()
        if (before != null && foreground) {
            val now = SystemClock.elapsedRealtime()
            for (t in b.threads) {
                val old = before[t.id] ?: continue
                if (t.status == BoardStatus.Waiting && (old.status != BoardStatus.Waiting || t.ask != old.ask)) asked[t.id] = now
            }
        }
        if (before == null || !s.alerts || foreground || b.present) return
        for (t in b.threads) {
            // what asked while the user sat at the computer still needs them once they leave
            if (left && t.status == BoardStatus.Waiting) {
                post(t, t.doing ?: "Waiting for your answer.")
                continue
            }
            val old = before[t.id] ?: continue
            val was = old.status
            // one approval answered and the next asked inside a single board is news too
            val asked = t.status == BoardStatus.Waiting && t.ask != null && t.ask != old.ask
            if (t.status == was && !asked) continue
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

    // the process counts as in front for a moment after its screen is gone, and a thread that
    // asked in that moment was never seen
    fun left() {
        val now = SystemClock.elapsedRealtime()
        val late = asked.filterValues { now - it < 1500 }.keys
        asked.clear()
        if (!store.saved.value.alerts || present) return
        for (id in late) {
            val t = last?.get(id)?.takeIf { it.status == BoardStatus.Waiting } ?: continue
            post(t, t.doing ?: "Waiting for your answer.")
        }
    }

    /** The app came to the front: everything it would say is on its screen. */
    fun cancelAll() {
        val nm = NotificationManagerCompat.from(ctx)
        last?.keys?.forEach { nm.cancel(it.toInt()) }
    }

    fun cancel(thread: Long) = NotificationManagerCompat.from(ctx).cancel(thread.toInt())

    /** An answer tapped on a notification never reached the computer. */
    fun unsent(thread: Long) {
        last?.get(thread)?.let { post(it.copy(ask = null), "That didn't reach your computer. Open the thread to answer.") }
    }

    private fun answer(t: BoardThread, request: String, allow: Boolean): NotificationCompat.Action {
        val send = PendingIntent.getBroadcast(
            ctx,
            t.id.toInt() * 2 + if (allow) 1 else 0,
            Intent(ctx, Answers::class.java)
                .putExtra(Answers.THREAD, t.id)
                .putExtra(Answers.REQUEST, request)
                .putExtra(Answers.ALLOW, allow),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return NotificationCompat.Action.Builder(0, if (allow) "Allow" else "Deny", send)
            // letting an agent run something takes the phone's owner, not whoever holds it locked
            .setAuthenticationRequired(allow)
            .build()
    }

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
        t.ask?.takeIf { t.status == BoardStatus.Waiting && t.live }?.let { request ->
            n.addAction(answer(t, request, allow = false))
            // before Android 12 nothing asks for the unlock, so Allow stays inside the app there
            if (Build.VERSION.SDK_INT >= 31) n.addAction(answer(t, request, allow = true))
        }
        NotificationManagerCompat.from(ctx).notify(t.id.toInt(), n.build())
    }

    companion object {
        const val AGENTS = "agents"
        const val LINK = "link"
    }
}
