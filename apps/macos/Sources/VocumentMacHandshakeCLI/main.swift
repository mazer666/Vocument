import Foundation
#if canImport(CoreGraphics)
import CoreGraphics
#endif
import VocumentMacKit
import VocumentFFI

@main
struct VocumentMacHandshakeCLI {
    static func main() throws {
        let client = DocumentCoreClient()
        let handshake = try client.performHandshake()

        guard handshake.baselineVersion == "0.1" else {
            fatalError("Expected baselineVersion '0.1', got '\(handshake.baselineVersion)'")
        }
        guard handshake.milestone == "M2" else {
            fatalError("Expected milestone 'M2', got '\(handshake.milestone)'")
        }
        guard handshake.strictOfflineDefault == true else {
            fatalError("Expected strictOfflineDefault == true (NET-001)")
        }
        guard handshake.telemetryEnabled == false else {
            fatalError("Expected telemetryEnabled == false (LOG-001)")
        }
        guard handshake.targetFps == 60 else {
            fatalError("Expected targetFps == 60 (PERF-001), got \(handshake.targetFps)")
        }
        guard handshake.defaultGermanVariant == "de-AT" else {
            fatalError("Expected defaultGermanVariant == 'de-AT' (I18N-002), got '\(handshake.defaultGermanVariant)'")
        }
        guard handshake.builtInOcrLanguages == ["deu", "eng"] else {
            fatalError("Expected builtInOcrLanguages == ['deu', 'eng'] (OCR-002), got \(handshake.builtInOcrLanguages)")
        }

        print("✓ Swift → UniFFI → Rust DocumentCore handshake verified: baseline=\(handshake.baselineVersion), milestone=\(handshake.milestone), offline=\(handshake.strictOfflineDefault), telemetry=\(handshake.telemetryEnabled), fps=\(handshake.targetFps)")

        // M0.1 verification: open real PDF via macOS ScopedLocalFileBroker → DocumentSession → MuPDF
        let _ = try client.handleMemoryPressure(level: .critical)
        let _ = try client.handleMemoryPressure(level: .normal)

        let openStart = DispatchTime.now()
        let samplePdfURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("tests/corpus/m0_1_sample_two_pages.pdf")
        let session = try client.openScopedDocument(at: samplePdfURL)

        guard session.sessionId.hasPrefix("sess_") else {
            fatalError("Expected session_id prefix 'sess_', got '\(session.sessionId)'")
        }
        guard session.documentId == "doc_1a00192b75fb403b" else {
            fatalError("Expected stable document_id 'doc_1a00192b75fb403b', got '\(session.documentId)'")
        }
        guard session.accessOrigin == .macOsScopedUrl else {
            fatalError("Expected accessOrigin == .macOsScopedUrl")
        }
        guard session.fileIdentity.sizeBytes == 697 else {
            fatalError("Expected sizeBytes == 697, got \(session.fileIdentity.sizeBytes)")
        }
        guard session.pdfSummary.pageCount == 2 else {
            fatalError("Expected pageCount == 2, got \(session.pdfSummary.pageCount)")
        }
        guard session.pdfSummary.title == "Vocument M0.1 Sample Document" else {
            fatalError("Unexpected title: \(String(describing: session.pdfSummary.title))")
        }
        guard session.pdfSummary.author == "Vocument Engineering" else {
            fatalError("Unexpected author: \(String(describing: session.pdfSummary.author))")
        }
        guard session.pdfSummary.engineIdentifier.hasPrefix("mupdf-1.") else {
            fatalError("Unexpected engineIdentifier: \(session.pdfSummary.engineIdentifier)")
        }

        let fetched = try client.sessionSnapshot(sessionId: session.sessionId)
        guard fetched == session else {
            fatalError("Fetched session snapshot did not match opened session")
        }

        print("✓ Swift (macOS M0.1) → FileBroker → DocumentSession → MuPDF verified: session=\(session.sessionId), doc=\(session.documentId), pages=\(session.pdfSummary.pageCount), title='\(session.pdfSummary.title ?? "")', engine=\(session.pdfSummary.engineIdentifier)")

        // M0.2 verification: DocumentActor page geometry, viewport render + prefetch, CGImage, zoom, and memory pressure
        let geo = try client.pageGeometry(sessionId: session.sessionId, pageIndex: 0)
        guard geo.widthMilliPt == 595_000 && geo.heightMilliPt == 842_000 else {
            fatalError("Expected A4 page geometry 595000x842000 milli-pt, got \(geo.widthMilliPt)x\(geo.heightMilliPt)")
        }

        let viewport = try client.renderViewportWithPrefetch(
            sessionId: session.sessionId,
            request: FfiTileRenderRequest(
                pageIndex: 0,
                scaleMilli: 1000,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: 595,
                tileHeightPx: 842
            )
        )
        let firstPageMs = Double(DispatchTime.now().uptimeNanoseconds - openStart.uptimeNanoseconds) / 1_000_000.0
        guard firstPageMs < 300.0 else {
            fatalError("First-page open+render exceeded 300 ms budget: \(firstPageMs) ms")
        }
        guard viewport.visibleTile.widthPx == 595 && viewport.visibleTile.heightPx == 842 else {
            fatalError("Unexpected rendered tile dimensions")
        }
        guard viewport.prefetchedPages == [1] else {
            fatalError("Expected prefetchedPages == [1], got \(viewport.prefetchedPages)")
        }
        // The conversion of the tile into a `CGImage` belongs to macOS. On a
        // platform that has no CoreGraphics the tile is still rendered and
        // measured — the picture is simply not handed to the window server.
        #if canImport(CoreGraphics)
        guard let cgImage = DocumentCoreClient.makeCGImage(from: viewport.visibleTile),
              cgImage.width == 595, cgImage.height == 842 else {
            fatalError("Failed to construct native CGImage from rendered RGBA8 tile")
        }
        #endif

        // Scroll to page 1 (must hit prefetched tile in TileCache)
        let hitsBefore = viewport.cacheStats.hitCount
        let page1Tile = try client.renderVisibleTile(
            sessionId: session.sessionId,
            request: FfiTileRenderRequest(
                pageIndex: 1,
                scaleMilli: 1000,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: 595,
                tileHeightPx: 842
            )
        )
        guard page1Tile.pageIndex == 1 else {
            fatalError("Expected pageIndex == 1")
        }
        let statsAfterHit = try client.tileCacheStats()
        guard statsAfterHit.hitCount == hitsBefore + 1 else {
            fatalError("Expected TileCache hit on prefetched page 1")
        }

        // Zoom to 200% (scaleMilli = 2000) sub-tile
        let zoomedTile = try client.renderVisibleTile(
            sessionId: session.sessionId,
            request: FfiTileRenderRequest(
                pageIndex: 0,
                scaleMilli: 2000,
                tileXPx: 0,
                tileYPx: 0,
                tileWidthPx: 256,
                tileHeightPx: 256
            )
        )
        guard zoomedTile.widthPx == 256 && zoomedTile.heightPx == 256 else {
            fatalError("Expected 256x256 zoomed tile")
        }

        // Memory pressure Warning + Critical verification
        let warningStats = try client.handleMemoryPressure(level: .warning)
        guard warningStats.pressureLevel == .warning else {
            fatalError("Expected pressureLevel == .warning")
        }
        let criticalStats = try client.handleMemoryPressure(level: .critical)
        guard criticalStats.cachedTileCount == 0 && criticalStats.currentUsedBytes == 0 else {
            fatalError("Expected critical memory pressure to purge TileCache")
        }
        let _ = try client.handleMemoryPressure(level: .normal)

        try client.closeSession(sessionId: session.sessionId)

        let formattedMs = String(format: "%.2f", firstPageMs)
        print("✓ Swift (macOS M0.2) → DocumentActor → DisplayList → TileCache → CGImage verified: first_page_ms=\(formattedMs), tile=595x842, prefetched=\(viewport.prefetchedPages), zoom=200%, memory_pressure=ok")

        // M0.3 verification: Navigation, Logical Page Labels, ThumbnailCache, Cleaned Plain-Text Copy, and Progressive SQLite FTS5 Search
        let m03PdfURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("tests/corpus/m0_3_navigation_and_search.pdf")
        let m03Session = try client.openScopedDocument(at: m03PdfURL)
        guard m03Session.pdfSummary.pageCount == 4 else {
            fatalError("Expected 4 pages in M0.3 sample PDF, got \(m03Session.pdfSummary.pageCount)")
        }

        // 1. Page descriptors & logical labels (PAGELABEL-001..004)
        let descriptors = try client.pageDescriptors(sessionId: m03Session.sessionId)
        let labels = descriptors.map(\.logicalLabel)
        guard labels == ["i", "ii", "1", "2"] else {
            fatalError("Expected embedded logical labels ['i', 'ii', '1', '2'], got \(labels)")
        }
        guard descriptors[2].physicalPageNumber == 3 else {
            fatalError("Expected physicalPageNumber == 3 for index 2")
        }
        let navRoman = try client.resolvePageNavigation(sessionId: m03Session.sessionId, query: "ii")
        guard navRoman.pageIndex == 1 && navRoman.physicalPageNumber == 2 else {
            fatalError("Expected 'ii' to resolve to pageIndex=1")
        }
        let navArabic = try client.resolvePageNavigation(sessionId: m03Session.sessionId, query: "1")
        guard navArabic.pageIndex == 2 && navArabic.physicalPageNumber == 3 else {
            fatalError("Expected logical '1' to resolve to pageIndex=2 (physical #3)")
        }
        let navPhysical = try client.resolvePageNavigation(sessionId: m03Session.sessionId, query: "#1")
        guard navPhysical.pageIndex == 0 else {
            fatalError("Expected '#1' to resolve to pageIndex=0")
        }

        // 2. Thumbnail rendering & separate ThumbnailCache (PAGEVIEW-001, PAGEVIEW-002)
        let thumbStatsBefore = try client.thumbnailCacheStats()
        let thumb0 = try client.renderPageThumbnail(
            sessionId: m03Session.sessionId,
            pageIndex: 0,
            maxDimensionPx: 160
        )
        guard thumb0.heightPx == 160 && thumb0.widthPx > 100 else {
            fatalError("Unexpected thumbnail dimensions: \(thumb0.widthPx)x\(thumb0.heightPx)")
        }
        #if canImport(CoreGraphics)
        guard DocumentCoreClient.makeCGImage(from: thumb0) != nil else {
            fatalError("Failed to create CGImage from thumbnail")
        }
        #endif
        let _ = try client.renderPageThumbnail(
            sessionId: m03Session.sessionId,
            pageIndex: 0,
            maxDimensionPx: 160
        )
        let thumbStatsAfter = try client.thumbnailCacheStats()
        guard thumbStatsAfter.hitCount == thumbStatsBefore.hitCount + 1 else {
            fatalError("Expected ThumbnailCache hit on second thumbnail request")
        }

        // 3. Text selection & cleaned plain-text copy (OCRCOPY-001, OCRCOPY-002)
        let selection = try client.selectPageText(
            sessionId: m03Session.sessionId,
            pageIndex: 1,
            selectionRect: nil
        )
        guard selection.rawText.contains("trust-") else {
            fatalError("Expected rawText to preserve hyphenated line break 'trust-'")
        }
        guard selection.cleanedText.contains("trustworthy") && !selection.cleanedText.contains("trust-") else {
            fatalError("Expected cleanedText to rejoin 'trust-\\nworthy' into 'trustworthy', got: \(selection.cleanedText)")
        }
        guard selection.cleanedText.contains("\n\nSecond paragraph") else {
            fatalError("Expected cleanedText to preserve paragraph break")
        }

        // 4. Progressive SQLite FTS5 Search BEFORE indexing completes (M0.3 Gate, SEARCH-001, SEARCH-002, SEARCH-009)
        let partialProgress = try client.indexSearchBatch(
            sessionId: m03Session.sessionId,
            preferredStartPage: 2,
            maxPages: 1
        )
        guard partialProgress.indexedPages == 1 && !partialProgress.isComplete else {
            fatalError("Expected partial indexing state (1/4 pages)")
        }

        let searchStart = DispatchTime.now()
        let partialSearch = try client.searchExact(
            sessionId: m03Session.sessionId,
            query: "SQLite FTS5"
        )
        let searchMs = Double(DispatchTime.now().uptimeNanoseconds - searchStart.uptimeNanoseconds) / 1_000_000.0
        guard searchMs < 150.0 else {
            fatalError("Classic FTS5 search exceeded 150 ms target: \(searchMs) ms")
        }
        guard !partialSearch.progress.isComplete && partialSearch.progress.indexedPages == 1 else {
            fatalError("Expected search response to reflect partial indexing state")
        }
        guard partialSearch.hits.count == 1 && partialSearch.hits[0].pageIndex == 2 && partialSearch.hits[0].logicalPageLabel == "1" else {
            fatalError("Expected partial FTS5 search hit on pageIndex=2 (label '1')")
        }

        let finalProgress = try client.indexSearchBatch(
            sessionId: m03Session.sessionId,
            preferredStartPage: 0,
            maxPages: 10
        )
        guard finalProgress.isComplete && finalProgress.indexedPages == 4 else {
            fatalError("Expected full FTS5 index completion (4/4 pages)")
        }

        try client.closeSession(sessionId: m03Session.sessionId)
        let formattedSearchMs = String(format: "%.2f", searchMs)
        print("✓ Swift (macOS M0.3) → PageLabels(\(labels.joined(separator: ","))) → ThumbnailCache(\(thumb0.widthPx)x\(thumb0.heightPx)) → CleanedCopy(OCRCOPY-002) → SQLite FTS5 Search verified: partial_hits=\(partialSearch.hits.count) (1/4 indexed), search_ms=\(formattedSearchMs)")

        // M0.4 verification: Highlight Annotation, Page Rotation, Semantic Transaction Commands, Undo/Redo, and Persistent Transaction Journal
        let m04Session = try client.openScopedDocument(at: m03PdfURL)
        let initialHist = try client.transactionHistory(sessionId: m04Session.sessionId)
        guard !initialHist.isDirty && !initialHist.canUndo && initialHist.currentRevision == 1 else {
            fatalError("Expected clean initial transaction history at revision 1")
        }

        let selPage0 = try client.selectPageText(
            sessionId: m04Session.sessionId,
            pageIndex: 0,
            selectionRect: nil
        )
        guard let firstRect = selPage0.highlightRects.first else {
            fatalError("Expected at least one highlight rect on page 0")
        }

        let histAfterHighlight = try client.addHighlightAnnotation(
            sessionId: m04Session.sessionId,
            annotation: FfiHighlightAnnotation(
                annotationId: "annot_mac_cli_001",
                pageIndex: 0,
                rect: firstRect,
                colorR: 255,
                colorG: 230,
                colorB: 80,
                opacityMilli: 450,
                author: "Vocument macOS CLI",
                contents: "M0.4 highlight annotation"
            )
        )
        guard histAfterHighlight.isDirty && histAfterHighlight.canUndo && histAfterHighlight.undoCount == 1 else {
            fatalError("Expected dirty session with 1 undo item after addHighlightAnnotation")
        }
        guard histAfterHighlight.undoDescription == "Add highlight annotation on page #1" else {
            fatalError("Unexpected undoDescription: \(String(describing: histAfterHighlight.undoDescription))")
        }

        let page0Highlights = try client.pageHighlightAnnotations(
            sessionId: m04Session.sessionId,
            pageIndex: 0
        )
        guard page0Highlights.count == 1 && page0Highlights[0].annotationId == "annot_mac_cli_001" else {
            fatalError("Expected 1 highlight annotation on page 0")
        }

        let histAfterRotate = try client.rotatePages(
            sessionId: m04Session.sessionId,
            pageIndices: [0],
            deltaDegrees: 90
        )
        guard histAfterRotate.undoCount == 2 && histAfterRotate.undoDescription == "Rotate page #1 by +90°" else {
            fatalError("Expected 2 undo items after page rotation")
        }
        let rotDegrees = try client.pageRotationDegrees(sessionId: m04Session.sessionId, pageIndex: 0)
        let rotGeometry = try client.pageGeometry(sessionId: m04Session.sessionId, pageIndex: 0)
        guard rotDegrees == 90 && rotGeometry.widthMilliPt == 842_000 && rotGeometry.heightMilliPt == 595_000 else {
            fatalError("Expected rotated page 0 geometry 842000x595000 at 90°, got \(rotGeometry.widthMilliPt)x\(rotGeometry.heightMilliPt) at \(rotDegrees)°")
        }

        let histAfterUndo = try client.undo(sessionId: m04Session.sessionId)
        guard histAfterUndo.canRedo && histAfterUndo.undoCount == 1 else {
            fatalError("Expected canRedo == true and undoCount == 1 after undo")
        }
        let rotAfterUndo = try client.pageRotationDegrees(sessionId: m04Session.sessionId, pageIndex: 0)
        guard rotAfterUndo == 0 else {
            fatalError("Expected rotation 0° after undo, got \(rotAfterUndo)°")
        }

        let histAfterRedo = try client.redo(sessionId: m04Session.sessionId)
        guard !histAfterRedo.canRedo && histAfterRedo.undoCount == 2 else {
            fatalError("Expected canRedo == false and undoCount == 2 after redo")
        }

        let journalJson = try client.exportTransactionJournalJson(sessionId: m04Session.sessionId)
        guard journalJson.contains("\"schema_version\": 1") && journalJson.contains("annot_mac_cli_001") else {
            fatalError("Expected exported persistent transaction journal JSON to contain schema_version 1 and annotation ID")
        }

        let replaySession = try client.openScopedDocument(at: m03PdfURL)
        let replayedHist = try client.restoreTransactionJournalJson(
            sessionId: replaySession.sessionId,
            journalJson: journalJson
        )
        guard replayedHist.isDirty && replayedHist.undoCount == 2 else {
            fatalError("Expected replayed session to restore 2 committed transactions")
        }
        let replayedRot = try client.pageRotationDegrees(sessionId: replaySession.sessionId, pageIndex: 0)
        let replayedAnnots = try client.pageHighlightAnnotations(sessionId: replaySession.sessionId, pageIndex: 0)
        guard replayedRot == 90 && replayedAnnots.count == 1 else {
            fatalError("Expected replayed session to have rotation 90° and 1 highlight annotation")
        }

        try client.closeSession(sessionId: m04Session.sessionId)
        try client.closeSession(sessionId: replaySession.sessionId)
        print("✓ Swift (macOS M0.4) → TransactionManager → HighlightAnnotation(\(replayedAnnots[0].annotationId)) → RotatePages(+90° → \(rotGeometry.widthMilliPt / 1000)x\(rotGeometry.heightMilliPt / 1000)pt) → Undo/Redo → PersistentTransactionJournal(replay=\(replayedHist.undoCount) tx) verified")

        // M0.5 verification: Safe Save, Atomic Staging, Post-Save Validation, Save As, External Conflict Detection, Gate, and Collision-Free Safe Copy
        let m05TempDir = FileManager.default.temporaryDirectory.appendingPathComponent("voc_mac_m05_\(ProcessInfo.processInfo.processIdentifier)")
        try FileManager.default.createDirectory(at: m05TempDir, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: m05TempDir)
        }
        let m05TestPdf = m05TempDir.appendingPathComponent("contract.pdf")
        let origPdfBytes = try Data(contentsOf: samplePdfURL)
        try origPdfBytes.write(to: m05TestPdf)

