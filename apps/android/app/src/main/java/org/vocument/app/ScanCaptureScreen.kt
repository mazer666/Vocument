package org.vocument.app

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import uniffi.vocument_ffi.FfiCapturedScanPage
import uniffi.vocument_ffi.FfiQuadCorners
import uniffi.vocument_ffi.FfiScanCleanupProfile
import uniffi.vocument_ffi.FfiScanFrame
import uniffi.vocument_ffi.FfiScanPixelFormat

/**
 * Scan Capture & Camera Ingestion Screen (`ROADMAP.md` M1.0, `SCAN-001`, `SCAN-004`, `SCAN-006`).
 * Interfaces live camera viewfinder frames with Rust `ImagingEngine` for real-time document
 * edge detection, multi-page scanning session state, and cleanup profile configuration.
 */
@Composable
fun ScanCaptureScreen(
    coreBridge: DocumentCoreBridge,
    modifier: Modifier = Modifier
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
    var selectedProfile by remember { mutableStateOf(FfiScanCleanupProfile.DOCUMENT_AUTO) }
    var statusMessage by remember { mutableStateOf(if (hasCameraPermission) "Camera active: point at document" else "Camera permission required") }
    var errorMessage by remember { mutableStateOf<String?>(null) }

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
            val corners = coreBridge.detectDocumentEdges(frameToCapture)
            val page = coreBridge.appendScanPage(
                sessionId = sessionId,
                frame = frameToCapture,
                corners = corners,
                profile = selectedProfile
            )
            val session = coreBridge.getScanSession(sessionId)
            capturedPages = session.pages
            val warped = runCatching { coreBridge.getWarpedScanPage(sessionId, page.pageIndex) }.getOrNull()
            val deskew = warped?.let { runCatching { coreBridge.deskewFrame(it) }.getOrNull() }
            val deskewInfo = if (deskew != null) ", skew=${deskew.skewAngleDegrees}°" else ""
            statusMessage = "Page #${page.pageIndex + 1u} rectified (${warped?.width ?: frameToCapture.width}x${warped?.height ?: frameToCapture.height}px$deskewInfo)"
            errorMessage = null
        }.onFailure { err ->
            errorMessage = "Scan error: ${err.message}"
        }
    }

    fun createSyntheticDocumentFrame(): FfiScanFrame {
        val width = 120u
        val height = 160u
        val totalBytes = (width * height * 4u).toInt()
        val pixels = ByteArray(totalBytes) { 35.toByte() }
        for (y in 25 until 135) {
            for (x in 20 until 100) {
                val idx = (y * 120 + x) * 4
                pixels[idx] = 245.toByte()
                pixels[idx + 1] = 245.toByte()
                pixels[idx + 2] = 245.toByte()
                pixels[idx + 3] = 255.toByte()
            }
        }
        return FfiScanFrame(
            width = width,
            height = height,
            stride = width,
            format = FfiScanPixelFormat.RGBA8,
            orientationDegrees = 0u,
            pixels = pixels,
            timestampMs = System.currentTimeMillis().toULong(),
            deviceAngleDegrees = 2.0f,
            brightnessHint = 0.85f,
            isSteady = true
        )
    }

    fun handleCloseSession() {
        activeSessionId?.let { id ->
            runCatching { coreBridge.closeScanSession(id) }
            activeSessionId = null
            capturedPages = emptyList()
            detectedCorners = null
            statusMessage = "Scan session completed and closed."
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
            Column {
                Text(
                    text = "Scan & OCR Capture",
                    style = MaterialTheme.typography.titleMedium
                )
                Text(
                    text = if (activeSessionId != null) "Session: ${activeSessionId?.take(12)}…" else "Ready",
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
                            val corners = runCatching { coreBridge.detectDocumentEdges(liveFrame) }.getOrNull()
                            detectedCorners = corners
                            if (corners != null && corners.confidenceMilli >= 650u) {
                                statusMessage = "Document detected (${corners.confidenceMilli}‰) — steady"
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

                // Overlay with dynamic quadrilateral corners
                ViewfinderOverlay(
                    corners = detectedCorners,
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

        // Cleanup Profile Selection
        Text(
            text = "Enhancement Profile",
            style = MaterialTheme.typography.labelMedium
        )
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
                    val frame = latestLiveFrame ?: createSyntheticDocumentFrame()
                    handleCaptureCurrentFrame(frame)
                },
                modifier = Modifier.weight(1f)
            ) {
                Text(if (latestLiveFrame != null) "Capture Page" else "Capture / Simulate Page")
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
    modifier: Modifier = Modifier
) {
    Canvas(modifier = modifier) {
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

            val quadColor = if (corners.confidenceMilli >= 650u) {
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

            for (cp in listOf(p1, p2, p3, p4)) {
                drawCircle(color = Color.White, radius = 6.dp.toPx(), center = cp)
                drawCircle(color = quadColor, radius = 4.dp.toPx(), center = cp)
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
