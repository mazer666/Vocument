package org.vocument.app

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import android.os.Handler
import android.os.Looper
import java.util.concurrent.Executors
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import uniffi.vocument_ffi.FfiCapturedScanPage
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiPoint2D
import uniffi.vocument_ffi.FfiQuadCorners
import uniffi.vocument_ffi.FfiScanCleanupProfile
import uniffi.vocument_ffi.FfiScanFrame
import uniffi.vocument_ffi.FfiScanPixelFormat
import uniffi.vocument_ffi.FfiScanSessionSnapshot

/**
 * Scan Capture & Camera Ingestion Screen (`ROADMAP.md` M1.0/M1.1/M1.4, `SCAN-001`..`SCAN-006`).
 * Interfaces live camera viewfinder frames with Rust `ImagingEngine` for real-time document
 * edge detection, interactive draggable corner adjustments, multi-page scan sessions, cleanup profiles,
 * and direct ingestion into `DocumentSession` as searchable PDF with progressive SQLite FTS5 search.
 */
@Composable
fun ScanCaptureScreen(
    coreBridge: DocumentCoreBridge,
    modifier: Modifier = Modifier,
    onDocumentSessionOpened: (FfiDocumentSessionSnapshot) -> Unit = {}
) {
    val context = LocalContext.current
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
        )
    }

    val permissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { granted ->
        hasCameraPermission = granted
    }

    var activeSessionId by remember { mutableStateOf<String?>(null) }
    var capturedPages by remember { mutableStateOf<List<FfiCapturedScanPage>>(emptyList()) }
    var latestLiveFrame by remember { mutableStateOf<FfiScanFrame?>(null) }
    var detectedCorners by remember { mutableStateOf<FfiQuadCorners?>(null) }
    var isManualAdjustmentActive by remember { mutableStateOf(false) }
    var selectedProfile by remember { mutableStateOf(FfiScanCleanupProfile.DOCUMENT_AUTO) }
    var statusMessage by remember { mutableStateOf(if (hasCameraPermission) "Camera active: point at document" else "Camera permission required") }
    var lastOcrSummary by remember { mutableStateOf<String?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var isProcessingFrame by remember { mutableStateOf(false) }

    // Heavy scan work (edge detection, perspective correction, cleanup, OCR) runs
    // on a background executor, never on the UI thread (`ARCHITECTURE.md` §8.4).
    // The executor is created once per screen and shut down when the screen
    // leaves the composition.
    val scanExecutor = remember { Executors.newSingleThreadExecutor() }
    val mainHandler = remember { Handler(Looper.getMainLooper()) }
    DisposableEffect(Unit) {
        onDispose { scanExecutor.shutdown() }
    }

    fun updateCorner(cornerIndex: Int, newX: Float, newY: Float) {
        val current = detectedCorners ?: FfiQuadCorners(
            topLeft = FfiPoint2D(0.15f, 0.15f),
            topRight = FfiPoint2D(0.85f, 0.15f),
            bottomRight = FfiPoint2D(0.85f, 0.85f),
            bottomLeft = FfiPoint2D(0.15f, 0.85f),
            confidenceMilli = 999u
        )
        val clampedPt = FfiPoint2D(newX.coerceIn(0.01f, 0.99f), newY.coerceIn(0.01f, 0.99f))
        val updated = when (cornerIndex) {
            0 -> current.copy(topLeft = clampedPt, confidenceMilli = 1000u)
            1 -> current.copy(topRight = clampedPt, confidenceMilli = 1000u)
            2 -> current.copy(bottomRight = clampedPt, confidenceMilli = 1000u)
            else -> current.copy(bottomLeft = clampedPt, confidenceMilli = 1000u)
        }
        detectedCorners = updated
        isManualAdjustmentActive = true
        statusMessage = "Manual corner alignment active (Handle #${cornerIndex + 1})"
    }

    fun ensureActiveSession(): String {
        val existing = activeSessionId
        if (existing != null) return existing
        val newId = coreBridge.startScanSession()
        activeSessionId = newId
        capturedPages = emptyList()
        return newId
    }

    /**
     * Handles a captured camera frame.
     *
     * The heavy work (edge detection, perspective correction, cleanup and OCR) runs
     * on a background executor so that the UI thread is never blocked. Only the
     * final status strings are published back to the Compose state.
     */
    fun handleCaptureCurrentFrame(frameToCapture: FfiScanFrame) {
        val sessionId = ensureActiveSession()
        val corners = detectedCorners ?: runCatching { coreBridge.detectDocumentEdges(frameToCapture) }.getOrNull()
        isProcessingFrame = true
        scanExecutor.execute {
            val result = runCatching {
                val page = coreBridge.appendScanPage(
                    sessionId = sessionId,
                    frame = frameToCapture,
                    corners = corners,
                    profile = selectedProfile
                )
                val session = coreBridge.getScanSession(sessionId)
                val warped = runCatching { coreBridge.getWarpedScanPage(sessionId, page.pageIndex) }.getOrNull()
                val enhanced = runCatching { coreBridge.getEnhancedScanPage(sessionId, page.pageIndex) }.getOrNull()
                val deskew = warped?.let { runCatching { coreBridge.deskewFrame(it) }.getOrNull() }
                val deskewInfo = if (deskew != null) ", skew=${deskew.skewAngleDegrees}°" else ""
                val profileInfo = ", ${selectedProfile.name.lowercase().replace('_', ' ')}"
                val ocrRes = coreBridge.ocrScanSessionPage(sessionId, page.pageIndex, listOf("deu", "eng"))
                val bboxesValid = ocrRes.words.isNotEmpty() && ocrRes.words.all { (it.bbox.x1 > it.bbox.x0) && (it.bbox.y1 > it.bbox.y0) }
                val bboxStatus = if (bboxesValid) "valid bboxes (${ocrRes.words.size})" else "invalid bboxes"
                val ocrSummary = "OCR: ${ocrRes.detectedLanguage} (${ocrRes.meanConfidence.toInt()}%, ${ocrRes.words.size}w, $bboxStatus)"
                val width = enhanced?.width ?: warped?.width ?: frameToCapture.width
                val height = enhanced?.height ?: warped?.height ?: frameToCapture.height
                CaptureOutcome(
                    session = session,
                    ocrSummary = ocrSummary,
                    message = "Page #${page.pageIndex + 1u} cleaned, $ocrSummary (${width}x${height}px$deskewInfo$profileInfo)"
                )
            }
            // Publish back to the UI thread. The Compose state must only be
            // mutated from the main thread.
            mainHandler.post {
                isProcessingFrame = false
                result
                    .onSuccess { outcome ->
                        capturedPages = outcome.session.pages
                        lastOcrSummary = outcome.ocrSummary
                        statusMessage = outcome.message
                        errorMessage = null
                    }
                    .onFailure { err ->
                        errorMessage = "Scan error: ${err.message}"
                    }
            }
        }
    }

    fun loadCorpusOrSyntheticFrame(): FfiScanFrame {
        val bitmap = runCatching {
            context.assets.open("corpus/deu_vertrag_vertraulichkeit_camera_distorted.png").use { stream ->
                android.graphics.BitmapFactory.decodeStream(stream)
            }
        }.getOrNull()

        if (bitmap != null) {
            val width = bitmap.width.toUInt()
            val height = bitmap.height.toUInt()
            val argbPixels = IntArray(bitmap.width * bitmap.height)
            bitmap.getPixels(argbPixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
            val rgbaBytes = ByteArray(bitmap.width * bitmap.height * 4)
            var byteIdx = 0
            for (pixel in argbPixels) {
                rgbaBytes[byteIdx] = (pixel shr 16 and 0xFF).toByte()
                rgbaBytes[byteIdx + 1] = (pixel shr 8 and 0xFF).toByte()
                rgbaBytes[byteIdx + 2] = (pixel and 0xFF).toByte()
                rgbaBytes[byteIdx + 3] = (pixel shr 24 and 0xFF).toByte()
                byteIdx += 4
            }
            return FfiScanFrame(
                width = width,
                height = height,
                stride = width,
                format = FfiScanPixelFormat.RGBA8,
                orientationDegrees = 0u,
                pixels = rgbaBytes,
                timestampMs = System.currentTimeMillis().toULong(),
                deviceAngleDegrees = 0f,
                brightnessHint = 1.0f,
                isSteady = true
            )
        }

        val width = 120u
        val height = 160u
        return FfiScanFrame(
            width = width,
            height = height,
            stride = width,
            format = FfiScanPixelFormat.RGBA8,
            orientationDegrees = 0u,
            pixels = ByteArray((width * height * 4u).toInt()) { 240.toByte() },
            timestampMs = System.currentTimeMillis().toULong(),
            deviceAngleDegrees = 0f,
            brightnessHint = 0.9f,
            isSteady = true
        )
    }

    fun handleCloseSession() {
        activeSessionId?.let { id ->
            runCatching { coreBridge.closeScanSession(id) }
            activeSessionId = null
            capturedPages = emptyList()
            detectedCorners = null
            isManualAdjustmentActive = false
            statusMessage = "Scan session completed and closed."
        }
    }

    fun handleIngestAsDocumentSession() {
        val sessionId = activeSessionId ?: return
        runCatching {
            val sessionSnap = coreBridge.ingestScanAsDocumentSession(
                scanSessionId = sessionId,
                cleanupProfile = selectedProfile,
                languages = listOf("deu", "eng"),
                displayName = "Scan_${System.currentTimeMillis()}.pdf"
            )
            activeSessionId = null
            capturedPages = emptyList()
            detectedCorners = null
            isManualAdjustmentActive = false
            statusMessage = "PDF synthesized (${sessionSnap.pdfSummary.pageCount} p.) & opened in session"
            onDocumentSessionOpened(sessionSnap)
        }.onFailure { err ->
            errorMessage = "PDF Ingestion error: ${err.message}"
        }
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Session Header & Badge
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f, fill = false)) {
                Text(
                    text = "Scan & OCR Capture",
                    style = MaterialTheme.typography.titleMedium
                )
                Text(
                    text = if (lastOcrSummary != null) "Session: ${activeSessionId?.take(12)}… | $lastOcrSummary" else if (activeSessionId != null) "Session: ${activeSessionId?.take(12)}… | $statusMessage" else statusMessage,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer
                ),
                shape = RoundedCornerShape(12.dp)
            ) {
                Text(
                    text = "${capturedPages.size} pages",
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onPrimaryContainer
                )
            }
        }

        // Viewfinder Preview with Live Camera or Permission Prompt
        Card(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f),
            shape = RoundedCornerShape(16.dp),
            colors = CardDefaults.cardColors(containerColor = Color.Black)
        ) {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center
            ) {
                if (hasCameraPermission) {
                    CameraPreviewView(
                        modifier = Modifier.fillMaxSize(),
                        onFrameAnalyzed = { liveFrame ->
                            latestLiveFrame = liveFrame
                            if (!isManualAdjustmentActive) {
                                val corners = runCatching { coreBridge.detectDocumentEdges(liveFrame) }.getOrNull()
                                detectedCorners = corners
                                if (corners != null && corners.confidenceMilli >= 650u) {
                                    statusMessage = "Document detected (${corners.confidenceMilli}‰) — steady"
                                }
                            }
                        }
                    )
                } else {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            text = "Camera access required for live scanning",
                            color = Color.White,
                            style = MaterialTheme.typography.bodyMedium
                        )
                        Button(onClick = { permissionLauncher.launch(Manifest.permission.CAMERA) }) {
                            Text("Grant Camera Permission")
                        }
                    }
                }

                // Overlay with dynamic quadrilateral corners and draggable adjustment handles
                ViewfinderOverlay(
                    corners = detectedCorners,
                    isManualAdjustment = isManualAdjustmentActive,
                    onCornerDragged = ::updateCorner,
                    modifier = Modifier.fillMaxSize()
                )

                // Status banner on top of viewfinder
                Box(
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .padding(top = 12.dp)
                        .background(
                            Color.Black.copy(alpha = 0.65f),
                            shape = RoundedCornerShape(8.dp)
                        )
                        .padding(horizontal = 12.dp, vertical = 6.dp)
                ) {
                    Text(
                        text = statusMessage,
                        color = Color.White,
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }
        }

        // Cleanup Profile Selection & Corner Reset
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "Enhancement Profile",
                style = MaterialTheme.typography.labelMedium
            )
            if (isManualAdjustmentActive) {
                OutlinedButton(
                    onClick = {
                        isManualAdjustmentActive = false
                        detectedCorners = null
                        statusMessage = "Auto corner detection restored"
                    }
                ) {
                    Text("Reset Auto Corners")
                }
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            FilterChip(
                selected = selectedProfile == FfiScanCleanupProfile.DOCUMENT_AUTO,
                onClick = { selectedProfile = FfiScanCleanupProfile.DOCUMENT_AUTO },
                label = { Text("Auto") }
            )
            FilterChip(
                selected = selectedProfile == FfiScanCleanupProfile.PRESERVE_PHOTO,
                onClick = { selectedProfile = FfiScanCleanupProfile.PRESERVE_PHOTO },
                label = { Text("Photo") }
            )
            FilterChip(
                selected = selectedProfile == FfiScanCleanupProfile.HIGH_CONTRAST_BW,
                onClick = { selectedProfile = FfiScanCleanupProfile.HIGH_CONTRAST_BW },
                label = { Text("B&W") }
            )
        }

        // Captured Pages Reel
        if (capturedPages.isNotEmpty()) {
            LazyRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(capturedPages) { page ->
                    CapturedPageThumbnail(page = page)
                }
            }
        }

        // Error message if any
        errorMessage?.let { err ->
            Text(
                text = err,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall
            )
        }

        // Controls
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Button(
                onClick = {
                    val frame = latestLiveFrame ?: loadCorpusOrSyntheticFrame()
                    handleCaptureCurrentFrame(frame)
                },
                modifier = Modifier.weight(1f)
            ) {
                Text(if (latestLiveFrame != null) "Capture Page" else "Capture / Simulate Page")
            }
            if (activeSessionId != null && capturedPages.isNotEmpty()) {
                Button(
                    onClick = { handleIngestAsDocumentSession() }
                ) {
                    Text("Open as PDF")
                }
            }
            if (activeSessionId != null) {
                OutlinedButton(
                    onClick = { handleCloseSession() }
                ) {
                    Text("Finish Session")
                }
            }
        }
    }
}

@Composable
fun CapturedPageThumbnail(page: FfiCapturedScanPage) {
    Card(
        modifier = Modifier
            .width(64.dp)
            .aspectRatio(0.707f),
        shape = RoundedCornerShape(6.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "P.${page.pageIndex + 1u}",
                style = MaterialTheme.typography.labelSmall
            )
        }
    }
}

/**
 * Result of a background capture run.
 *
 * Produced on the background executor and published to the Compose state on the
 * UI thread, so that no heavy object is touched from the wrong thread.
 */
private data class CaptureOutcome(
    val session: FfiScanSessionSnapshot,
    val ocrSummary: String,
    val message: String
)
