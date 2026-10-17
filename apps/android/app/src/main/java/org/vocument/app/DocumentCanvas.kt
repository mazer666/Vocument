package org.vocument.app

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiTileRenderRequest
import uniffi.vocument_ffi.FfiViewportRenderResult

/**
 * Native Android `DocumentCanvas` Composable (`ARCHITECTURE.md` §10, `ROADMAP.md` M0.2).
 * Renders RGBA8 tiles produced by `DocumentCore` (`DocumentActor` + `TileCache`), supporting
 * scroll, pinch/button zoom (`50%`–`300%`), neighbouring-page prefetch, and memory-pressure actions.
 */
@Composable
fun DocumentCanvas(
    session: FfiDocumentSessionSnapshot,
    currentPageIndex: UInt,
    scaleMilli: UInt,
    viewportResult: FfiViewportRenderResult,
    coreBridge: DocumentCoreBridge,
    onPageChange: (UInt) -> Unit,
    onScaleChange: (UInt) -> Unit,
    onMemoryPressure: (FfiMemoryPressureLevel) -> Unit
) {
    val bitmap = remember(viewportResult.visibleTile) {
        val tile = viewportResult.visibleTile
        val pixels = coreBridge.decodeRgba8ToArgbPixels(tile)
        Bitmap.createBitmap(
            pixels,
            tile.widthPx.toInt(),
            tile.heightPx.toInt(),
            Bitmap.Config.ARGB_8888
        ).asImageBitmap()
    }

    val stats = viewportResult.cacheStats
    val prefetchedLabel = if (viewportResult.prefetchedPages.isEmpty()) {
        "none"
    } else {
        viewportResult.prefetchedPages.joinToString(", ") { (it + 1u).toString() }
    }

    Column(modifier = Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            OutlinedButton(
                onClick = { if (currentPageIndex > 0u) onPageChange(currentPageIndex - 1u) },
                enabled = currentPageIndex > 0u
            ) {
                Text("◀ Prev")
            }

            Text(
                text = "Page ${currentPageIndex + 1u} / ${session.pdfSummary.pageCount}",
                style = MaterialTheme.typography.titleSmall
            )

            OutlinedButton(
                onClick = {
                    if (currentPageIndex + 1u < session.pdfSummary.pageCount) {
                        onPageChange(currentPageIndex + 1u)
                    }
                },
                enabled = currentPageIndex + 1u < session.pdfSummary.pageCount
            ) {
                Text("Next ▶")
            }
        }

        Spacer(modifier = Modifier.height(6.dp))

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OutlinedButton(
                onClick = { onScaleChange((scaleMilli - 250u).coerceAtLeast(500u)) },
                enabled = scaleMilli > 500u
            ) {
                Text("Zoom −")
            }
            Text("${scaleMilli / 10u}%")
            OutlinedButton(
                onClick = { onScaleChange((scaleMilli + 250u).coerceAtMost(3000u)) },
                enabled = scaleMilli < 3000u
            ) {
                Text("Zoom +")
            }
            Spacer(modifier = Modifier.weight(1f))
            Button(onClick = { onMemoryPressure(FfiMemoryPressureLevel.WARNING) }) {
                Text("Trim 50%")
            }
            Button(onClick = { onMemoryPressure(FfiMemoryPressureLevel.CRITICAL) }) {
                Text("Purge")
            }
        }

        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = "TileCache: ${stats.cachedTileCount} tiles (${stats.currentUsedBytes / 1024uL} KB) • " +
                "Hits: ${stats.hitCount} • Misses: ${stats.missCount} • Prefetched: $prefetchedLabel",
            style = MaterialTheme.typography.bodySmall
        )
        Spacer(modifier = Modifier.height(8.dp))

        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color(0xFFEEEEEE))
                .pointerInput(scaleMilli) {
                    detectTransformGestures { _, _, zoom, _ ->
                        val next = ((scaleMilli.toFloat() * zoom).toUInt() / 250u) * 250u
                        val clamped = next.coerceIn(500u, 3000u)
                        if (clamped != scaleMilli) {
                            onScaleChange(clamped)
                        }
                    }
                }
                .horizontalScroll(rememberScrollState())
                .verticalScroll(rememberScrollState()),
            contentAlignment = Alignment.Center
        ) {
            Image(
                bitmap = bitmap,
                contentDescription = "Rendered PDF page ${currentPageIndex + 1u}",
                modifier = Modifier
                    .padding(12.dp)
                    .background(Color.White)
            )
        }
    }
}

/**
 * Helper computing the scaled viewport `FfiTileRenderRequest` for [pageIndex] at [scaleMilli].
 */
fun buildViewportRenderRequest(
    coreBridge: DocumentCoreBridge,
    sessionId: String,
    pageIndex: UInt,
    scaleMilli: UInt
): FfiTileRenderRequest {
    val geometry = coreBridge.pageGeometry(sessionId, pageIndex)
    val widthPx = ((geometry.widthMilliPt.toULong() * scaleMilli.toULong()) / 1_000_000uL)
        .toUInt()
        .coerceAtLeast(1u)
    val heightPx = ((geometry.heightMilliPt.toULong() * scaleMilli.toULong()) / 1_000_000uL)
        .toUInt()
        .coerceAtLeast(1u)
    return FfiTileRenderRequest(
        pageIndex = pageIndex,
        scaleMilli = scaleMilli,
        tileXPx = 0u,
        tileYPx = 0u,
        tileWidthPx = widthPx,
        tileHeightPx = heightPx
    )
}
