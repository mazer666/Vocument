package org.vocument.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.Test
import uniffi.vocument_ffi.FfiPdfRectMilliPt

/**
 * Android test verifying native Page Workbench, Geometry, Crop and Deskew operations (`ROADMAP.md` M3.3, `CROP-001`–`003`, `ROTATE-001`–`003`).
 */
class WorkbenchBridgeTest {
    companion object {
        @JvmStatic
        @BeforeClass
        fun setUpNativeLibrary() {
            val candidatePaths = listOf(
                "../../target/debug",
                "../../../target/debug",
                "target/debug"
            )
            val existing = candidatePaths.firstOrNull { File(it).exists() }
            if (existing != null) {
                val absolute = File(existing).absolutePath
                val current = System.getProperty("jna.library.path")
                System.setProperty(
                    "jna.library.path",
                    if (current.isNullOrBlank()) absolute else "$current:$absolute"
                )
            }
        }
    }

    private val bridge = DocumentCoreBridge()

    private fun openSampleSession(): String {
        val sampleFile = listOf(
            File("../../../tests/corpus/m0_1_sample_two_pages.pdf"),
            File("../../tests/corpus/m0_1_sample_two_pages.pdf"),
            File("tests/corpus/m0_1_sample_two_pages.pdf")
        ).firstOrNull { it.exists() } ?: error("m0_1_sample_two_pages.pdf not found")

        val session = bridge.openSafDocument(
            safUriString = "content://com.android.providers.downloads.documents/document/m3_3_test",
            displayName = sampleFile.name,
            safDocumentId = "downloads:m3_3_test",
            lastModifiedUnixMs = sampleFile.lastModified(),
            bytes = sampleFile.readBytes()
        )
        return session.sessionId
    }

    @Test
    fun testWorkbenchPreflightAndPageBoxes() {
        val sessionId = openSampleSession()
        try {
            val preflight = bridge.workbenchPreflight(sessionId)
            assertFalse("Sample document should not be signed", preflight.isSigned)
            assertTrue("Sample document should allow assembly", preflight.allowsAssembly)

            val boxes = bridge.getPageBoxes(sessionId, 0u)
            val expectedMedia = FfiPdfRectMilliPt(0, 0, 595_000, 842_000)
            assertEquals("MediaBox must match standard A4", expectedMedia, boxes.mediaBox)
            assertEquals("Default CropBox must equal MediaBox", expectedMedia, boxes.cropBox)
        } finally {
            bridge.closeSession(sessionId)
        }
    }

    @Test
    fun testDisplayCropAndUndoRedo() {
        val sessionId = openSampleSession()
        try {
            val initialBoxes = bridge.getPageBoxes(sessionId, 0u)
            val initialMedia = FfiPdfRectMilliPt(0, 0, 595_000, 842_000)
            assertEquals(initialMedia, initialBoxes.mediaBox)

            val cropRect = FfiPdfRectMilliPt(50_000, 50_000, 545_000, 792_000)
            val cropTx = bridge.applyDisplayCrop(sessionId, listOf(0u), cropRect)
            assertEquals("Set page boundaries for page #1", cropTx.description)

            val croppedBoxes = bridge.getPageBoxes(sessionId, 0u)
            assertEquals("MediaBox must remain intact on display crop", initialMedia, croppedBoxes.mediaBox)
            assertEquals("CropBox must reflect new viewport", cropRect, croppedBoxes.cropBox)

            // Undo
            val undoHist = bridge.undo(sessionId)
            assertTrue(undoHist.canRedo)
            val restoredBoxes = bridge.getPageBoxes(sessionId, 0u)
            assertEquals("CropBox must be restored after undo", initialMedia, restoredBoxes.cropBox)

            // Redo
            val redoHist = bridge.redo(sessionId)
            assertTrue(redoHist.canUndo)
            val redoneBoxes = bridge.getPageBoxes(sessionId, 0u)
            assertEquals(cropRect, redoneBoxes.cropBox)
        } finally {
            bridge.closeSession(sessionId)
        }
    }

    @Test
    fun testHardCropAndUndoRedo() {
        val sessionId = openSampleSession()
        try {
            val cropRect = FfiPdfRectMilliPt(50_000, 50_000, 545_000, 792_000)
            val hardCropTx = bridge.applyHardCrop(sessionId, listOf(0u), cropRect)
            assertEquals("Apply hard crop to page #1", hardCropTx.description)

            val hardBoxes = bridge.getPageBoxes(sessionId, 0u)
            val expectedClamped = FfiPdfRectMilliPt(0, 0, 495_000, 742_000)
            assertEquals("MediaBox clamped to content boundary", expectedClamped, hardBoxes.mediaBox)
            assertEquals("CropBox clamped to content boundary", expectedClamped, hardBoxes.cropBox)

            // Reversible Undo
            bridge.undo(sessionId)
            val uncroppedBoxes = bridge.getPageBoxes(sessionId, 0u)
            assertEquals(FfiPdfRectMilliPt(0, 0, 595_000, 842_000), uncroppedBoxes.mediaBox)
        } finally {
            bridge.closeSession(sessionId)
        }
    }

    @Test
    fun testDeskewAngleAndRotation() {
        val sessionId = openSampleSession()
        try {
            val detectedAngle = bridge.detectDeskewAngle(sessionId, 0u)
            assertEquals(0.0f, detectedAngle, 0.01f)

            val deskewTx = bridge.deskewPages(sessionId, listOf(0u), 2500)
            assertEquals("Deskew page #1 by +2.5°", deskewTx.description)

            bridge.undo(sessionId)
        } finally {
            bridge.closeSession(sessionId)
        }
    }

    @Test
    fun testAutoCropMargins() {
        val sessionId = openSampleSession()
        try {
            val autoCropTx = bridge.autoCropPages(sessionId, listOf(0u), 14170)
            assertEquals("Auto-crop page #1", autoCropTx.description)

            bridge.undo(sessionId)
        } finally {
            bridge.closeSession(sessionId)
        }
    }
}
