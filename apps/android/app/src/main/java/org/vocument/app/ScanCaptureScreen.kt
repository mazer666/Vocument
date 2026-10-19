package org.vocument.app

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import uniffi.vocument_ffi.FfiCapturedScanPage
import uniffi.vocument_ffi.FfiDocumentSessionSnapshot
import uniffi.vocument_ffi.FfiPoint2D
import uniffi.vocument_ffi.FfiQuadCorners
import uniffi.vocument_ffi.FfiScanCleanupProfile
import uniffi.vocument_ffi.FfiScanFrame
import uniffi.vocument_ffi.FfiScanPixelFormat

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

    fun handleCaptureCurrentFrame(frameToCapture: FfiScanFrame) {
        runCatching {
            val sessionId = ensureActiveSession()
            val corners = detectedCorners ?: coreBridge.detectDocumentEdges(frameToCapture)
            val page = coreBridge.appendScanPage(
                sessionId = sessionId,
                frame = frameToCapture,
                corners = corners,
                profile = selectedProfile
            )
            val session = coreBridge.getScanSession(sessionId)
            capturedPages = session.pages
            val warped = runCatching { coreBridge.getWarpedScanPage(sessionId, page.pageIndex) }.getOrNull()
            val enhanced = runCatching { coreBridge.getEnhancedScanPage(sessionId, page.pageIndex) }.getOrNull()
            val deskew = warped?.let { runCatching { coreBridge.deskewFrame(it) }.getOrNull() }
            val deskewInfo = if (deskew != null) ", skew=${deskew.skewAngleDegrees}°" else ""
            val profileInfo = ", ${selectedProfile.name.lowercase().replace('_', ' ')}"
            val ocrRes = coreBridge.ocrScanSessionPage(sessionId, page.pageIndex, listOf("deu", "eng"))
            val bboxesValid = ocrRes.words.isNotEmpty() && ocrRes.words.all { (it.bbox.x1 > it.bbox.x0) && (it.bbox.y1 > it.bbox.y0) }
            val bboxStatus = if (bboxesValid) "valid bboxes (${ocrRes.words.size})" else "invalid bboxes"
            val ocrSummary = "OCR: ${ocrRes.detectedLanguage} (${ocrRes.meanConfidence.toInt()}%, ${ocrRes.words.size}w, $bboxStatus)"
            lastOcrSummary = ocrSummary
            statusMessage = "Page #${page.pageIndex + 1u} cleaned, $ocrSummary (${enhanced?.width ?: warped?.width ?: frameToCapture.width}x${enhanced?.height ?: warped?.height ?: frameToCapture.height}px$deskewInfo$profileInfo)"
            errorMessage = null
        }.onFailure { err ->
            errorMessage = "Scan error: ${err.message}"
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
fun ViewfinderOverlay(
    corners: FfiQuadCorners?,
    isManualAdjustment: Boolean,
    onCornerDragged: (cornerIndex: Int, newX: Float, newY: Float) -> Unit,
    modifier: Modifier = Modifier
) {
    var activeDraggingCorner by remember { mutableStateOf<Int?>(null) }
    var canvasSize by remember { mutableStateOf(IntSize.Zero) }

    Canvas(
        modifier = modifier
            .onSizeChanged { canvasSize = it }
            .pointerInput(corners) {
                detectDragGestures(
                    onDragStart = { offset ->
                        if (canvasSize.width > 0 && canvasSize.height > 0) {
                            val activeCorners = corners ?: FfiQuadCorners(
                                topLeft = FfiPoint2D(0.15f, 0.15f),
                                topRight = FfiPoint2D(0.85f, 0.15f),
                                bottomRight = FfiPoint2D(0.85f, 0.85f),
                                bottomLeft = FfiPoint2D(0.15f, 0.85f),
                                confidenceMilli = 999u
                            )
                            val p0 = Offset(activeCorners.topLeft.x * canvasSize.width, activeCorners.topLeft.y * canvasSize.height)
                            val p1 = Offset(activeCorners.topRight.x * canvasSize.width, activeCorners.topRight.y * canvasSize.height)
                            val p2 = Offset(activeCorners.bottomRight.x * canvasSize.width, activeCorners.bottomRight.y * canvasSize.height)
                            val p3 = Offset(activeCorners.bottomLeft.x * canvasSize.width, activeCorners.bottomLeft.y * canvasSize.height)
                            val points = listOf(p0, p1, p2, p3)
                            val hitRadius = 48.dp.toPx()
                            val hitIndex = points.indexOfFirst { (it - offset).getDistance() <= hitRadius }
                            if (hitIndex != -1) {
                                activeDraggingCorner = hitIndex
                            }
                        }
                    },
                    onDragEnd = {
                        activeDraggingCorner = null
                    },
                    onDragCancel = {
                        activeDraggingCorner = null
                    },
                    onDrag = { change, dragAmount ->
                        val cornerIdx = activeDraggingCorner
                        if (cornerIdx != null && canvasSize.width > 0 && canvasSize.height > 0) {
                            change.consume()
                            val activeCorners = corners ?: FfiQuadCorners(
                                topLeft = FfiPoint2D(0.15f, 0.15f),
                                topRight = FfiPoint2D(0.85f, 0.15f),
                                bottomRight = FfiPoint2D(0.85f, 0.85f),
                                bottomLeft = FfiPoint2D(0.15f, 0.85f),
                                confidenceMilli = 999u
                            )
                            val currentNorm = when (cornerIdx) {
                                0 -> activeCorners.topLeft
                                1 -> activeCorners.topRight
                                2 -> activeCorners.bottomRight
                                else -> activeCorners.bottomLeft
                            }
                            val newX = (currentNorm.x + dragAmount.x / canvasSize.width).coerceIn(0.01f, 0.99f)
                            val newY = (currentNorm.y + dragAmount.y / canvasSize.height).coerceIn(0.01f, 0.99f)
                            onCornerDragged(cornerIdx, newX, newY)
                        }
                    }
                )
            }
    ) {
        val canvasWidth = size.width
        val canvasHeight = size.height

        val guideColor = Color.White.copy(alpha = 0.3f)
        drawLine(
            color = guideColor,
            start = Offset(canvasWidth * 0.15f, canvasHeight * 0.5f),
            end = Offset(canvasWidth * 0.85f, canvasHeight * 0.5f),
            strokeWidth = 1f
        )
        drawLine(
            color = guideColor,
            start = Offset(canvasWidth * 0.5f, canvasHeight * 0.15f),
            end = Offset(canvasWidth * 0.5f, canvasHeight * 0.85f),
            strokeWidth = 1f
        )

        if (corners != null) {
            val p1 = Offset(corners.topLeft.x * canvasWidth, corners.topLeft.y * canvasHeight)
            val p2 = Offset(corners.topRight.x * canvasWidth, corners.topRight.y * canvasHeight)
            val p3 = Offset(corners.bottomRight.x * canvasWidth, corners.bottomRight.y * canvasHeight)
            val p4 = Offset(corners.bottomLeft.x * canvasWidth, corners.bottomLeft.y * canvasHeight)

            val polyPath = Path().apply {
                moveTo(p1.x, p1.y)
                lineTo(p2.x, p2.y)
                lineTo(p3.x, p3.y)
                lineTo(p4.x, p4.y)
                close()
            }

            val quadColor = if (isManualAdjustment) {
                Color(0xFF29B6F6) // Cyan/Blue in manual adjustment mode
            } else if (corners.confidenceMilli >= 650u) {
                Color(0xFF00E676) // High confidence: Vibrant Green
            } else {
                Color(0xFFFFD600) // Moderate confidence: Amber
            }

            drawPath(
                path = polyPath,
                color = quadColor.copy(alpha = 0.2f)
            )
            drawPath(
                path = polyPath,
                color = quadColor,
                style = Stroke(width = 3.dp.toPx())
            )

            val points = listOf(p1, p2, p3, p4)
            for ((idx, cp) in points.withIndex()) {
                val isDragging = activeDraggingCorner == idx
                if (isDragging) {
                    drawCircle(color = quadColor.copy(alpha = 0.35f), radius = 24.dp.toPx(), center = cp)
                    drawCircle(color = Color.White, radius = 12.dp.toPx(), center = cp)
                    drawCircle(color = quadColor, radius = 8.dp.toPx(), center = cp)
                } else {
                    drawCircle(color = Color.Black.copy(alpha = 0.4f), radius = 16.dp.toPx(), center = cp)
                    drawCircle(color = Color.White, radius = 8.dp.toPx(), center = cp)
                    drawCircle(color = quadColor, radius = 5.dp.toPx(), center = cp)
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
