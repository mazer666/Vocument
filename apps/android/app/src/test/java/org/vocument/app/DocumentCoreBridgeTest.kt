package org.vocument.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiTileRenderRequest

class DocumentCoreBridgeTest {
    @Test
    fun verifyAndroidPlatformUniFFIHandshakePdfSessionAndRendering() {
        val bridge = DocumentCoreBridge()
        val handshake = bridge.performHandshake()

        assertEquals("0.1", handshake.baselineVersion)
        assertEquals("M0", handshake.milestone)
        assertTrue("NET-001 strict offline default must be true", handshake.strictOfflineDefault)
        assertFalse("LOG-001 telemetry must be false", handshake.telemetryEnabled)
        assertEquals(60u, handshake.targetFps)
        assertEquals("de-AT", handshake.defaultGermanVariant)
        assertEquals(listOf("deu", "eng"), handshake.builtInOcrLanguages)

        bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
        bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

        val sampleFile = File("../../tests/corpus/m0_1_sample_two_pages.pdf")
        val session = bridge.openSafDocument(
            safUriString = "content://com.android.providers.downloads.documents/document/m0_1_test",
            displayName = sampleFile.name,
            safDocumentId = "downloads:m0_1_test",
            lastModifiedUnixMs = sampleFile.lastModified(),
            bytes = sampleFile.readBytes()
        )
        assertEquals("doc_1a00192b75fb403b", session.documentId)
        assertEquals(FfiFileAccessOrigin.ANDROID_SAF_URI, session.accessOrigin)
        assertEquals(2u, session.pdfSummary.pageCount)
        assertEquals("Vocument M0.1 Sample Document", session.pdfSummary.title)

        val geo = bridge.pageGeometry(session.sessionId, 0u)
        assertEquals(595_000u, geo.widthMilliPt)
        assertEquals(842_000u, geo.heightMilliPt)

        val viewport = bridge.renderViewportWithPrefetch(
            session.sessionId,
            FfiTileRenderRequest(
                pageIndex = 0u,
                scaleMilli = 1000u,
                tileXPx = 0u,
                tileYPx = 0u,
                tileWidthPx = 595u,
                tileHeightPx = 842u
            )
        )
        assertEquals(595u, viewport.visibleTile.widthPx)
        assertEquals(842u, viewport.visibleTile.heightPx)
        assertEquals(listOf(1u), viewport.prefetchedPages)

        val argbPixels = bridge.decodeRgba8ToArgbPixels(viewport.visibleTile)
        assertEquals(595 * 842, argbPixels.size)

        val criticalStats = bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
        assertEquals(0u, criticalStats.cachedTileCount)
        bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

        bridge.closeSession(session.sessionId)
    }
}
