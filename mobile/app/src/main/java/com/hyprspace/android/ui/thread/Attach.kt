package com.hyprspace.android.ui.thread

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments
import androidx.activity.result.contract.ActivityResultContracts.PickMultipleVisualMedia
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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

/** A picked photo or file: its thumbnail or name, and its path on the computer once uploaded. */
data class Attachment(
    val key: Long,
    val thumb: ImageBitmap? = null,
    val name: String = "",
    val path: String? = null,
    val failed: Boolean = false,
)

private const val SIDE = 2048
private const val THUMB = 160
private const val MAX_FILE = 20 * 1024 * 1024

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

private fun nameOf(context: Context, uri: Uri): String =
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
        if (it.moveToFirst()) it.getString(0) else null
    } ?: uri.lastPathSegment?.substringAfterLast('/') ?: "file"

/**
 * Picks photos, or files too when [files] is on, through a small sheet. Each one shows on
 * [onPick] at once, goes up to the computer, and shows again with its path (or as failed).
 */
@Composable
fun rememberAttach(app: App, files: Boolean, onPick: (Attachment) -> Unit): () -> Unit {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var choosing by remember { mutableStateOf(false) }
    fun send(a: Attachment, bytes: ByteArray, name: String?) {
        onPick(a)
        scope.launch {
            val path = app.link.upload(bytes, name)
            onPick(a.copy(path = path, failed = path == null))
        }
    }
    val photos = rememberLauncherForActivityResult(PickMultipleVisualMedia(4)) { uris ->
        for (uri in uris) {
            scope.launch {
                val (bytes, thumb) = runCatching { shrink(ctx, uri) }.getOrNull() ?: return@launch
                send(Attachment(System.nanoTime(), thumb.asImageBitmap()), bytes, null)
            }
        }
    }
    val documents = rememberLauncherForActivityResult(OpenMultipleDocuments()) { uris ->
        for (uri in uris.take(4)) {
            scope.launch {
                val name = runCatching { nameOf(ctx, uri) }.getOrDefault("file")
                val bytes = withContext(Dispatchers.IO) {
                    runCatching { ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() } }.getOrNull()
                }
                when {
                    bytes == null -> onPick(Attachment(System.nanoTime(), name = name, failed = true))
                    bytes.size > MAX_FILE -> {
                        app.link.fail("$name is over 20 MB, too big to send.")
                        onPick(Attachment(System.nanoTime(), name = name, failed = true))
                    }
                    else -> send(Attachment(System.nanoTime(), name = name), bytes, name)
                }
            }
        }
    }
    val pickPhotos = { photos.launch(PickVisualMediaRequest(PickVisualMedia.ImageOnly)) }
    if (choosing) {
        AttachSheet(
            onPhotos = { choosing = false; pickPhotos() },
            onFiles = { choosing = false; documents.launch(arrayOf("*/*")) },
            onDismiss = { choosing = false },
        )
    }
    return if (files) ({ choosing = true }) else pickPhotos
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AttachSheet(onPhotos: () -> Unit, onFiles: () -> Unit, onDismiss: () -> Unit) {
    val h = LocalHues.current
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = h.surface2,
    ) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Attach", style = MaterialTheme.typography.titleLarge, color = h.text1)
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(h.ink(0.04f))) {
                AttachRow("Photos", R.drawable.ic_image_plus, onPhotos)
                HorizontalDivider(Modifier.padding(start = 64.dp), color = h.border0)
                AttachRow("Files", R.drawable.ic_file_text, onFiles)
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun AttachRow(label: String, icon: Int, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier.fillMaxWidth().clickableQuiet(onClick).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(38.dp), contentAlignment = Alignment.Center) {
            Icon(painterResource(icon), null, Modifier.size(18.dp), tint = h.text2)
        }
        Spacer(Modifier.width(12.dp))
        Text(label, style = MaterialTheme.typography.bodyLarge, color = h.text1)
    }
}

@Composable
fun AttachmentThumb(p: Attachment, onRemove: () -> Unit) {
    val h = LocalHues.current
    Box(Modifier.size(60.dp)) {
        val thumb = p.thumb
        if (thumb != null) {
            Image(
                thumb,
                null,
                Modifier.fillMaxSize().clip(RoundedCornerShape(10.dp)).alpha(if (p.path == null) 0.5f else 1f),
                contentScale = ContentScale.Crop,
            )
        } else {
            Column(
                Modifier
                    .fillMaxSize()
                    .clip(RoundedCornerShape(10.dp))
                    .background(h.ink(0.07f))
                    .alpha(if (p.path == null) 0.5f else 1f)
                    .padding(horizontal = 5.dp, vertical = 8.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Icon(painterResource(R.drawable.ic_file_text), null, Modifier.size(18.dp), tint = h.text2)
                Text(p.name, fontSize = 9.sp, lineHeight = 11.sp, color = h.text2, maxLines = 2, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
            }
        }
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
