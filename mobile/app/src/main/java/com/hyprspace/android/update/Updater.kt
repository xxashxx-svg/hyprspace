package com.hyprspace.android.update

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.os.Build
import com.hyprspace.android.BuildConfig
import com.hyprspace.android.net.wire
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import okhttp3.OkHttpClient
import okhttp3.Request

sealed interface Update {
    data object Idle : Update
    data object Checking : Update
    data object Current : Update
    data class Downloading(val version: String) : Update
    data class Ready(val version: String, val file: File) : Update
    data class Installing(val version: String) : Update
    data class Failed(val message: String) : Update
}

data class Release(val version: String, val url: String, val size: Long, val sha256: String?)

object Releases {
    const val LATEST = "https://api.github.com/repos/xxashxx-svg/hyprspace/releases/latest"
    const val APK = "HyprSpace-android.apk"

    fun parse(json: String): Release? {
        val v = wire.parseToJsonElement(json).jsonObject
        val tag = v["tag_name"]?.jsonPrimitive?.content ?: return null
        val asset = v["assets"]?.jsonArray?.map { it.jsonObject }?.firstOrNull { it.text("name") == APK } ?: return null
        return Release(
            version = tag.removePrefix("v"),
            url = asset.text("browser_download_url") ?: return null,
            size = asset["size"]?.jsonPrimitive?.longOrNull ?: 0,
            sha256 = asset.text("digest")?.takeIf { it.startsWith("sha256:") }?.removePrefix("sha256:"),
        )
    }

    fun newer(a: String, than: String): Boolean {
        val x = parts(a)
        val y = parts(than)
        for (i in 0 until maxOf(x.size, y.size)) {
            val d = x.getOrElse(i) { 0 } - y.getOrElse(i) { 0 }
            if (d != 0) return d > 0
        }
        return false
    }

    private fun parts(v: String) = v.substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 }

    private fun JsonObject.text(key: String) = this[key]?.jsonPrimitive?.content
}

class Updater(private val context: Context, private val scope: CoroutineScope) {
    private val _state = MutableStateFlow<Update>(Update.Idle)
    val state: StateFlow<Update> = _state
    val enabled = !BuildConfig.DEBUG
    private val dir = File(context.cacheDir, "updates")
    private val http = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()
    @Volatile private var checked = 0L

    fun check(force: Boolean = false) {
        if (!enabled) return
        val busy = _state.value
        if (busy is Update.Checking || busy is Update.Downloading || busy is Update.Installing || busy is Update.Ready) return
        val now = System.currentTimeMillis()
        if (!force && now - checked < EVERY) return
        checked = now
        if (force) _state.value = Update.Checking
        scope.launch(Dispatchers.IO) {
            _state.value = runCatching { fetch() }.getOrElse { Update.Failed("Couldn't check for updates. ${it.message.orEmpty()}".trim()) }
        }
    }

    private fun fetch(): Update {
        val body = http.newCall(Request.Builder().url(Releases.LATEST).header("Accept", "application/vnd.github+json").build())
            .execute().use { r ->
                if (!r.isSuccessful) error("GitHub answered ${r.code}.")
                r.body.string()
            }
        val release = Releases.parse(body) ?: return Update.Current
        if (!Releases.newer(release.version, BuildConfig.VERSION_NAME)) {
            dir.deleteRecursively()
            return Update.Current
        }
        val file = File(dir, "HyprSpace-${release.version}.apk")
        if (!(file.exists() && file.length() == release.size && matches(file, release.sha256))) {
            _state.value = Update.Downloading(release.version)
            dir.deleteRecursively()
            dir.mkdirs()
            val part = File(dir, "download.part")
            http.newCall(Request.Builder().url(release.url).build()).execute().use { r ->
                if (!r.isSuccessful) error("GitHub answered ${r.code}.")
                part.outputStream().use { out -> r.body.byteStream().copyTo(out) }
            }
            if (!matches(part, release.sha256)) {
                part.delete()
                return Update.Failed("The download didn't match the release. It'll try again later.")
            }
            part.renameTo(file)
        }
        if (!sameSigner(file)) {
            return Update.Failed("This install can't update itself. Uninstall it once and install the APK from GitHub, then updates come on their own.")
        }
        return Update.Ready(release.version, file)
    }

    private fun matches(file: File, sha256: String?): Boolean {
        sha256 ?: return true
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(64 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                digest.update(buf, 0, n)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }.equals(sha256, ignoreCase = true)
    }

    @Suppress("DEPRECATION")
    private fun sameSigner(file: File): Boolean {
        val pm = context.packageManager
        val flags = PackageManager.GET_SIGNING_CERTIFICATES
        val theirs = pm.getPackageArchiveInfo(file.path, flags) ?: return false
        if (theirs.packageName != context.packageName) return false
        val mine = pm.getPackageInfo(context.packageName, flags)
        val a = mine.signingInfo?.apkContentsSigners?.map { it.toCharsString() }?.toSet()
        val b = theirs.signingInfo?.apkContentsSigners?.map { it.toCharsString() }?.toSet()
        return a != null && a == b
    }

    fun install(quiet: Boolean = false) {
        val ready = _state.value as? Update.Ready ?: return
        if (quiet && Build.VERSION.SDK_INT < 31) return
        _state.value = Update.Installing(ready.version)
        scope.launch(Dispatchers.IO) {
            runCatching {
                val installer = context.packageManager.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                    setAppPackageName(context.packageName)
                    if (Build.VERSION.SDK_INT >= 31) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
                }
                val id = installer.createSession(params)
                installer.openSession(id).use { session ->
                    session.openWrite("base.apk", 0, ready.file.length()).use { out ->
                        ready.file.inputStream().use { it.copyTo(out) }
                        session.fsync(out)
                    }
                    val intent = Intent(context, Installed::class.java).putExtra(Installed.QUIET, quiet)
                    // the installer fills in the status, so this one has to stay mutable
                    val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
                    session.commit(PendingIntent.getBroadcast(context, id, intent, flags).intentSender)
                }
            }.onFailure { _state.value = Update.Failed("The update didn't install. ${it.message.orEmpty()}".trim()) }
        }
    }

    internal fun held() {
        val s = _state.value as? Update.Installing ?: return
        val file = File(dir, "HyprSpace-${s.version}.apk")
        _state.value = if (file.exists()) Update.Ready(s.version, file) else Update.Idle
    }

    internal fun failed(message: String?) {
        held()
        if (_state.value !is Update.Ready) _state.value = Update.Failed("The update didn't install. ${message.orEmpty()}".trim())
    }

    companion object {
        private const val EVERY = 6 * 60 * 60 * 1000L
    }
}
