package com.hyprspace.android.ui.thread

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickMultipleVisualMedia
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.core.graphics.scale
import com.hyprspace.android.App
import com.hyprspace.android.R
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet
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

@Composable
fun PhotoThumb(p: Photo, onRemove: () -> Unit) {
    val h = LocalHues.current
    Box(Modifier.size(60.dp)) {
        Image(
            p.thumb,
            null,
            Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).alpha(if (p.path == null) 0.5f else 1f),
            contentScale = ContentScale.Crop,
        )
        if (p.path == null && !p.failed) {
            CircularProgressIndicator(Modifier.align(Alignment.Center).size(18.dp), strokeWidth = 2.dp, color = h.text1)
        }
        if (p.failed) {
            Box(Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).background(h.error.copy(alpha = 0.35f)))
        }
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .padding(3.dp)
                .size(20.dp)
                .clip(CircleShape)
                .background(h.bg.copy(alpha = 0.8f))
                .clickableQuiet(onRemove),
            contentAlignment = Alignment.Center,
        ) {
            Icon(painterResource(R.drawable.ic_x), "Remove", Modifier.size(11.dp), tint = h.text1)
        }
    }
}