        let m05Session = try client.openScopedDocument(at: m05TestPdf)
        _ = try client.rotatePages(sessionId: m05Session.sessionId, pageIndices: [0], deltaDegrees: 90)
        let dirtySnap = try client.sessionSnapshot(sessionId: m05Session.sessionId)
        guard dirtySnap.isDirty else {
            fatalError("Expected session to be dirty after rotation in M0.5 handshake")
        }

        // 1. Safe save of dirty session:
        let saveOutcome = try client.saveSession(sessionId: m05Session.sessionId)
        guard case .success(let succ) = saveOutcome else {
            fatalError("Expected successful save outcome on clean target file")
        }
        guard succ.savedRevision == 2 else {
            fatalError("Expected saved revision == 2, got \(succ.savedRevision)")
        }
        let cleanSnap = try client.sessionSnapshot(sessionId: m05Session.sessionId)
        guard !cleanSnap.isDirty else {
            fatalError("Expected session isDirty == false after successful save")
        }

        // 2. Simulate external file modification on disk:
        Thread.sleep(forTimeInterval: 0.02)
        var extBytes = origPdfBytes
        extBytes.append(Data("\n%external edit from competitor app\n".utf8))
        try extBytes.write(to: m05TestPdf)

        // Verify proactive check detects external change:
        let extCheck = try client.checkExternalChanges(sessionId: m05Session.sessionId)
        guard extCheck.hasExternalChanges && extCheck.comparisonSummary.contains("ModifiedExternally") else {
            fatalError("Expected proactive check to detect ModifiedExternally")
        }

