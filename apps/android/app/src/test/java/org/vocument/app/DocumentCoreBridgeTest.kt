package org.vocument.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiJuristPaletteEntry
import uniffi.vocument_ffi.DocumentCoreException
import uniffi.vocument_ffi.FfiRedactionCheck
import uniffi.vocument_ffi.FfiRedactionOccurrence
import uniffi.vocument_ffi.FfiRedactionReport
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiTileRenderRequest

class DocumentCoreBridgeTest {
    /**
     * The report of a redaction is only accepted when every check has
     * succeeded. A check that was not carried out is not a pass — the
     * content of the document may still be there.
     */
    @Test
    fun aRedactionWhoseVerificationDidNotSucceedIsRefused() {
        val report = FfiRedactionReport(
            pageIndex = 0u,
            regionCount = 1u,
            removedWords = listOf("VERTRAULICH"),
            removedWordCount = 1u,
            textLayerClean = FfiRedactionCheck.PASSED,
            searchIndexClean = FfiRedactionCheck.PASSED,
            renderedPageClean = FfiRedactionCheck.NOT_CHECKED,
            fileBytesClean = FfiRedactionCheck.PASSED,
            remainingOccurrences = emptyList(),
            uninspectedPlaces = emptyList(),
            verified = true,
            revision = 2u
        )
        val failure = runCatching { requireVerifiedRedaction(0u, report) }.exceptionOrNull()
        assertTrue(
            "a report with a check that was not carried out must be refused",
            failure is DocumentCoreException.RedactionUnverified
        )
        assertTrue(
            "the error has to name the check that failed",
            failure?.message?.contains("the picture of the page") == true
        )
    }

    @Test
    fun thePlacesWhereTheSameTextStillStandsAreTurnedIntoWarnings() {
        val report = FfiRedactionReport(
            pageIndex = 0u,
            regionCount = 1u,
            removedWords = listOf("VERTRAULICH"),
            removedWordCount = 1u,
            textLayerClean = FfiRedactionCheck.PASSED,
            searchIndexClean = FfiRedactionCheck.PASSED,
            renderedPageClean = FfiRedactionCheck.PASSED,
            fileBytesClean = FfiRedactionCheck.PASSED,
            remainingOccurrences = listOf(
                FfiRedactionOccurrence(
                    "content",
                    1,
                    "content stream of the page"
                )
            ),
            uninspectedPlaces = listOf(
                FfiRedactionOccurrence(
                    "uninspected",
                    -1,
                    "/Root/Names/EmbeddedFiles/Names[1]"
                )
            ),
            verified = true,
            revision = 2u
        )
        val warnings = redactionWarnings(report)
        assertEquals(
            "every place the user has to know about is named: $warnings",
            2,
            warnings.size
        )
        assertTrue(
            "the place where the same text still stands is named",
            warnings.any { it.contains("still stands") && it.contains("page 1") }
        )
        assertTrue(
            "the place that could not be read is named",
            warnings.any { it.contains("could not be read") }
        )
    }

    @Test
    fun theRailOfTheProfessionReachesTheCaller() {
        val bridge = DocumentCoreBridge()
        val rail = bridge.juristPalette()
        assertTrue(
            "the rail carries more than one colour: $rail",
            rail.size >= 5
        )
        val objection = rail.firstOrNull { it.key == "objection" }
        assertTrue(
            "the rail knows the objection: $rail",
            objection != null
        )
        assertTrue(
            "every colour of the rail means something",
            rail.all { it.meaning.isNotBlank() && it.label.isNotBlank() }
        )
    }

    @Test
    fun theRailOfTheUserMayReplaceTheDefault() {
        val bridge = DocumentCoreBridge()
        val before = bridge.juristPalette()
        try {
            bridge.setJuristPalette(
                listOf(
                    FfiJuristPaletteEntry(
                        "house",
                        "Hausfarbe",
                        "Die Farbe des Hauses.",
                        12u,
                        34u,
                        56u
                    )
                )
            )
            val rail = bridge.juristPalette()
            assertEquals("the rail of the user has one entry", 1, rail.size)
            assertEquals("house", rail[0].key)
            assertTrue("the rail was replaced", rail.size != before.size)
        } finally {
            // The rail of the process is shared: the test puts it back, so
            // that the other tests see the standard rail.
            bridge.setJuristPalette(before)
        }
    }

