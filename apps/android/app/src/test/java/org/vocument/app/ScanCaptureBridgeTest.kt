package org.vocument.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Test
import uniffi.vocument_ffi.FfiScanCleanupProfile
import uniffi.vocument_ffi.FfiScanFrame
import uniffi.vocument_ffi.FfiScanPixelFormat

/**
 * Android test verifying the native scan-capture pipeline and live document edge detection (`ROADMAP.md` M1.0, `SCAN-001`, `SCAN-004`, `SCAN-006`).
 */
class ScanCaptureBridgeTest {
    companion object {
        @JvmStatic
        @BeforeClass
        fun setUpNativeLibrary() {
            val candidatePaths = listOf(
                "../../target/debug",
                "../../../target/debug",
                "target/debug"
            )
            val existing = candidatePaths.firstOrNull { java.io.File(it).exists() }
            if (existing != null) {
                val absolute = java.io.File(existing).absolutePath
                val current = System.getProperty("jna.library.path")
                System.setProperty(
                    "jna.library.path",
                    if (current.isNullOrBlank()) absolute else "$current:$absolute"
                )
            }
        }
    }

    private val bridge = DocumentCoreBridge()

    private fun createSyntheticDocumentFrame(width: UInt, height: UInt): FfiScanFrame {
        val totalBytes = (width * height * 4u).toInt()
        val pixels = ByteArray(totalBytes) { 25.toByte() } // Dark background

        // Document rectangle in the center (inset by ~20%)
        val startX = (width.toInt() * 0.2).toInt()
        val endX = (width.toInt() * 0.8).toInt()
        val startY = (height.toInt() * 0.2).toInt()
        val endY = (height.toInt() * 0.8).toInt()

        for (y in startY until endY) {
            for (x in startX until endX) {
                val idx = (y * width.toInt() + x) * 4
                pixels[idx] = 240.toByte()     // R
                pixels[idx + 1] = 240.toByte() // G
                pixels[idx + 2] = 240.toByte() // B
                pixels[idx + 3] = 255.toByte() // A
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
            deviceAngleDegrees = 1.0f,
            brightnessHint = 0.9f,
            isSteady = true
        )
    }

    @Test
    fun testLiveDocumentEdgeDetectionReturnsAccurateCorners() {
        val frame = createSyntheticDocumentFrame(width = 100u, height = 100u)
        val detected = bridge.detectDocumentEdges(frame)

        assertNotNull("Edge detector should find the synthetic document", detected)
        val corners = detected!!

        // High confidence on clean contrast:
        assertTrue("Confidence should exceed 600 milli", corners.confidenceMilli >= 600u)

        // Verify normalized coordinates roughly match our 20% to 80% inset
        assertTrue("Top-left X should be near 0.2", corners.topLeft.x in 0.15f..0.25f)
        assertTrue("Top-left Y should be near 0.2", corners.topLeft.y in 0.15f..0.25f)
        assertTrue("Bottom-right X should be near 0.8", corners.bottomRight.x in 0.75f..0.85f)
        assertTrue("Bottom-right Y should be near 0.8", corners.bottomRight.y in 0.75f..0.85f)
    }

    @Test
    fun testMultiPageScanSessionLifecycle() {
        val sessionId = bridge.startScanSession()
        assertTrue("Session ID must have prefix", sessionId.startsWith("scan_sess_"))

        val frame1 = createSyntheticDocumentFrame(width = 120u, height = 160u)
        val corners1 = bridge.detectDocumentEdges(frame1)

        val page0 = bridge.appendScanPage(
            sessionId = sessionId,
            frame = frame1,
            corners = corners1,
            profile = FfiScanCleanupProfile.DOCUMENT_AUTO
        )
        assertEquals("First page index must be 0", 0u, page0.pageIndex)
        assertEquals("Frame width must match", 120u, page0.frameWidth)
        assertEquals("Frame height must match", 160u, page0.frameHeight)

        val frame2 = createSyntheticDocumentFrame(width = 120u, height = 160u)
        val page1 = bridge.appendScanPage(
            sessionId = sessionId,
            frame = frame2,
            corners = null, // Auto-detection inside append_page
            profile = FfiScanCleanupProfile.HIGH_CONTRAST_BW
        )
        assertEquals("Second page index must be 1", 1u, page1.pageIndex)

        val sessionSnap = bridge.getScanSession(sessionId)
        assertEquals("Total page count must be 2", 2u, sessionSnap.pageCount)
        assertEquals(2, sessionSnap.pages.size)
        assertTrue("Session must be active", sessionSnap.isActive)

        bridge.closeScanSession(sessionId)
    }
}