        // Make another local mutation:
        _ = try client.rotatePages(sessionId: m05Session.sessionId, pageIndices: [0], deltaDegrees: 90)

        // 3. M0.5 GATE VERIFICATION: Save MUST NOT silently overwrite!
        let conflictOutcome = try client.saveSession(sessionId: m05Session.sessionId)
        guard case .conflict(let conflict) = conflictOutcome else {
            fatalError("FATAL: M0.5 Gate breached! External file was silently overwritten.")
        }
        guard conflict.comparisonSummary.contains("ModifiedExternally") else {
            fatalError("Expected conflict comparison to contain ModifiedExternally, got \(conflict.comparisonSummary)")
        }
        guard conflict.suggestedSafeCopyPath.contains("Conflicted Copy") else {
            fatalError("Expected suggestedSafeCopyPath to suggest Conflicted Copy, got \(conflict.suggestedSafeCopyPath)")
        }

        // Verify disk content was NOT overwritten:
        let onDisk = try Data(contentsOf: m05TestPdf)
        guard onDisk == extBytes else {
            fatalError("FATAL: Disk content was corrupted or overwritten!")
        }

        // 4. Resolve conflict via collision-free safe copy:
        guard let safeCopySnap = try client.resolveSaveConflict(
            sessionId: m05Session.sessionId,
            resolution: .saveLocalAsCollisionFreeCopy
        ) else {
            fatalError("Expected resolveSaveConflict to return safe copy snapshot")
        }
        guard safeCopySnap.targetToken.contains("Safe Copy") && FileManager.default.fileExists(atPath: safeCopySnap.targetToken) else {
            fatalError("Expected collision-free safe copy file to exist on disk: \(safeCopySnap.targetToken)")
        }

