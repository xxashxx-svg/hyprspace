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
    /** Tell the user when an agent needs them or finishes. */
    val notify: Boolean = true,
    /** Stay connected in the background, which notifications need. */
    val stay: Boolean = true,
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
