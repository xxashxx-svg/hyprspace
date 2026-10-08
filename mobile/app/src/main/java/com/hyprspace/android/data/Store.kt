// What the phone keeps: the computers it paired with, which one it talks to, its own settings,
// and the last theme a desktop sent, so the app opens in the right colors before it connects.
// One JSON blob in DataStore, small enough to read at start.

package com.hyprspace.android.data

import android.content.Context
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import com.hyprspace.android.net.BoardTheme
import com.hyprspace.android.net.Desktop
import com.hyprspace.android.net.wire
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.Serializable

@Serializable
data class Saved(
    val desktops: List<Desktop> = emptyList(),
    val active: String? = null,
    /**
     * Notifications, off until switched on. On, the app stays connected in the background, which
     * is the only way it hears from the computer there.
     */
    val alerts: Boolean = false,
    /** Notify when a run finishes too, not only when an agent needs an answer. */
    val finished: Boolean = false,
    /** Ask for a terminal sized to the phone when it opens. */
    val fit: Boolean = true,
    val theme: BoardTheme? = null,
) {
    fun current(): Desktop? = desktops.firstOrNull { it.id == active } ?: desktops.firstOrNull()
}

class Store(context: Context, private val scope: CoroutineScope) {
    private val ds = PreferenceDataStoreFactory.create { context.preferencesDataStoreFile("hyprspace") }
    private val key = stringPreferencesKey("saved")
    private val state = MutableStateFlow(load())
    val saved: StateFlow<Saved> = state

    private fun load(): Saved = runBlocking {
        val json = ds.data.first()[key] ?: return@runBlocking Saved()
        runCatching { wire.decodeFromString(Saved.serializer(), json) }.getOrDefault(Saved())
    }

    fun update(f: (Saved) -> Saved) {
        val next = synchronized(this) {
            val n = f(state.value)
            if (n == state.value) return
            state.value = n
            n
        }
        scope.launch { ds.edit { it[key] = wire.encodeToString(Saved.serializer(), next) } }
    }

    fun remember(d: Desktop) = update { s ->
        s.copy(desktops = s.desktops.filter { it.id != d.id } + d, active = d.id)
    }

    fun forget(id: String) = update { s ->
        val left = s.desktops.filter { it.id != id }
        s.copy(desktops = left, active = if (s.active == id) left.firstOrNull()?.id else s.active)
    }
}