        // 5. Collision-free path generator verification:
        let generatedCollPath = client.generateCollisionFreePath(basePath: m05TestPdf.path, suffix: "Safe Copy")
        guard generatedCollPath.contains("Safe Copy") else {
            fatalError("Unexpected generated collision-free path: \(generatedCollPath)")
        }

        try client.closeSession(sessionId: m05Session.sessionId)
        print("✓ Swift (macOS M0.5) → SafeSave → AtomicStaging → PostSaveValidation → ExternalConflictDetection → GateVerified(no-silent-overwrite) → CollisionFreeSafeCopy(\(safeCopySnap.savedRevision) rev) verified")

        // M0.6 verification: Crash Recovery Container, Crash Detection, Restore / Preview / Discard / Open as New, and Gate Invariant
        let m06TempDir = FileManager.default.temporaryDirectory.appendingPathComponent("voc_mac_m06_\(ProcessInfo.processInfo.processIdentifier)")
        try FileManager.default.createDirectory(at: m06TempDir, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: m06TempDir)
        }
        let m06TestPdf = m06TempDir.appendingPathComponent("m06_rec_test.pdf")
        try origPdfBytes.write(to: m06TestPdf)
        let initialM06Bytes = try Data(contentsOf: m06TestPdf)

        let m06Session = try client.openScopedDocument(at: m06TestPdf)
        let docId = m06Session.documentId

        // Mutate session:
        _ = try client.rotatePages(sessionId: m06Session.sessionId, pageIndices: [0], deltaDegrees: 90)

        // Simulate crash: session is dirty and active, process crash simulated by inspecting recovery container before close/save
        guard let recInfo = try client.checkDocumentRecovery(sourceToken: m06TestPdf.path) else {
            fatalError("Expected recovery info to be detected for dirty unclosed document")
        }
        guard recInfo.documentId == docId else {
            fatalError("Expected docId '\(docId)', got '\(recInfo.documentId)'")
        }
        guard recInfo.uncommittedRevisions >= 1 else {
            fatalError("Expected uncommitted revisions >= 1, got \(recInfo.uncommittedRevisions)")
        }

        // Scan all recoverable documents:
        let allRec = try client.scanRecoverableDocuments()
        guard allRec.contains(where: { $0.documentId == docId }) else {
            fatalError("Expected scanned recoverable documents to include '\(docId)'")
        }

        // M0.6 GATE VERIFICATION: Original disk file remains 100% byte-for-byte identical!
        let diskDuringCrash = try Data(contentsOf: m06TestPdf)
        guard diskDuringCrash == initialM06Bytes else {
            fatalError("FATAL: M0.6 Gate breached! Original file was modified during crash or before user save.")
        }

        // Execute Restore action:
        let restoreResult = try client.executeRecoveryAction(documentId: docId, action: .restore)
        guard restoreResult.action == FfiRecoveryAction.restore else {
            fatalError("Expected restore action result")
        }
        guard let restoredSnap = restoreResult.sessionSnapshot else {
            fatalError("Expected restored session snapshot")
        }

        // Verify disk file is STILL 100% untouched after Restore:
        let diskAfterRestore = try Data(contentsOf: m06TestPdf)
        guard diskAfterRestore == initialM06Bytes else {
            fatalError("FATAL: M0.6 Gate breached! Original file was modified during recovery restore.")
        }

        try client.closeSession(sessionId: m06Session.sessionId)
        try client.closeSession(sessionId: restoredSnap.sessionId)
        print("✓ Swift (macOS M0.6) → RecoveryContainer → CrashDetection → RestoreAction → GateVerified(disk-untouched-until-save) verified")

        // -------------------------------------------------------------
        // M0.7 INTEROPERABILITY GATE VERIFICATION (ROADMAP.md M0.7)
        // Full M0 Workflow: Open -> Search -> Select -> Highlight -> Rotate -> Undo -> Redo -> Save -> Crash -> Recover -> Save -> Foreign Viewer Reopen
        // -------------------------------------------------------------
        let interopReport = try client.runInteropGateSuite()
        guard interopReport.m0ScenarioPassed else {
            fatalError("M0.7 Interop Gate failed: DoD workflow did not complete successfully.")
        }
        guard interopReport.mupdfReopenVerified && interopReport.mutoolVerified else {
            fatalError("M0.7 Interop Gate failed: MuPDF verification failed.")
        }
        // The foreign viewers of the gate are `sips` and the PDFium of
        // Chrome — both belong to macOS. On another platform the gate checks
        // the viewers that exist there, and says which ones it could not run.
        #if canImport(Darwin)
        guard interopReport.coregraphicsSipsVerified && interopReport.chromePdfiumVerified else {
            fatalError("M0.7 Interop Gate failed: foreign viewer verification failed.")
        }
        #else
        guard interopReport.mupdfReopenVerified && interopReport.mutoolVerified else {
            fatalError("M0.7 Interop Gate failed: the viewers of this platform did not open the document.")
        }
        print("NOTE: the foreign viewers of the gate (`sips`, PDFium of Chrome) are NOT RUN on this platform — they belong to macOS.")
        #endif
        print("✓ Swift (macOS M0.7) → InteropGate → FullWorkflow → MuPDF(\(interopReport.mupdfReopenVerified)) → mutool(\(interopReport.mutoolVerified)) → Quartz(\(interopReport.coregraphicsSipsVerified)) → PDFium(\(interopReport.chromePdfiumVerified)) → AllGatesPassed")
    }
}
