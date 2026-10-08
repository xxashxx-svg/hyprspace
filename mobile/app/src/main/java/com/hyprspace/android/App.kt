package com.hyprspace.android

import android.app.Application
import android.net.ConnectivityManager
import android.net.Network
import android.provider.Settings
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import com.hyprspace.android.data.Store
import com.hyprspace.android.net.Link
import com.hyprspace.android.service.LinkService
import com.hyprspace.android.service.Notifier
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class App : Application() {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    lateinit var store: Store
        private set
    lateinit var link: Link
        private set
    lateinit var notifier: Notifier
        private set
    val nav = com.hyprspace.android.ui.Nav()
    private var pause: Job? = null

    override fun onCreate() {
        super.onCreate()
        store = Store(this, scope)
        link = Link(store, scope)
        notifier = Notifier(this, store)
        Link.deviceName = {
            Settings.Global.getString(contentResolver, Settings.Global.DEVICE_NAME)
                ?: android.os.Build.MODEL
        }
        scope.launch { link.board.collect { b -> b?.let(notifier::board) } }
        getSystemService(ConnectivityManager::class.java).registerDefaultNetworkCallback(
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) = link.nudge()
            },
        )
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                notifier.foreground = true
                pause?.cancel()
                if (store.saved.value.current() != null) link.start()
                link.nudge()
                LinkService.sync(this@App)
            }

            override fun onStop(owner: LifecycleOwner) {
                notifier.foreground = false
                if (!store.saved.value.stay) {
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
