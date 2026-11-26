// What a long press on the app's icon offers: a new thread, and the threads last worked in.

package com.hyprspace.android

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import com.hyprspace.android.net.Board
import com.hyprspace.android.net.BoardKind
import com.hyprspace.android.net.Shelf

class Shortcuts(private val ctx: Context) {
    private var last: List<Pair<Long, String>>? = null

    fun sync(board: Board?) {
        val recent = board?.threads.orEmpty()
            .filter { it.shelf == Shelf.Active && it.parent == null && it.title.isNotBlank() }
            .sortedByDescending { it.touched }
            .take(3)
        val now = recent.map { it.id to it.title }
        // Android counts how often an app sets these, and the board changes every second or so
        if (now == last) return
        last = now
        val open = { Intent(ctx, MainActivity::class.java).setAction(Intent.ACTION_VIEW) }
        val fresh = ShortcutInfoCompat.Builder(ctx, "new")
            .setShortLabel("New thread")
            .setIcon(IconCompat.createWithResource(ctx, R.drawable.ic_plus))
            .setIntent(open().putExtra(MainActivity.NEW, true))
            .build()
        val threads = recent.map { t ->
            val icon = if (t.kind == BoardKind.Terminal) R.drawable.ic_terminal else R.drawable.ic_sparkles
            ShortcutInfoCompat.Builder(ctx, "thread${t.id}")
                .setShortLabel(t.title.take(24))
                .setLongLabel(t.title.take(60))
                .setIcon(IconCompat.createWithResource(ctx, icon))
                .setIntent(open().putExtra(MainActivity.THREAD, t.id))
                .build()
        }
        runCatching {
            ShortcutManagerCompat.setDynamicShortcuts(ctx, if (board == null) emptyList() else listOf(fresh) + threads)
        }
    }
}
