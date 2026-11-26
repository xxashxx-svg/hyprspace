package com.hyprspace.android

import android.app.Application
import android.net.ConnectivityManager
import android.net.Network
import android.provider.Settings
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import com.hyprspace.android.data.Notes
import com.hyprspace.android.data.Store
import com.hyprspace.android.net.Link
import com.hyprspace.android.service.LinkService
import com.hyprspace.android.service.Notifier
import com.hyprspace.android.update.Updater
import com.hyprspace.android.widget.UsageWidget
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

class App : Application() {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    lateinit var store: Store
        private set
    lateinit var link: Link
        private set
    lateinit var notifier: Notifier
        private set
    lateinit var updater: Updater
        private set
    lateinit var notes: Notes
        private set
    private lateinit var shortcuts: Shortcuts
    val nav = com.hyprspace.android.ui.Nav()
    private var pause: Job? = null
    private var quiet: Job? = null

    override fun onCreate() {
        super.onCreate()
        store = Store(this, scope)
        link = Link(store, scope)
        notifier = Notifier(this, store)
        updater = Updater(this, scope)
        notes = Notes(this, store)
        shortcuts = Shortcuts(this)
        link.finder = { fingerprint -> com.hyprspace.android.net.find(this, fingerprint) }
        Link.deviceName = {
            Settings.Global.getString(contentResolver, Settings.Global.DEVICE_NAME)
                ?: android.os.Build.MODEL
        }
        scope.launch {
            link.board.collect { b ->
                b ?: return@collect
                notifier.board(b)
                shortcuts.sync(b)
                notes.prune(b.threads.map { it.id }.toSet())
                val now = System.currentTimeMillis()
                val kept = store.saved.value
                // the same numbers still move the widget's time on, but not on every board
                if (b.usage.isNotEmpty() && (b.usage != kept.usage || now - kept.usageAt > 5 * 60_000)) {
                    store.update { it.copy(usage = b.usage, usageAt = now) }
                    UsageWidget.refresh(this@App)
                }
            }
        }
        // a computer that removed this phone takes the background connection with it
        scope.launch {
            store.saved.map { it.current() != null }.distinctUntilChanged().drop(1).collect { paired ->
                LinkService.sync(this@App)
                if (!paired) shortcuts.sync(null)
            }
        }
        getSystemService(ConnectivityManager::class.java).registerDefaultNetworkCallback(
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = link.nudge()
            },
        )
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                notifier.foreground = true
                link.awake = true
                notifier.cancelAll()
                pause?.cancel()
                quiet?.cancel()
                updater.check()
                if (store.saved.value.current() != null) link.start()
                link.nudge()
                LinkService.sync(this@App)
            }

            override fun onStop(owner: LifecycleOwner) {
                notifier.foreground = false
                notifier.left()
                link.awake = false
                quiet = scope.launch {
                    delay(60_000)
                    updater.install(quiet = true)
                }
                if (!store.saved.value.alerts) {
                    // a quick trip to another app keeps the line; a longer one lets it go
                    pause = scope.launch {
                        delay(60_000)
                        link.stop()
                    }
                }
            }
        })
    }
}
