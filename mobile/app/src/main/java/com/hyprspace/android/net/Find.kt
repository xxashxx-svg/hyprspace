// Finding the paired computer on the local network when the address it was saved under stops
// answering: the router handed it a new one, or the bridge had to move to another port. The
// desktop announces itself over mDNS with its certificate's fingerprint (engine/src/phone/net.rs),
// so the phone knows which announcement is its computer, and the pin still checks the rest.

package com.hyprspace.android.net

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume

private const val SERVICE = "_hyprspace._tcp."

/** Where the computer with this fingerprint says it is: its addresses and port. */
data class Found(val hosts: List<String>, val port: Int)

/** Looks for up to [wait] ms. Null when it isn't heard from. */
suspend fun find(ctx: Context, fingerprint: String, wait: Long = 5_000): Found? {
    val nsd = ctx.getSystemService(NsdManager::class.java) ?: return null
    val seen = Channel<NsdServiceInfo>(Channel.UNLIMITED)
    val listener = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) {
            seen.trySend(info)
        }
        override fun onServiceLost(info: NsdServiceInfo) {}
        override fun onDiscoveryStarted(type: String) {}
        override fun onDiscoveryStopped(type: String) {}
        override fun onStartDiscoveryFailed(type: String, code: Int) {
            seen.close()
        }
        override fun onStopDiscoveryFailed(type: String, code: Int) {}
    }
    runCatching { nsd.discoverServices(SERVICE, NsdManager.PROTOCOL_DNS_SD, listener) }.onFailure { return null }
    try {
        return withTimeoutOrNull(wait) {
            for (info in seen) {
                // only one resolve runs at a time, so each waits for the last
                val full = resolve(nsd, info) ?: continue
                val f = full.attributes["f"]?.toString(Charsets.UTF_8)
                if (f != fingerprint) continue
                val hosts = hosts(full)
                if (hosts.isNotEmpty() && full.port > 0) return@withTimeoutOrNull Found(hosts, full.port)
            }
            null
        }
    } finally {
        runCatching { nsd.stopServiceDiscovery(listener) }
    }
}

@Suppress("DEPRECATION")
private fun hosts(info: NsdServiceInfo): List<String> {
    val all = if (android.os.Build.VERSION.SDK_INT >= 34) info.hostAddresses else listOfNotNull(info.host)
    // IPv4 first: link-local IPv6 needs a scope the URL can't carry
    return all.sortedBy { it is java.net.Inet6Address }
        .filterNot { it.isLinkLocalAddress || it.isLoopbackAddress }
        .mapNotNull { it.hostAddress }
}

@Suppress("DEPRECATION")
private suspend fun resolve(nsd: NsdManager, info: NsdServiceInfo): NsdServiceInfo? =
    suspendCancellableCoroutine { cont ->
        val l = object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, code: Int) {
                if (cont.isActive) cont.resume(null)
            }
            override fun onServiceResolved(info: NsdServiceInfo) {
                if (cont.isActive) cont.resume(info)
            }
        }
        runCatching { nsd.resolveService(info, l) }.onFailure { if (cont.isActive) cont.resume(null) }
    }
