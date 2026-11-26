// What the phone keeps about threads for itself: the message half typed in each, and which
// finished ones were read here. The computer's own unseen mark only clears when the thread is
// opened there, so the phone remembers what it showed. Keyed by computer and thread, since
// every computer numbers its threads from the start.

package com.hyprspace.android.data

import android.content.Context
import androidx.compose.runtime.mutableStateMapOf
import androidx.core.content.edit
import com.hyprspace.android.net.BoardThread

class Notes(context: Context, private val store: Store) {
    private val prefs = context.getSharedPreferences("notes", Context.MODE_PRIVATE)
    /** A thread's `touched` when it was last read here. */
    private val read = mutableStateMapOf<String, Long>()
    private val flagged = mutableStateMapOf<String, Boolean>()

    init {
        for ((k, v) in prefs.all) {
            when {
                k.startsWith(READ) && v is Long -> read[k.removePrefix(READ)] = v
                k.startsWith(FLAG) -> flagged[k.removePrefix(FLAG)] = true
            }
        }
    }

    private fun key(thread: Long) = "${store.saved.value.current()?.id}/$thread"

    fun unread(t: BoardThread): Boolean {
        val k = key(t.id)
        return flagged[k] == true || (t.unseen && read[k] != t.touched)
    }

    fun markRead(t: BoardThread) {
        val k = key(t.id)
        if (read[k] == t.touched && flagged[k] != true) return
        read[k] = t.touched
        flagged.remove(k)
        prefs.edit { putLong(READ + k, t.touched).remove(FLAG + k) }
    }

    fun markUnread(thread: Long) {
        val k = key(thread)
        flagged[k] = true
        prefs.edit { putBoolean(FLAG + k, true) }
    }

    fun draft(thread: Long): String = prefs.getString(DRAFT + key(thread), "").orEmpty()

    fun draft(thread: Long, text: String) {
        val k = DRAFT + key(thread)
        if (text == prefs.getString(k, "").orEmpty()) return
        prefs.edit { if (text.isBlank()) remove(k) else putString(k, text) }
    }

    /** Drops what was kept for threads the current computer no longer has. */
    fun prune(threads: Set<Long>) {
        val mine = "${store.saved.value.current()?.id}/"
        val gone = prefs.all.keys.filter { k ->
            val rest = k.substringAfter(':')
            rest.startsWith(mine) && rest.removePrefix(mine).toLongOrNull()?.let { it !in threads } == true
        }
        if (gone.isEmpty()) return
        prefs.edit { gone.forEach(::remove) }
        for (k in gone) {
            read.remove(k.substringAfter(':'))
            flagged.remove(k.substringAfter(':'))
        }
    }

    private companion object {
        const val READ = "read:"
        const val FLAG = "flag:"
        const val DRAFT = "draft:"
    }
}
