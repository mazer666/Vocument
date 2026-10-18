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

        // M1.1: Verify warped page retrieval from session
        val warpedPage0 = bridge.getWarpedScanPage(sessionId, 0u)
        assertTrue("Warped page must have non-zero width", warpedPage0.width > 0u)
        assertTrue("Warped page must have non-zero height", warpedPage0.height > 0u)

        bridge.closeScanSession(sessionId)
    }

    @Test
    fun testPerspectiveCorrectionWarpsQuadIntoRectangularFrame() {
        val frame = createSyntheticDocumentFrame(width = 100u, height = 100u)
        val corners = bridge.detectDocumentEdges(frame)
        assertNotNull("Must detect corners", corners)

        val warped = bridge.warpPerspective(frame, corners!!)
        assertTrue("Warped width must be in expected range (~60px)", warped.width in 55u..65u)
        assertTrue("Warped height must be in expected range (~60px)", warped.height in 55u..65u)
        assertEquals("Warped format should remain RGBA8", FfiScanPixelFormat.RGBA8, warped.format)
    }

    @Test
    fun testDeskewAndAdaptiveRotation() {
        val frame = createSyntheticDocumentFrame(width = 80u, height = 60u)
        val deskew = bridge.deskewFrame(frame)
        assertTrue("Deskew angle must be within ±15 degrees", kotlin.math.abs(deskew.skewAngleDegrees) <= 15.0f)

        val rotated90 = bridge.rotateFrame(frame, 1u)
        assertEquals("Rotated frame width must equal source height", 60u, rotated90.width)
        assertEquals("Rotated frame height must equal source width", 80u, rotated90.height)
        assertEquals("Orientation degrees must be 90", 90u, rotated90.orientationDegrees)

        // Landscape frame auto-oriented to portrait:
        val oriented = bridge.autoOrientFrame(frame, preferPortrait = true)
        assertEquals("Auto-oriented width must be 60", 60u, oriented.width)
        assertEquals("Auto-oriented height must be 80", 80u, oriented.height)
    }

    private fun createSyntheticShadowedDocumentFrame(width: UInt, height: UInt): FfiScanFrame {
        val totalBytes = (width * height * 4u).toInt()
        val pixels = ByteArray(totalBytes) { 255.toByte() }
        val w = width.toInt()
        val h = height.toInt()
        for (y in 0 until h) {
            val yf = y.toFloat() / h.toFloat()
            val row = y * w * 4
            for (x in 0 until w) {
                val xf = x.toFloat() / w.toFloat()
                var bg = 110f + xf * 130f
                if (kotlin.math.abs(yf - 0.5f) < 0.02f) {
                    bg *= 0.75f
                }
                val isText = (y % 20 < 4) && (x in 20 until (w - 20))
                val lum = if (isText) 25f else bg
                val idx = row + (x * 4)
                val byteVal = lum.toInt().coerceIn(0, 255).toByte()
                pixels[idx] = byteVal
                pixels[idx + 1] = byteVal
                pixels[idx + 2] = byteVal
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
            deviceAngleDegrees = 0.0f,
            brightnessHint = 0.8f,
            isSteady = true
        )
    }

    @Test
    fun testCleanupProfilesAndVisualRegressionMetrics() {
        val frame = createSyntheticShadowedDocumentFrame(width = 128u, height = 128u)

        // 1. DocumentAuto profile:
        val autoEnhanced = bridge.enhanceScanFrame(frame, FfiScanCleanupProfile.DOCUMENT_AUTO)
        assertEquals("Enhanced width must match", 128u, autoEnhanced.width)
        assertEquals("Enhanced height must match", 128u, autoEnhanced.height)

        val autoMetrics = bridge.evaluateCleanupQuality(frame, autoEnhanced)
        assertTrue("Background uniformity should be >= 0.90 (got ${autoMetrics.backgroundUniformity})", autoMetrics.backgroundUniformity >= 0.90f)
        assertTrue("Contrast ratio should be >= 0.80 (got ${autoMetrics.contrastRatio})", autoMetrics.contrastRatio >= 0.80f)
        assertTrue("Shadow attenuation should be >= 0.80 (got ${autoMetrics.shadowAttenuation})", autoMetrics.shadowAttenuation >= 0.80f)

        // 2. HighContrastBw profile:
        val bwEnhanced = bridge.enhanceScanFrame(frame, FfiScanCleanupProfile.HIGH_CONTRAST_BW)
        val bwMetrics = bridge.evaluateCleanupQuality(frame, bwEnhanced)
        assertTrue("HighContrastBw must be binarized", bwMetrics.isBinarized)
        assertTrue("Binarized background uniformity >= 0.95", bwMetrics.backgroundUniformity >= 0.95f)

        // 3. PreservePhoto profile:
        val photoEnhanced = bridge.enhanceScanFrame(frame, FfiScanCleanupProfile.PRESERVE_PHOTO)
        val photoMetrics = bridge.evaluateCleanupQuality(frame, photoEnhanced)
        assertTrue("PreservePhoto must not be binarized", !photoMetrics.isBinarized)

        // 4. Session enhanced page retrieval:
        val sessionId = bridge.startScanSession()
        val page = bridge.appendScanPage(sessionId, frame, null, FfiScanCleanupProfile.DOCUMENT_AUTO)
        val retrievedEnhanced = bridge.getEnhancedScanPage(sessionId, page.pageIndex)
        val warpedPage = bridge.getWarpedScanPage(sessionId, page.pageIndex)
        assertEquals("Retrieved enhanced frame width must match warped width", warpedPage.width, retrievedEnhanced.width)
        assertTrue("Retrieved enhanced frame width must be positive", retrievedEnhanced.width > 0u)

        // 5. M1.3 OCR on session page:
        val ocrResult = bridge.ocrScanSessionPage(sessionId, page.pageIndex, listOf("deu", "eng"))
        assertNotNull("OCR result must be produced", ocrResult)
        assertTrue("Frame width must match", ocrResult.frameWidth > 0u)
        assertTrue("Frame height must match", ocrResult.frameHeight > 0u)
        bridge.closeScanSession(sessionId)
    }

    @Test
    fun testLanguageDetectionAndOcrPipeline() {
        val deuText = "Vereinbarung über Vertraulichkeit und Geschäftsgeheimnisse gemäß ABGB. Erfüllungsort ist Wien, Österreich."
        val detectedDeu = bridge.detectTextLanguage(deuText)
        assertEquals("Primary language must be German", "deu", detectedDeu)

        val engText = "Mutual Non-Disclosure Agreement. Each receiving party shall protect proprietary information."
        val detectedEng = bridge.detectTextLanguage(engText)
        assertEquals("Primary language must be English", "eng", detectedEng)

        val pgmFile = listOf(
            java.io.File("tests/corpus/ocr/deu_vertrag_vertraulichkeit.pgm"),
            java.io.File("../../tests/corpus/ocr/deu_vertrag_vertraulichkeit.pgm"),
            java.io.File("../../../tests/corpus/ocr/deu_vertrag_vertraulichkeit.pgm")
        ).first { it.exists() }
        val pgmBytes = pgmFile.readBytes()
        val realDeuFrame = FfiScanFrame(
            width = 1240u,
            height = 1755u,
            stride = 1240u,
            format = FfiScanPixelFormat.GRAY8,
            orientationDegrees = 0u,
            pixels = pgmBytes.copyOfRange(pgmBytes.size - 1240 * 1755, pgmBytes.size),
            timestampMs = System.currentTimeMillis().toULong(),
            deviceAngleDegrees = 0f,
            brightnessHint = 0.9f,
            isSteady = true
        )

        val ocrRes = bridge.ocrCleanScanFrame(realDeuFrame, FfiScanCleanupProfile.DOCUMENT_AUTO, listOf("deu", "eng"), true)
        assertNotNull("OCR on cleaned frame should return result", ocrRes)
        assertEquals("Detected language must be German", "deu", ocrRes.detectedLanguage)
        assertTrue("Real OCR must extract at least 30 words (got ${ocrRes.words.size})", ocrRes.words.size >= 30)
        assertTrue("Must extract VEREINBARUNG", ocrRes.fullText.contains("VEREINBARUNG"))
        assertTrue("Must extract VERTRAULICHKEIT", ocrRes.fullText.contains("VERTRAULICHKEIT"))
        assertTrue("Must extract ABGB", ocrRes.fullText.contains("ABGB"))
        assertTrue("Must extract Wien", ocrRes.fullText.contains("Wien"))
        assertTrue("Must extract Österreich", ocrRes.fullText.contains("Österreich"))
        assertTrue("Mean confidence must be >= 85.0% (got ${ocrRes.meanConfidence}%)", ocrRes.meanConfidence >= 85.0f)
        assertTrue("Word bounding boxes must be valid", ocrRes.words.all { (it.bbox.x1 > it.bbox.x0) && (it.bbox.y1 > it.bbox.y0) })
    }
}
