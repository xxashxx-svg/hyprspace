package com.hyprspace.android.net

import java.net.URLDecoder
import kotlinx.serialization.Serializable

/**
 * What the desktop's pairing QR code holds: `hyprspace://pair?n=name&h=host,host&p=port&f=fp&c=secret`.
 * A code typed by hand has no fingerprint; the phone then trusts the first certificate it sees
 * and shows its security code to compare with the desktop's.
 */
data class PairLink(
    val name: String,
    val hosts: List<String>,
    val port: Int,
    val fingerprint: String?,
    val code: String,
) {
    companion object {
        fun parse(text: String): PairLink? {
            val t = text.trim()
            val prefix = "hyprspace://pair?"
            if (!t.startsWith(prefix, ignoreCase = true)) return null
            val q = t.substring(prefix.length).split('&').mapNotNull { kv ->
                val i = kv.indexOf('=')
                if (i <= 0) null else kv.substring(0, i) to URLDecoder.decode(kv.substring(i + 1), "UTF-8")
            }.toMap()
            return from(q["n"], q["h"], q["p"], q["f"], q["c"])
        }

        /** The same fields from wherever they came; null when any that matter is missing. */
        fun from(name: String?, hosts: String?, port: String?, fingerprint: String?, code: String?): PairLink? {
            val h = hosts?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }.orEmpty()
            val p = port?.toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
            val c = code?.trim()?.takeIf { it.isNotEmpty() } ?: return null
            if (h.isEmpty()) return null
            return PairLink(name?.takeIf { it.isNotBlank() } ?: h.first(), h, p, fingerprint?.takeIf { it.isNotBlank() }, c)
        }

        /** "192.168.1.20:47821" or a bare host, as typed on the manual pairing screen. */
        fun typed(address: String, code: String): PairLink? {
            val a = address.trim().removePrefix("https://").removePrefix("wss://").trimEnd('/')
            val host = a.substringBeforeLast(':', a).trim('[', ']')
            val port = a.substringAfterLast(':', "").ifEmpty { "47821" }
            return from(null, host, port, null, code)
        }
    }
}

/** A computer this phone paired with. */
@Serializable
data class Desktop(
    /** The certificate's fingerprint, which also names the desktop here. */
    val id: String,
    val name: String,
    val hosts: List<String>,
    val port: Int,
    val token: String,
    val paired: Long,
    /** The address that answered last, tried first. */
    val last: String? = null,
) {
    /** Hosts in the order to try them. */
    fun order(): List<String> = (listOfNotNull(last) + hosts).distinct()

    /** Eight characters of the fingerprint, to compare with the desktop's Settings. */
    fun securityCode(): String = id.filter { it.isLetterOrDigit() }.take(8).uppercase().chunked(4).joinToString("-")
}
