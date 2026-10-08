package com.hyprspace.android.ui.thread

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickMultipleVisualMedia
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.core.graphics.scale
import com.hyprspace.android.App
import java.io.ByteArrayOutputStream
import kotlin.math.max
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** A picked photo: its thumbnail, and its path on the computer once uploaded. */
data class Photo(val key: Long, val thumb: ImageBitmap, val path: String? = null, val failed: Boolean = false)

private const val SIDE = 2048
private const val THUMB = 160

private suspend fun shrink(context: Context, uri: Uri): Pair<ByteArray, Bitmap> = withContext(Dispatchers.IO) {
    val bmp = ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, uri)) { d, info, _ ->
        val (w, h) = info.size.width to info.size.height
        val scale = SIDE.toFloat() / max(w, h)
        if (scale < 1f) d.setTargetSize((w * scale).roundToInt().coerceAtLeast(1), (h * scale).roundToInt().coerceAtLeast(1))
        d.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
    }
    val out = ByteArrayOutputStream()
    bmp.compress(Bitmap.CompressFormat.JPEG, 85, out)
    val t = THUMB.toFloat() / max(bmp.width, bmp.height)
    val thumb = bmp.scale((bmp.width * t).roundToInt().coerceAtLeast(1), (bmp.height * t).roundToInt().coerceAtLeast(1))
    out.toByteArray() to thumb
}

/**
 * Opens Android's photo picker. Each photo is shrunk, shown to [onPhoto] at once, uploaded to
 * the computer, and shown again with its path (or as failed).
 */
@Composable
fun rememberPhotoPicker(app: App, many: Boolean, onPhoto: (Photo) -> Unit): () -> Unit {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val take: (List<Uri>) -> Unit = { uris ->
        for (uri in uris) {
            scope.launch {
                val (bytes, thumb) = runCatching { shrink(ctx, uri) }.getOrNull() ?: return@launch
                val p = Photo(System.nanoTime(), thumb.asImageBitmap())
                onPhoto(p)
                val path = app.link.upload(bytes)
                onPhoto(p.copy(path = path, failed = path == null))
            }
        }
    }
    val several = rememberLauncherForActivityResult(PickMultipleVisualMedia(4)) { take(it) }
    val one = rememberLauncherForActivityResult(PickVisualMedia()) { uri -> take(listOfNotNull(uri)) }
    val request = PickVisualMediaRequest(PickVisualMedia.ImageOnly)
    return { if (many) several.launch(request) else one.launch(request) }
}
