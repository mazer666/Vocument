import Foundation
import CoreGraphics
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
        guard handshake.milestone == "M0" else {
            fatalError("Expected milestone 'M0', got '\(handshake.milestone)'")
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
        guard let cgImage = DocumentCoreClient.makeCGImage(from: viewport.visibleTile),
              cgImage.width == 595, cgImage.height == 842 else {
            fatalError("Failed to construct native CGImage from rendered RGBA8 tile")
        }

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
        guard DocumentCoreClient.makeCGImage(from: thumb0) != nil else {
            fatalError("Failed to create CGImage from thumbnail")
        }
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
    }
}
