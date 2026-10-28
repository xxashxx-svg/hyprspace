// Images and videos a reply points at with markdown's ![alt](path). The file lives on the computer,
// so it comes over the connection once (`Link.fetch`), and is kept for the rest of the session.
// Images show in the reply; a video shows a tile and plays full screen in the phone's own player.

package com.hyprspace.android.ui.thread

import android.graphics.BitmapFactory
import android.widget.MediaController
import android.widget.VideoView
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.hyprspace.android.R
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Where a thread's replies get their media: its folder, for relative paths, and the fetch. */
class MediaSource(val cwd: String, val fetch: suspend (String, File) -> Result<File>) {
    fun resolve(src: String): String {
        val path = src.removePrefix("file://")
        val absolute = path.startsWith("/") || path.startsWith("\\") || Regex("^[A-Za-z]:[\\\\/]").containsMatchIn(path)
        if (absolute || cwd.isEmpty()) return path
        val sep = if ('\\' in cwd) '\\' else '/'
        return cwd.trimEnd('/', '\\') + sep + path.removePrefix("./").replace('/', sep)
    }
}

val LocalMedia = staticCompositionLocalOf<MediaSource?> { null }

private val VIDEO = setOf("mp4", "mov", "webm", "m4v")
private val IMAGE = setOf("png", "jpg", "jpeg", "gif", "webp", "bmp")

fun isMedia(src: String): Boolean = src.substringAfterLast('.').lowercase().let { it in VIDEO || it in IMAGE }

// for the session: a reply scrolled away and back shouldn't ask the computer again
private val images = object : android.util.LruCache<String, ImageBitmap>(48 * 1024 * 1024) {
    override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
}
private val videos = HashMap<String, File>()

@Composable
fun Media(src: String, alt: String) {
    val m = LocalMedia.current
    val h = LocalHues.current
    if (m == null || src.startsWith("http://") || src.startsWith("https://") || !isMedia(src)) {
        Text(alt.ifBlank { src }, style = MaterialTheme.typography.bodyMedium, color = h.text3)
        return
    }
    val path = m.resolve(src)
    if (path.substringAfterLast('.').lowercase() in VIDEO) VideoTile(m, path, alt) else Picture(m, path, alt)
}

@Composable
private fun Picture(m: MediaSource, path: String, alt: String) {
    val h = LocalHues.current
    var bitmap by remember(path) { mutableStateOf(images.get(path)) }
    var error by remember(path) { mutableStateOf<String?>(null) }
    var full by remember { mutableStateOf(false) }
    val context = LocalContext.current
    LaunchedEffect(path) {
        if (bitmap != null) return@LaunchedEffect
        m.fetch(path, cached(context, path))
            .mapCatching { file ->
                withContext(Dispatchers.Default) {
                    decode(file)?.asImageBitmap() ?: error("That image didn't decode.")
                }
            }
            .onSuccess { images.put(path, it); bitmap = it }
            .onFailure { error = it.message }
    }
    val b = bitmap
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 120.dp, max = 360.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.05f))
            .then(if (b != null) Modifier.clickableQuiet { full = true } else Modifier),
        contentAlignment = Alignment.Center,
    ) {
        when {
            b != null -> Image(b, alt, Modifier.fillMaxWidth(), contentScale = ContentScale.FillWidth)
            error != null -> Text(error!!, Modifier.padding(16.dp), style = MaterialTheme.typography.bodySmall, color = h.text3)
            else -> CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = h.text3)
        }
    }
    if (full && b != null) {
        Dialog(onDismissRequest = { full = false }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            Box(Modifier.fillMaxSize().background(Color.Black).clickableQuiet { full = false }, contentAlignment = Alignment.Center) {
                Image(b, alt, Modifier.fillMaxWidth(), contentScale = ContentScale.Fit)
            }
        }
    }
}

private fun cached(context: android.content.Context, path: String) =
    File(File(context.cacheDir, "media"), "${path.hashCode().toUInt()}.${path.substringAfterLast('.')}")

/** No wider than a phone needs: a 4K screenshot whole would take tens of megabytes. */
private fun decode(file: File): android.graphics.Bitmap? {
    val size = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(file.path, size)
    var sample = 1
    while (size.outWidth / (sample * 2) >= 1440) sample *= 2
    return BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply { inSampleSize = sample })
}

@Composable
private fun VideoTile(m: MediaSource, path: String, alt: String) {
    val h = LocalHues.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var file by remember(path) { mutableStateOf(videos[path]) }
    var loading by remember { mutableStateOf(false) }
    var error by remember(path) { mutableStateOf<String?>(null) }
    var playing by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.05f))
            .clickableQuiet {
                if (file != null) {
                    playing = true
                } else if (!loading) {
                    loading = true
                    scope.launch {
                        m.fetch(path, cached(context, path))
                            .onSuccess { videos[path] = it; file = it; playing = true }
                            .onFailure { error = it.message }
                        loading = false
                    }
                }
            }
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(36.dp).clip(RoundedCornerShape(10.dp)).background(h.ink(0.08f)), contentAlignment = Alignment.Center) {
            if (loading) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = h.text3)
            } else {
                Icon(painterResource(R.drawable.ic_circle_play), "Play", Modifier.size(18.dp), tint = h.text1)
            }
        }
        Text(
            error ?: alt.ifBlank { path.substringAfterLast('/').substringAfterLast('\\') },
            Modifier.weight(1f),
            style = MaterialTheme.typography.bodyMedium,
            color = if (error != null) h.text3 else h.text1,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
    val f = file
    if (playing && f != null) {
        Dialog(onDismissRequest = { playing = false }, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            Box(Modifier.fillMaxSize().background(Color.Black), contentAlignment = Alignment.Center) {
                AndroidView(
                    factory = { ctx ->
                        VideoView(ctx).apply {
                            val controls = MediaController(ctx)
                            controls.setAnchorView(this)
                            setMediaController(controls)
                            setVideoPath(f.absolutePath)
                            setOnPreparedListener { start() }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}
