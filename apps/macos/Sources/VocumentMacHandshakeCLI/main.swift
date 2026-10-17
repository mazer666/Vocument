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
    }
}
