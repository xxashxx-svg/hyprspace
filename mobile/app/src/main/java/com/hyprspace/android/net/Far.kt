package com.hyprspace.android.net

/**
 * The way to a computer from any network (mobile/far): a port on this phone that leads to the
 * computer with an iroh key, reached with the same pinned TLS as any other address.
 */
object Far {
    private val loaded = runCatching { System.loadLibrary("hyprspace_far") }.isSuccess

    @JvmStatic private external fun port(key: String): Int

    /** The local port for [key], or null when this phone can't open one. */
    fun open(key: String): Int? = if (loaded) runCatching { port(key) }.getOrNull()?.takeIf { it > 0 } else null
}