    @Test
    fun verifyAndroidPlatformUniFFIHandshakePdfSessionAndRendering() {
        val bridge = DocumentCoreBridge()
        val handshake = bridge.performHandshake()

        assertEquals("0.1", handshake.baselineVersion)
        assertEquals("M2", handshake.milestone)
        assertTrue("NET-001 strict offline default must be true", handshake.strictOfflineDefault)
        assertFalse("LOG-001 telemetry must be false", handshake.telemetryEnabled)
        assertEquals(60u, handshake.targetFps)
        assertEquals("de-AT", handshake.defaultGermanVariant)
        assertEquals(listOf("deu", "eng"), handshake.builtInOcrLanguages)

        bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
        bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

        val sampleFile = listOf(
            File("../../../tests/corpus/m0_1_sample_two_pages.pdf"),
            File("../../tests/corpus/m0_1_sample_two_pages.pdf"),
            File("tests/corpus/m0_1_sample_two_pages.pdf")
        ).firstOrNull { it.exists() } ?: error("m0_1_sample_two_pages.pdf not found")
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

        // The inspection of the links, the attachments and the signatures is
        // reachable for the user: the bridge calls the core and hands the
        // report over.
        val inspection = bridge.inspectDocument(session.sessionId)
        assertTrue(
            "the report must carry the findings of the document",
            inspection.actions.isNotEmpty() || inspection.attachments.isNotEmpty() ||
                inspection.signatures.isNotEmpty() || inspection.riskyCount == 0u
        )
        assertEquals(
            "the number of the risky findings must match the list",
            inspection.riskyCount,
            (inspection.actions.count { it.isRisky } + inspection.attachments.count { it.isRisky }).toUInt()
        )

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

        val m03File = listOf(
            File("../../../tests/corpus/m0_3_navigation_and_search.pdf"),
            File("../../tests/corpus/m0_3_navigation_and_search.pdf"),
            File("tests/corpus/m0_3_navigation_and_search.pdf")
        ).firstOrNull { it.exists() } ?: error("m0_3_navigation_and_search.pdf not found")
        val m03Session = bridge.openSafDocument(
            safUriString = "content://com.android.providers.downloads.documents/document/m0_3_test",
            displayName = m03File.name,
            safDocumentId = "downloads:m0_3_test",
            lastModifiedUnixMs = m03File.lastModified(),
            bytes = m03File.readBytes()
        )
        val descriptors = bridge.pageDescriptors(m03Session.sessionId)
        assertEquals(listOf("i", "ii", "1", "2"), descriptors.map { it.logicalLabel })

        val navTarget = bridge.resolvePageNavigation(m03Session.sessionId, "1")
        assertEquals(2u, navTarget.pageIndex)

        val thumb = bridge.renderPageThumbnail(m03Session.sessionId, 0u, 128u)
        assertEquals(128u, thumb.heightPx)

        val selection = bridge.selectPageText(m03Session.sessionId, 1u, null)
        assertTrue(selection.cleanedText.contains("trustworthy"))

        val progress = bridge.indexSearchBatch(m03Session.sessionId, 2u, 1u)
        assertEquals(1u, progress.indexedPages)
        assertFalse(progress.isComplete)

        val searchResponse = bridge.searchExact(m03Session.sessionId, "SQLite FTS5")
        assertFalse(searchResponse.progress.isComplete)
        assertEquals(1, searchResponse.hits.size)
        assertEquals(2u, searchResponse.hits[0].pageIndex)

        val histAfterHighlight = bridge.addHighlightAnnotation(
            m03Session.sessionId,
            uniffi.vocument_ffi.FfiHighlightAnnotation(
                annotationId = "annot_junit_001",
                pageIndex = 0u,
                rect = selection.highlightRects.first(),
                colorR = 255u,
                colorG = 230u,
                colorB = 80u,
                opacityMilli = 450u,
                author = "JUnit",
                contents = "M0.4 test highlight"
            )
        )
        assertTrue(histAfterHighlight.isDirty)
        assertEquals(1u, histAfterHighlight.undoCount)

        val histAfterRot = bridge.rotatePages(m03Session.sessionId, listOf(0u), 90)
        assertEquals(2u, histAfterRot.undoCount)
        assertEquals(90u, bridge.pageRotationDegrees(m03Session.sessionId, 0u))

        val histUndo = bridge.undo(m03Session.sessionId)
        assertTrue(histUndo.canRedo)
        assertEquals(0u, bridge.pageRotationDegrees(m03Session.sessionId, 0u))

        val histRedo = bridge.redo(m03Session.sessionId)
        assertFalse(histRedo.canRedo)
        assertEquals(90u, bridge.pageRotationDegrees(m03Session.sessionId, 0u))

        val journalJson = bridge.exportTransactionJournalJson(m03Session.sessionId)
        assertTrue(journalJson.contains("\"schema_version\": 1"))

        bridge.closeSession(m03Session.sessionId)
    }
}
