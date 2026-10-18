package org.vocument.app

import java.io.File
import uniffi.vocument_ffi.FfiFileAccessOrigin
import uniffi.vocument_ffi.FfiMemoryPressureLevel
import uniffi.vocument_ffi.FfiRecoveryAction
import uniffi.vocument_ffi.FfiSaveConflictResolution
import uniffi.vocument_ffi.FfiSaveOutcome
import uniffi.vocument_ffi.FfiTileRenderRequest

fun main() {
    val bridge = DocumentCoreBridge()
    val handshake = bridge.performHandshake()

    check(handshake.baselineVersion == "0.1") {
        "Expected baselineVersion '0.1', got '${handshake.baselineVersion}'"
    }
    check(handshake.milestone == "M0") {
        "Expected milestone 'M0', got '${handshake.milestone}'"
    }
    check(handshake.strictOfflineDefault) {
        "Expected strictOfflineDefault == true (NET-001)"
    }
    check(!handshake.telemetryEnabled) {
        "Expected telemetryEnabled == false (LOG-001)"
    }
    check(handshake.targetFps == 60u) {
        "Expected targetFps == 60 (PERF-001), got ${handshake.targetFps}"
    }
    check(handshake.defaultGermanVariant == "de-AT") {
        "Expected defaultGermanVariant == 'de-AT' (I18N-002), got '${handshake.defaultGermanVariant}'"
    }
    check(handshake.builtInOcrLanguages == listOf("deu", "eng")) {
        "Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got ${handshake.builtInOcrLanguages}"
    }

    println(
        "✓ Kotlin (Android) → UniFFI → Rust DocumentCore handshake verified: " +
            "baseline=${handshake.baselineVersion}, milestone=${handshake.milestone}, " +
            "offline=${handshake.strictOfflineDefault}, telemetry=${handshake.telemetryEnabled}, " +
            "fps=${handshake.targetFps}"
    )

    bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
    bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

    // M0.1 verification: open real PDF via Android SAF FileBroker → DocumentSession → MuPDF
    val openStartNs = System.nanoTime()
    val sampleFile = File("tests/corpus/m0_1_sample_two_pages.pdf")
    val pdfBytes = sampleFile.readBytes()
    val session = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_1_sample",
        displayName = sampleFile.name,
        safDocumentId = "downloads:m0_1_sample",
        lastModifiedUnixMs = sampleFile.lastModified(),
        bytes = pdfBytes
    )

    check(session.sessionId.startsWith("sess_")) {
        "Expected session_id prefix 'sess_', got '${session.sessionId}'"
    }
    check(session.documentId == "doc_1a00192b75fb403b") {
        "Expected stable document_id 'doc_1a00192b75fb403b', got '${session.documentId}'"
    }
    check(session.accessOrigin == FfiFileAccessOrigin.ANDROID_SAF_URI) {
        "Expected accessOrigin == ANDROID_SAF_URI, got ${session.accessOrigin}"
    }
    check(session.fileIdentity.sizeBytes == 697uL) {
        "Expected sizeBytes == 697, got ${session.fileIdentity.sizeBytes}"
    }
    check(session.pdfSummary.pageCount == 2u) {
        "Expected pageCount == 2, got ${session.pdfSummary.pageCount}"
    }
    check(session.pdfSummary.title == "Vocument M0.1 Sample Document") {
        "Unexpected title: ${session.pdfSummary.title}"
    }
    check(session.pdfSummary.author == "Vocument Engineering") {
        "Unexpected author: ${session.pdfSummary.author}"
    }
    check(session.pdfSummary.engineIdentifier.startsWith("mupdf-1.")) {
        "Unexpected engineIdentifier: ${session.pdfSummary.engineIdentifier}"
    }

    val fetched = bridge.getSessionSnapshot(session.sessionId)
    check(fetched == session) {
        "Fetched session snapshot did not match opened session"
    }

    println(
        "✓ Kotlin (Android M0.1) → SAF FileBroker → DocumentSession → MuPDF verified: " +
            "session=${session.sessionId}, doc=${session.documentId}, " +
            "pages=${session.pdfSummary.pageCount}, title='${session.pdfSummary.title}', " +
            "engine=${session.pdfSummary.engineIdentifier}"
    )

    // M0.2 verification: DocumentActor geometry, viewport render + prefetch, ARGB decode, zoom, and memory pressure
    val geo = bridge.pageGeometry(session.sessionId, 0u)
    check(geo.widthMilliPt == 595_000u && geo.heightMilliPt == 842_000u) {
        "Expected A4 page geometry 595000x842000 milli-pt, got ${geo.widthMilliPt}x${geo.heightMilliPt}"
    }

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
    val firstPageMs = (System.nanoTime() - openStartNs) / 1_000_000.0
    check(firstPageMs < 300.0) {
        "First-page open+render exceeded 300 ms budget: $firstPageMs ms"
    }
    check(viewport.visibleTile.widthPx == 595u && viewport.visibleTile.heightPx == 842u) {
        "Unexpected rendered tile dimensions"
    }
    check(viewport.prefetchedPages == listOf(1u)) {
        "Expected prefetchedPages == [1], got ${viewport.prefetchedPages}"
    }
    val argbPixels = bridge.decodeRgba8ToArgbPixels(viewport.visibleTile)
    check(argbPixels.size == 595 * 842) {
        "Expected 500990 ARGB pixels, got ${argbPixels.size}"
    }

    // Scroll to page 1 (must hit prefetched tile in TileCache)
    val hitsBefore = viewport.cacheStats.hitCount
    val page1Tile = bridge.renderVisibleTile(
        session.sessionId,
        FfiTileRenderRequest(
            pageIndex = 1u,
            scaleMilli = 1000u,
            tileXPx = 0u,
            tileYPx = 0u,
            tileWidthPx = 595u,
            tileHeightPx = 842u
        )
    )
    check(page1Tile.pageIndex == 1u) { "Expected pageIndex == 1" }
    val statsAfterHit = bridge.tileCacheStats()
    check(statsAfterHit.hitCount == hitsBefore + 1uL) {
        "Expected TileCache hit on prefetched page 1"
    }

    // Zoom to 200% (scaleMilli = 2000) sub-tile
    val zoomedTile = bridge.renderVisibleTile(
        session.sessionId,
        FfiTileRenderRequest(
            pageIndex = 0u,
            scaleMilli = 2000u,
            tileXPx = 0u,
            tileYPx = 0u,
            tileWidthPx = 256u,
            tileHeightPx = 256u
        )
    )
    check(zoomedTile.widthPx == 256u && zoomedTile.heightPx == 256u) {
        "Expected 256x256 zoomed tile"
    }

    // Memory pressure Warning + Critical verification
    val warningStats = bridge.handleMemoryPressure(FfiMemoryPressureLevel.WARNING)
    check(warningStats.pressureLevel == FfiMemoryPressureLevel.WARNING) {
        "Expected pressureLevel == WARNING"
    }
    val criticalStats = bridge.handleMemoryPressure(FfiMemoryPressureLevel.CRITICAL)
    check(criticalStats.cachedTileCount == 0u && criticalStats.currentUsedBytes == 0uL) {
        "Expected critical memory pressure to purge TileCache"
    }
    bridge.handleMemoryPressure(FfiMemoryPressureLevel.NORMAL)

    bridge.closeSession(session.sessionId)

    val formattedMs = String.format(java.util.Locale.ROOT, "%.2f", firstPageMs)
    println(
        "✓ Kotlin (Android M0.2) → DocumentActor → DisplayList → TileCache → ARGB Bitmap verified: " +
            "first_page_ms=$formattedMs, tile=595x842, prefetched=${viewport.prefetchedPages}, " +
            "zoom=200%, memory_pressure=ok"
    )

    // M0.3 verification: Navigation, Logical Page Labels, ThumbnailCache, Cleaned Plain-Text Copy, and Progressive SQLite FTS5 Search
    val m03File = File("tests/corpus/m0_3_navigation_and_search.pdf")
    val m03Session = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_3_sample",
        displayName = m03File.name,
        safDocumentId = "downloads:m0_3_sample",
        lastModifiedUnixMs = m03File.lastModified(),
        bytes = m03File.readBytes()
    )
    check(m03Session.pdfSummary.pageCount == 4u) {
        "Expected 4 pages in M0.3 sample PDF, got ${m03Session.pdfSummary.pageCount}"
    }

    // 1. Page descriptors & logical labels (PAGELABEL-001..004)
    val descriptors = bridge.pageDescriptors(m03Session.sessionId)
    val labels = descriptors.map { it.logicalLabel }
    check(labels == listOf("i", "ii", "1", "2")) {
        "Expected embedded logical labels ['i', 'ii', '1', '2'], got $labels"
    }
    check(descriptors[2].physicalPageNumber == 3u) {
        "Expected physicalPageNumber == 3 for index 2"
    }
    val navRoman = bridge.resolvePageNavigation(m03Session.sessionId, "ii")
    check(navRoman.pageIndex == 1u && navRoman.physicalPageNumber == 2u) {
        "Expected 'ii' to resolve to pageIndex=1"
    }
    val navArabic = bridge.resolvePageNavigation(m03Session.sessionId, "1")
    check(navArabic.pageIndex == 2u && navArabic.physicalPageNumber == 3u) {
        "Expected logical '1' to resolve to pageIndex=2 (physical #3)"
    }
    val navPhysical = bridge.resolvePageNavigation(m03Session.sessionId, "#1")
    check(navPhysical.pageIndex == 0u) {
        "Expected '#1' to resolve to pageIndex=0"
    }

    // 2. Thumbnail rendering & separate ThumbnailCache (PAGEVIEW-001, PAGEVIEW-002)
    val thumbStatsBefore = bridge.thumbnailCacheStats()
    val thumb0 = bridge.renderPageThumbnail(m03Session.sessionId, 0u, 160u)
    check(thumb0.heightPx == 160u && thumb0.widthPx > 100u) {
        "Unexpected thumbnail dimensions: ${thumb0.widthPx}x${thumb0.heightPx}"
    }
    val thumbPixels = bridge.decodeThumbnailToArgbPixels(thumb0)
    check(thumbPixels.size == (thumb0.widthPx * thumb0.heightPx).toInt()) {
        "Unexpected decoded thumbnail ARGB pixel count"
    }
    bridge.renderPageThumbnail(m03Session.sessionId, 0u, 160u)
    val thumbStatsAfter = bridge.thumbnailCacheStats()
    check(thumbStatsAfter.hitCount == thumbStatsBefore.hitCount + 1uL) {
        "Expected ThumbnailCache hit on second thumbnail request"
    }

    // 3. Text selection & cleaned plain-text copy (OCRCOPY-001, OCRCOPY-002)
    val selection = bridge.selectPageText(m03Session.sessionId, 1u, null)
    check(selection.rawText.contains("trust-")) {
        "Expected rawText to preserve hyphenated line break 'trust-'"
    }
    check(selection.cleanedText.contains("trustworthy") && !selection.cleanedText.contains("trust-")) {
        "Expected cleanedText to rejoin 'trust-\\nworthy' into 'trustworthy', got: ${selection.cleanedText}"
    }
    check(selection.cleanedText.contains("\n\nSecond paragraph")) {
        "Expected cleanedText to preserve paragraph break"
    }

    // 4. Progressive SQLite FTS5 Search BEFORE indexing completes (M0.3 Gate, SEARCH-001, SEARCH-002, SEARCH-009)
    val partialProgress = bridge.indexSearchBatch(m03Session.sessionId, 2u, 1u)
    check(partialProgress.indexedPages == 1u && !partialProgress.isComplete) {
        "Expected partial indexing state (1/4 pages)"
    }

    val searchStartNs = System.nanoTime()
    val partialSearch = bridge.searchExact(m03Session.sessionId, "SQLite FTS5")
    val searchMs = (System.nanoTime() - searchStartNs) / 1_000_000.0
    check(searchMs < 150.0) {
        "Classic FTS5 search exceeded 150 ms target: $searchMs ms"
    }
    check(!partialSearch.progress.isComplete && partialSearch.progress.indexedPages == 1u) {
        "Expected search response to reflect partial indexing state"
    }
    check(
        partialSearch.hits.size == 1 &&
            partialSearch.hits[0].pageIndex == 2u &&
            partialSearch.hits[0].logicalPageLabel == "1"
    ) {
        "Expected partial FTS5 search hit on pageIndex=2 (label '1')"
    }

    val finalProgress = bridge.indexSearchBatch(m03Session.sessionId, 0u, 10u)
    check(finalProgress.isComplete && finalProgress.indexedPages == 4u) {
        "Expected full FTS5 index completion (4/4 pages)"
    }

    bridge.closeSession(m03Session.sessionId)
    val formattedSearchMs = String.format(java.util.Locale.ROOT, "%.2f", searchMs)
    println(
        "✓ Kotlin (Android M0.3) → PageLabels(${labels.joinToString(",")}) → " +
            "ThumbnailCache(${thumb0.widthPx}x${thumb0.heightPx}) → CleanedCopy(OCRCOPY-002) → " +
            "SQLite FTS5 Search verified: partial_hits=${partialSearch.hits.size} (1/4 indexed), search_ms=$formattedSearchMs"
    )

    // M0.4 verification: Highlight Annotation, Page Rotation, Semantic Transaction Commands, Undo/Redo, and Persistent Transaction Journal
    val m04Session = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_4_sample",
        displayName = m03File.name,
        safDocumentId = "downloads:m0_4_sample",
        lastModifiedUnixMs = m03File.lastModified(),
        bytes = m03File.readBytes()
    )
    val initialHist = bridge.transactionHistory(m04Session.sessionId)
    check(!initialHist.isDirty && !initialHist.canUndo && initialHist.currentRevision == 1uL) {
        "Expected clean initial transaction history at revision 1"
    }

    val selPage0 = bridge.selectPageText(m04Session.sessionId, 0u, null)
    val firstRect = selPage0.highlightRects.first()

    val histAfterHighlight = bridge.addHighlightAnnotation(
        m04Session.sessionId,
        uniffi.vocument_ffi.FfiHighlightAnnotation(
            annotationId = "annot_kotlin_cli_001",
            pageIndex = 0u,
            rect = firstRect,
            colorR = 255u,
            colorG = 230u,
            colorB = 80u,
            opacityMilli = 450u,
            author = "Vocument Android CLI",
            contents = "M0.4 Kotlin highlight annotation"
        )
    )
    check(histAfterHighlight.isDirty && histAfterHighlight.canUndo && histAfterHighlight.undoCount == 1u) {
        "Expected dirty session with 1 undo item after addHighlightAnnotation"
    }
    check(histAfterHighlight.undoDescription == "Add highlight annotation on page #1") {
        "Unexpected undoDescription: ${histAfterHighlight.undoDescription}"
    }

    val page0Highlights = bridge.pageHighlightAnnotations(m04Session.sessionId, 0u)
    check(page0Highlights.size == 1 && page0Highlights[0].annotationId == "annot_kotlin_cli_001") {
        "Expected 1 highlight annotation on page 0"
    }

    val histAfterRotate = bridge.rotatePages(m04Session.sessionId, listOf(0u), 90)
    check(histAfterRotate.undoCount == 2u && histAfterRotate.undoDescription == "Rotate page #1 by +90°") {
        "Expected 2 undo items after page rotation"
    }
    val rotDegrees = bridge.pageRotationDegrees(m04Session.sessionId, 0u)
    val rotGeo = bridge.pageGeometry(m04Session.sessionId, 0u)
    check(rotDegrees == 90u && rotGeo.widthMilliPt == 842_000u && rotGeo.heightMilliPt == 595_000u) {
        "Expected rotated page 0 geometry 842000x595000 at 90°, got ${rotGeo.widthMilliPt}x${rotGeo.heightMilliPt} at ${rotDegrees}°"
    }

    val histAfterUndo = bridge.undo(m04Session.sessionId)
    check(histAfterUndo.canRedo && histAfterUndo.undoCount == 1u) {
        "Expected canRedo == true and undoCount == 1 after undo"
    }
    check(bridge.pageRotationDegrees(m04Session.sessionId, 0u) == 0u) {
        "Expected rotation 0° after undo"
    }

    val histAfterRedo = bridge.redo(m04Session.sessionId)
    check(!histAfterRedo.canRedo && histAfterRedo.undoCount == 2u) {
        "Expected canRedo == false and undoCount == 2 after redo"
    }

    val journalJson = bridge.exportTransactionJournalJson(m04Session.sessionId)
    check(journalJson.contains("\"schema_version\": 1") && journalJson.contains("annot_kotlin_cli_001")) {
        "Expected exported persistent transaction journal JSON to contain schema_version 1 and annotation ID"
    }

    val replaySession = bridge.openSafDocument(
        safUriString = "content://com.android.providers.downloads.documents/document/m0_4_replay",
        displayName = m03File.name,
        safDocumentId = "downloads:m0_4_replay",
        lastModifiedUnixMs = m03File.lastModified(),
        bytes = m03File.readBytes()
    )
    val replayedHist = bridge.restoreTransactionJournalJson(replaySession.sessionId, journalJson)
    check(replayedHist.isDirty && replayedHist.undoCount == 2u) {
        "Expected replayed session to restore 2 committed transactions"
    }
    val replayedRot = bridge.pageRotationDegrees(replaySession.sessionId, 0u)
    val replayedAnnots = bridge.pageHighlightAnnotations(replaySession.sessionId, 0u)
    check(replayedRot == 90u && replayedAnnots.size == 1) {
        "Expected replayed session to have rotation 90° and 1 highlight annotation"
    }

    bridge.closeSession(m04Session.sessionId)
    bridge.closeSession(replaySession.sessionId)
    println(
        "✓ Kotlin (Android M0.4) → TransactionManager → HighlightAnnotation(${replayedAnnots[0].annotationId}) → " +
            "RotatePages(+90° → ${rotGeo.widthMilliPt / 1000u}x${rotGeo.heightMilliPt / 1000u}pt) → Undo/Redo → " +
            "PersistentTransactionJournal(replay=${replayedHist.undoCount} tx) verified"
    )

    // M0.5 verification: Safe Save, Atomic Staging, Post-Save Validation, External Conflict Detection, Gate, and Collision-Free Safe Copy
    val m05TempDir = File(System.getProperty("java.io.tmpdir"), "voc_kt_m05_${System.currentTimeMillis()}")
    m05TempDir.mkdirs()
    try {
        val m05TestPdf = File(m05TempDir, "contract.pdf")
        val samplePdf = File("tests/corpus/m0_1_sample_two_pages.pdf")
        val origBytes = samplePdf.readBytes()
        m05TestPdf.writeBytes(origBytes)

        val m05Session = bridge.openLocalDocument(m05TestPdf.absolutePath)

        // Mutate session:
        bridge.rotatePages(m05Session.sessionId, listOf(0u), 90)
        val dirtySnap = bridge.getSessionSnapshot(m05Session.sessionId)
        check(dirtySnap.isDirty) {
            "Expected session to be dirty after mutation"
        }

        // 1. Safe save dirty session:
        val saveOutcome = bridge.saveSession(m05Session.sessionId)
        check(saveOutcome is FfiSaveOutcome.Success) {
            "Expected successful save outcome on clean target"
        }
        check(saveOutcome.snapshot.savedRevision == 2uL) {
            "Expected saved revision == 2, got ${saveOutcome.snapshot.savedRevision}"
        }
        val cleanSnap = bridge.getSessionSnapshot(m05Session.sessionId)
        check(!cleanSnap.isDirty) {
            "Expected isDirty == false after successful save"
        }

        // 2. Simulate external modification:
        Thread.sleep(20)
        val extBytes = origBytes + "\n%external change by another process\n".toByteArray(Charsets.UTF_8)
        m05TestPdf.writeBytes(extBytes)

        val extCheck = bridge.checkExternalChanges(m05Session.sessionId)
        check(extCheck.hasExternalChanges && extCheck.comparisonSummary.contains("ModifiedExternally")) {
            "Expected proactive external change check to detect ModifiedExternally"
        }

        // Make another local mutation:
        bridge.rotatePages(m05Session.sessionId, listOf(0u), 90)

        // 3. M0.5 GATE VERIFICATION: Save MUST NOT silently overwrite!
        val conflictOutcome = bridge.saveSession(m05Session.sessionId)
        check(conflictOutcome is FfiSaveOutcome.Conflict) {
            "FATAL: M0.5 Gate breached! External file was silently overwritten."
        }
        check(conflictOutcome.conflict.comparisonSummary.contains("ModifiedExternally")) {
            "Expected comparisonSummary to contain ModifiedExternally"
        }
        check(conflictOutcome.conflict.suggestedSafeCopyPath.contains("Conflicted Copy")) {
            "Expected suggestedSafeCopyPath to suggest Conflicted Copy"
        }

        // Verify disk content was NOT overwritten:
        val onDisk = m05TestPdf.readBytes()
        check(onDisk.contentEquals(extBytes)) {
            "FATAL: Disk content was overwritten or corrupted!"
        }

        // 4. Resolve conflict via collision-free safe copy:
        val safeCopySnap = bridge.resolveSaveConflict(
            m05Session.sessionId,
            FfiSaveConflictResolution.SAVE_LOCAL_AS_COLLISION_FREE_COPY
        )
        check(safeCopySnap != null && safeCopySnap.targetToken.contains("Safe Copy")) {
            "Expected safe copy snapshot with collision-free path"
        }
        check(File(safeCopySnap.targetToken).exists()) {
            "Expected safe copy file to exist on disk: ${safeCopySnap.targetToken}"
        }

        // 5. Collision-free path generator verification:
        val collPath = bridge.generateCollisionFreePath(m05TestPdf.absolutePath, "Safe Copy")
        check(collPath.contains("Safe Copy")) {
            "Unexpected collision-free path generated: $collPath"
        }

        bridge.closeSession(m05Session.sessionId)
        println(
            "✓ Kotlin (Android M0.5) → SafeSave → AtomicStaging → PostSaveValidation → " +
                "ExternalConflictDetection → GateVerified(no-silent-overwrite) → " +
                "CollisionFreeSafeCopy(${safeCopySnap.savedRevision} rev) verified"
        )
    } finally {
        m05TempDir.deleteRecursively()
    }

    // M0.6 verification: Recovery Container, Crash Detection, Restore Action, and Gate Invariant
    val m06TempDir = File(System.getProperty("java.io.tmpdir"), "voc_kt_m06_${ProcessHandle.current().pid()}")
    m06TempDir.mkdirs()
    try {
        val m06TestPdf = File(m06TempDir, "m06_rec_test.pdf")
        val samplePdf = File("tests/corpus/m0_1_sample_two_pages.pdf")
        val origBytes = samplePdf.readBytes()
        m06TestPdf.writeBytes(origBytes)
        val initialM06Bytes = m06TestPdf.readBytes()

        val m06Session = bridge.openLocalDocument(m06TestPdf.absolutePath)
        val docId = m06Session.documentId

        // Mutate session:
        bridge.rotatePages(m06Session.sessionId, listOf(0u), 90)

        // Simulate crash: session dirty and active, check recovery before close/save
        val recInfo = bridge.checkDocumentRecovery(m06TestPdf.absolutePath)
        check(recInfo != null) {
            "Expected recovery info to be detected for dirty unclosed document"
        }
        check(recInfo.documentId == docId) {
            "Expected docId '$docId', got '${recInfo.documentId}'"
        }
        check(recInfo.uncommittedRevisions >= 1uL) {
            "Expected uncommittedRevisions >= 1, got ${recInfo.uncommittedRevisions}"
        }

        // Scan all recoverable documents:
        val allRec = bridge.scanRecoverableDocuments()
        check(allRec.any { it.documentId == docId }) {
            "Expected scanned recoverable documents to include '$docId'"
        }

        // M0.6 GATE VERIFICATION: Original disk file remains 100% byte-for-byte identical!
        val diskDuringCrash = m06TestPdf.readBytes()
        check(diskDuringCrash.contentEquals(initialM06Bytes)) {
            "FATAL: M0.6 Gate breached! Original file was modified during crash or before user save."
        }

        // Execute Restore action:
        val restoreResult = bridge.executeRecoveryAction(docId, FfiRecoveryAction.RESTORE)
        check(restoreResult.action == FfiRecoveryAction.RESTORE) {
            "Expected restore action result"
        }
        val restoredSnap = restoreResult.sessionSnapshot
        check(restoredSnap != null) {
            "Expected restored session snapshot"
        }

        // Verify disk file is STILL 100% untouched after Restore:
        val diskAfterRestore = m06TestPdf.readBytes()
        check(diskAfterRestore.contentEquals(initialM06Bytes)) {
            "FATAL: M0.6 Gate breached! Original file was modified during recovery restore."
        }

        bridge.closeSession(m06Session.sessionId)
        bridge.closeSession(restoredSnap.sessionId)
        println(
            "✓ Kotlin (Android M0.6) → RecoveryContainer → CrashDetection → " +
                "RestoreAction → GateVerified(disk-untouched-until-save) verified"
        )
    } finally {
        m06TempDir.deleteRecursively()
    }

    // M0.7 verification: Interoperability Gate & Full M0 Workflow
    val interopReport = bridge.runInteropGateSuite()
    check(interopReport.m0ScenarioPassed) {
        "M0.7 Interop Gate failed: DoD workflow did not complete successfully."
    }
    check(interopReport.mupdfReopenVerified && interopReport.mutoolVerified) {
        "M0.7 Interop Gate failed: MuPDF verification failed."
    }
    check(interopReport.coregraphicsSipsVerified && interopReport.chromePdfiumVerified) {
        "M0.7 Interop Gate failed: foreign viewer verification failed."
    }
    println(
        "✓ Kotlin (Android M0.7) → InteropGate → FullWorkflow → " +
            "MuPDF(${interopReport.mupdfReopenVerified}) → mutool(${interopReport.mutoolVerified}) → " +
            "Quartz(${interopReport.coregraphicsSipsVerified}) → PDFium(${interopReport.chromePdfiumVerified}) → AllGatesPassed"
    )
}
